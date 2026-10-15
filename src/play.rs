//! Play the Monty Hall game yourself, against the exact answers.

use std::io::{BufRead, Write};

use crate::paradox::{pct, Lang};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub switched: u64,
    pub switched_won: u64,
    pub stayed: u64,
    pub stayed_won: u64,
}

fn rate(won: u64, of: u64) -> String {
    if of == 0 {
        "-".to_string()
    } else {
        format!("{won}/{of} ({})", pct(won as f64 / of as f64))
    }
}

fn summary(t: &Tally, doors: u32, lang: Lang) -> String {
    let (sw, st) = (
        rate(t.switched_won, t.switched),
        rate(t.stayed_won, t.stayed),
    );
    let (es, et) = (
        pct((doors - 1) as f64 / doors as f64),
        pct(1.0 / doors as f64),
    );
    match lang {
        Lang::En => format!("Switching won {sw} (exact {es}); staying won {st} (exact {et})."),
        Lang::Ja => format!("変えて当たり {sw}(正解 {es})、変えずに当たり {st}(正解 {et})。"),
    }
}

/// Read one answer; None at the end of input or on "q".
fn ask(
    input: &mut impl BufRead,
    out: &mut impl Write,
    prompt: &str,
) -> std::io::Result<Option<String>> {
    write!(out, "{prompt}")?;
    out.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let a = line.trim().to_lowercase();
    Ok(if a == "q" || a == "quit" {
        None
    } else {
        Some(a)
    })
}

/// The game loop, line by line: pick a door, see the host open the others
/// but one, then switch or stay. Ends on "q" or the end of input.
pub fn monty_hall(
    doors: u32,
    seed: u64,
    lang: Lang,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> std::io::Result<Tally> {
    let mut rng = Rng::new(seed);
    let mut t = Tally::default();
    let n = doors as u64;
    match lang {
        Lang::En => writeln!(
            out,
            "Monty Hall with {doors} doors (seed {seed}). Type q to stop."
        )?,
        Lang::Ja => writeln!(
            out,
            "ドア{doors}枚のモンティ・ホール(seed {seed})。qで終了します。"
        )?,
    }
    'game: loop {
        let car = rng.below(n);
        let prompt = match lang {
            Lang::En => format!("\nPick a door (1-{doors}): "),
            Lang::Ja => format!("\nドアを選んでください(1-{doors}): "),
        };
        let pick = loop {
            match ask(input, out, &prompt)? {
                None => break 'game,
                Some(a) => match a.parse::<u64>() {
                    Ok(d) if (1..=n).contains(&d) => break d - 1,
                    _ => match lang {
                        Lang::En => writeln!(out, "Type a number from 1 to {doors}.")?,
                        Lang::Ja => writeln!(out, "1から{doors}の数字を入力してください。")?,
                    },
                },
            }
        };
        let kept = if car != pick {
            car
        } else {
            let k = rng.below(n - 1);
            if k >= pick {
                k + 1
            } else {
                k
            }
        };
        let prompt = match lang {
            Lang::En => format!(
                "The host opens the doors other than yours and door {}: all goats. Switch to door {}? (y/n): ",
                kept + 1,
                kept + 1
            ),
            Lang::Ja => format!(
                "司会者は、あなたのドアとドア{}以外を開けました。すべてヤギです。ドア{}に変えますか?(y/n): ",
                kept + 1,
                kept + 1
            ),
        };
        let switch = loop {
            match ask(input, out, &prompt)?.as_deref() {
                None => break 'game,
                Some("y" | "yes") => break true,
                Some("n" | "no") => break false,
                Some(_) => match lang {
                    Lang::En => writeln!(out, "Type y or n.")?,
                    Lang::Ja => writeln!(out, "yかnを入力してください。")?,
                },
            }
        };
        let final_pick = if switch { kept } else { pick };
        let won = final_pick == car;
        if switch {
            t.switched += 1;
            t.switched_won += won as u64;
        } else {
            t.stayed += 1;
            t.stayed_won += won as u64;
        }
        match lang {
            Lang::En => writeln!(
                out,
                "The car was behind door {}: you {}.",
                car + 1,
                if won { "win" } else { "lose" }
            )?,
            Lang::Ja => writeln!(
                out,
                "車はドア{}でした: {}",
                car + 1,
                if won { "当たり" } else { "はずれ" }
            )?,
        }
        writeln!(out, "{}", summary(&t, doors, lang))?;
    }
    writeln!(out, "\n{}", summary(&t, doors, lang))?;
    Ok(t)
}
