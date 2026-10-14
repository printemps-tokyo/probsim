use std::io::Write;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use probsim::benford::{self, Source};
use probsim::output::{benford_json, benford_report, outcome_json, report, summary, TextOptions};
use probsim::paradox::{secretary_best_skip, Lang, Paradox};
use probsim::sim::{simulate, Outcome};

const HELP: &str = "\
probsim: watch probability paradoxes converge, next to their exact answers.

Usage: probsim <paradox> [options]
       probsim all [options]
       probsim list

Paradoxes:
  monty-hall   Switch doors or not?             --doors N (3), --random-host
  birthday     A shared birthday in a group     --people N (23), --days N (365)
  coupon       Draws to complete a gacha        --items N (10)        (alias: gacha)
  secretary    Hire the best, deciding on the spot
                                                --candidates N (100), --skip K (best)
  buffon       Buffon's needle, and pi          --length L (1), --spacing T (2), L <= T
  benford      Leading digits                   --source powers2|fibonacci (powers2),
                                                --file PATH (your own numbers)

Options:
  --trials N     Trials to run, 1 to 100,000,000 (100,000); for benford, terms
  --seed N       Seed for a repeatable run (random when omitted; always printed)
  --check        Exit 1 if an exact answer is outside the 99.9% interval
                 (benford: p < 0.001)
  --json         JSON instead of text
  --no-chart     Leave out the convergence chart
  --width N      Chart width in columns, 10 to 200 (60)
  --lang L       auto, ja or en (auto reads LC_ALL, LC_MESSAGES, LANG)
  -h, --help     This help
  -V, --version  Version
";

/// 99.9% two-sided, for --check: a correct simulation fails it about once in 1,000 runs.
const Z999: f64 = 3.290_526_731_491_926;

struct Args {
    command: String,
    trials: u64,
    seed: Option<u64>,
    check: bool,
    json: bool,
    chart: bool,
    width: usize,
    lang: Option<Lang>,
    doors: u32,
    random_host: bool,
    people: u32,
    days: u32,
    items: u32,
    candidates: u32,
    skip: Option<u32>,
    length: f64,
    spacing: f64,
    source: Option<String>,
    file: Option<String>,
    /// Options given, to reject ones that do not apply to the command.
    given: Vec<String>,
}

fn number<T: std::str::FromStr>(flag: &str, v: Option<String>) -> Result<T, String> {
    let v = v.ok_or_else(|| format!("{flag} needs a value"))?;
    v.replace('_', "")
        .parse::<T>()
        .map_err(|_| format!("{flag}: {v:?} is not a valid number"))
}

fn in_range<T: PartialOrd + std::fmt::Display>(
    flag: &str,
    v: T,
    lo: T,
    hi: T,
) -> Result<T, String> {
    if v < lo || v > hi {
        Err(format!("{flag} must be from {lo} to {hi}"))
    } else {
        Ok(v)
    }
}

fn parse(argv: Vec<String>) -> Result<Option<Args>, String> {
    let mut it = argv.into_iter();
    let mut a = Args {
        command: String::new(),
        trials: 100_000,
        seed: None,
        check: false,
        json: false,
        chart: true,
        width: 60,
        lang: None,
        doors: 3,
        random_host: false,
        people: 23,
        days: 365,
        items: 10,
        candidates: 100,
        skip: None,
        length: 1.0,
        spacing: 2.0,
        source: None,
        file: None,
        given: Vec::new(),
    };
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let mut val = || inline.clone().or_else(|| it.next());
        match flag.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("probsim {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--trials" => a.trials = in_range(&flag, number(&flag, val())?, 1, 100_000_000)?,
            "--seed" => a.seed = Some(number(&flag, val())?),
            "--check" => a.check = true,
            "--json" => a.json = true,
            "--no-chart" => a.chart = false,
            "--width" => a.width = in_range(&flag, number(&flag, val())?, 10, 200)?,
            "--lang" => {
                a.lang = match val().as_deref() {
                    Some("auto") => None,
                    Some("ja") => Some(Lang::Ja),
                    Some("en") => Some(Lang::En),
                    _ => return Err("--lang must be auto, ja or en".into()),
                }
            }
            "--doors" => a.doors = in_range(&flag, number(&flag, val())?, 3, 1_000)?,
            "--random-host" => a.random_host = true,
            "--people" => a.people = in_range(&flag, number(&flag, val())?, 1, 100_000)?,
            "--days" => a.days = in_range(&flag, number(&flag, val())?, 1, 100_000)?,
            "--items" => a.items = in_range(&flag, number(&flag, val())?, 1, 10_000)?,
            "--candidates" => a.candidates = in_range(&flag, number(&flag, val())?, 1, 100_000)?,
            "--skip" => a.skip = Some(number(&flag, val())?),
            "--length" => a.length = number(&flag, val())?,
            "--spacing" => a.spacing = number(&flag, val())?,
            "--source" => a.source = Some(val().ok_or("--source needs a value")?),
            "--file" => a.file = Some(val().ok_or("--file needs a value")?),
            f if f.starts_with('-') => return Err(format!("unknown option {f:?}; see --help")),
            _ if a.command.is_empty() => {
                a.command = arg;
                continue;
            }
            _ => return Err(format!("unexpected argument {arg:?}; see --help")),
        }
        if flag.starts_with("--") {
            a.given.push(flag);
        }
    }
    if a.command.is_empty() {
        return Err("no paradox given; see --help or probsim list".into());
    }
    Ok(Some(a))
}

/// Options only some commands take.
fn applies(command: &str, flag: &str) -> bool {
    let only: &[(&str, &[&str])] = &[
        ("--doors", &["monty-hall"]),
        ("--random-host", &["monty-hall"]),
        ("--people", &["birthday"]),
        ("--days", &["birthday"]),
        ("--items", &["coupon", "gacha"]),
        ("--candidates", &["secretary"]),
        ("--skip", &["secretary"]),
        ("--length", &["buffon"]),
        ("--spacing", &["buffon"]),
        ("--source", &["benford"]),
        ("--file", &["benford"]),
    ];
    only.iter()
        .find(|(f, _)| *f == flag)
        .is_none_or(|(_, cmds)| cmds.contains(&command))
}

fn paradox(a: &Args, command: &str) -> Result<Paradox, String> {
    Ok(match command {
        "monty-hall" => Paradox::MontyHall {
            doors: a.doors,
            host_knows: !a.random_host,
        },
        "birthday" => Paradox::Birthday {
            people: a.people,
            days: a.days,
        },
        "coupon" | "gacha" => Paradox::Coupon { items: a.items },
        "secretary" => {
            let skip = match a.skip {
                Some(k) if k >= a.candidates => {
                    return Err(format!(
                        "--skip must be less than --candidates ({})",
                        a.candidates
                    ))
                }
                Some(k) => k,
                None => secretary_best_skip(a.candidates),
            };
            Paradox::Secretary {
                candidates: a.candidates,
                skip,
            }
        }
        "buffon" => {
            let ok = |x: f64| x.is_finite() && x > 0.0;
            if !ok(a.length) || !ok(a.spacing) {
                return Err("--length and --spacing must be positive numbers".into());
            }
            if a.length > a.spacing {
                return Err(
                    "--length must not exceed --spacing (the formula is for a short needle)".into(),
                );
            }
            Paradox::Buffon {
                length: a.length,
                spacing: a.spacing,
            }
        }
        other => return Err(format!("unknown paradox {other:?}; see probsim list")),
    })
}

fn source(a: &Args) -> Result<Source, String> {
    if let Some(path) = &a.file {
        if a.source.as_deref().is_some_and(|s| s != "file") {
            return Err("--file reads your numbers; leave out --source".into());
        }
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read {path:?}: {e}"))?;
        let numbers = benford::numbers_in(&text);
        return Ok(Source::File(path.clone(), numbers));
    }
    match a.source.as_deref() {
        None | Some("powers2") => Ok(Source::PowersOfTwo(a.trials)),
        Some("fibonacci") => Ok(Source::Fibonacci(a.trials)),
        Some("file") => Err("--source file needs --file PATH".into()),
        Some(other) => Err(format!(
            "--source must be powers2 or fibonacci, not {other:?}"
        )),
    }
}

const LIST: &[(&str, &str)] = &[
    (
        "monty-hall",
        "Switch doors or not? (also --random-host: the careless host)",
    ),
    ("birthday", "How likely is a shared birthday in a group?"),
    (
        "coupon",
        "How many draws to complete a gacha? (alias: gacha)",
    ),
    (
        "secretary",
        "Hire the best candidate, deciding on the spot: the 37% rule",
    ),
    ("buffon", "Buffon's needle: estimate pi by dropping needles"),
    (
        "benford",
        "Why 1 leads 30% of the time: Benford's law, also on your own data",
    ),
];

fn run(a: Args) -> Result<ExitCode, String> {
    let lang = a
        .lang
        .unwrap_or_else(|| Lang::from_env(|k| std::env::var(k).ok()));
    let seed = a.seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64 ^ std::process::id() as u64)
    });
    if let Some(f) = a.given.iter().find(|f| !applies(&a.command, f)) {
        return Err(format!("{f} does not apply to {:?}; see --help", a.command));
    }
    let opt = TextOptions {
        lang,
        chart: a.chart,
        width: a.width,
    };
    let points = a.width.max(60);
    let mut out = std::io::stdout().lock();
    let write = |out: &mut std::io::StdoutLock, s: &str| {
        out.write_all(s.as_bytes()).map_err(|e| e.to_string())
    };
    let ok_outcome = |o: &Outcome| !a.check || o.consistent(Z999);
    let ok_benford = |p: f64| !a.check || p >= 0.001;
    match a.command.as_str() {
        "list" => {
            for (name, about) in LIST {
                write(&mut out, &format!("{name:<12} {about}\n"))?;
            }
            Ok(ExitCode::SUCCESS)
        }
        "benford" => {
            let src = source(&a)?;
            let analysis = benford::analyze(&src.digits());
            if analysis.total == 0 {
                return Err("no nonzero numbers found".into());
            }
            let text = if a.json {
                benford_json(&src, &analysis) + "\n"
            } else {
                benford_report(&src, &analysis, &opt)
            };
            write(&mut out, &text)?;
            Ok(if ok_benford(analysis.p_value) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        "all" => {
            let commands = ["monty-hall", "birthday", "coupon", "secretary", "buffon"];
            let mut outcomes = Vec::new();
            for c in commands {
                outcomes.push(simulate(&paradox(&a, c)?, a.trials, seed, points));
            }
            let src = Source::PowersOfTwo(a.trials);
            let analysis = benford::analyze(&src.digits());
            let text = if a.json {
                format!(
                    "{{\"seed\":{seed},\"results\":[{}],\"benford\":{}}}\n",
                    outcomes
                        .iter()
                        .map(outcome_json)
                        .collect::<Vec<_>>()
                        .join(","),
                    benford_json(&src, &analysis)
                )
            } else {
                summary(&outcomes, Some((&src, &analysis)), lang)
                    + &match lang {
                        Lang::En => format!(
                            "\n{} trials each, seed {seed}\n",
                            probsim::output::grouped(a.trials)
                        ),
                        Lang::Ja => format!(
                            "\n各{}回、seed {seed}\n",
                            probsim::output::grouped(a.trials)
                        ),
                    }
            };
            write(&mut out, &text)?;
            let ok = outcomes.iter().all(ok_outcome) && ok_benford(analysis.p_value);
            Ok(if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        c => {
            let p = paradox(&a, c)?;
            let o = simulate(&p, a.trials, seed, points);
            if o.counted == 0 {
                return Err("no trial counted; run more trials".into());
            }
            let text = if a.json {
                outcome_json(&o) + "\n"
            } else {
                report(&o, &opt)
            };
            write(&mut out, &text)?;
            Ok(if ok_outcome(&o) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match parse(argv).and_then(|a| match a {
        None => Ok(ExitCode::SUCCESS),
        Some(a) => run(a),
    }) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("probsim: {e}");
            ExitCode::from(2)
        }
    }
}
