use std::io::Write;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use probsim::benford::{self, Source};
use probsim::output::{
    benford_json, benford_report, explain_block, outcome_json, report, summary, TextOptions,
};
use probsim::paradox::{secretary_best_skip, Lang, Paradox};
use probsim::rng::Rng;
use probsim::sim::{derived_seed, simulate, Outcome};

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
                                                --file PATH (your own numbers),
                                                --column N, --delimiter C (CSV)

Options:
  --trials N     Trials to run, 1 to 100,000,000 (100,000); for benford, terms
  --explain N    Tell the first N trials step by step, 1 to 100 (same seed, same trials)
  --seed N       Seed for a repeatable run, 0 to 9007199254740991 (random when
                 omitted; always printed)
  --check        Exit 1 if an exact answer is outside the 99.9% interval
                 (benford: p < 0.001); exit 2 if there are too few trials to judge
  --json         JSON instead of text
  --no-chart     Leave out the convergence chart
  --width N      Chart width in columns, 10 to 200 (60)
  --lang L       auto, ja or en (auto reads LC_ALL, LC_MESSAGES, LANG)
  -h, --help     This help
  -V, --version  Version
";

/// 99.9% two-sided, for --check: with enough trials, a correct simulation
/// fails one check about once in 1,000 runs.
const Z999: f64 = 3.290_526_731_491_926;

/// The largest seed that survives a trip through JSON numbers (2^53 - 1).
const MAX_SEED: u64 = (1 << 53) - 1;

struct Args {
    command: String,
    trials: u64,
    explain: u32,
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
    column: Option<usize>,
    delimiter: Option<char>,
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

/// Options that take no value.
const SWITCHES: &[&str] = &[
    "--check",
    "--json",
    "--no-chart",
    "--random-host",
    "--help",
    "--version",
];

fn parse(argv: Vec<String>) -> Result<Option<Args>, String> {
    let mut it = argv.into_iter();
    let mut a = Args {
        command: String::new(),
        trials: 100_000,
        explain: 0,
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
        column: None,
        delimiter: None,
        given: Vec::new(),
    };
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        if inline.is_some() && SWITCHES.contains(&flag.as_str()) {
            return Err(format!("{flag} takes no value"));
        }
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
            "--explain" => a.explain = in_range(&flag, number(&flag, val())?, 1, 100)?,
            "--seed" => a.seed = Some(in_range(&flag, number(&flag, val())?, 0, MAX_SEED)?),
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
            "--column" => a.column = Some(in_range(&flag, number(&flag, val())?, 1, 10_000)?),
            "--delimiter" => {
                a.delimiter = Some(match val().as_deref() {
                    Some("tab") | Some("\t") => '\t',
                    Some(s) if s.chars().count() == 1 => s.chars().next().unwrap(),
                    _ => return Err("--delimiter must be one character, or tab".into()),
                })
            }
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

/// Whether an option means anything for a command.
fn applies(a: &Args, flag: &str) -> bool {
    let c = a.command.as_str();
    let sims = [
        "monty-hall",
        "birthday",
        "coupon",
        "gacha",
        "secretary",
        "buffon",
    ];
    match flag {
        _ if c == "list" => false,
        "--doors" | "--random-host" => c == "monty-hall",
        "--people" | "--days" => c == "birthday",
        "--items" => c == "coupon" || c == "gacha",
        "--candidates" | "--skip" => c == "secretary",
        "--length" | "--spacing" => c == "buffon",
        "--source" | "--file" => c == "benford",
        "--column" | "--delimiter" => c == "benford" && a.file.is_some(),
        // A file has as many numbers as it has; --trials sets a sequence's length.
        "--trials" => c != "benford" || a.file.is_none(),
        "--explain" => sims.contains(&c),
        "--no-chart" | "--width" => c != "all",
        _ => true,
    }
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
            let r = a.length / a.spacing;
            if !(r.is_finite() && r > 0.0) {
                return Err("--length and --spacing are too far apart in size".into());
            }
            if r > 1.0 {
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
        // Not necessarily UTF-8 (a Shift_JIS CSV, say): numbers are ASCII,
        // so anything else is replaced and ignored.
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read {path:?}: {e}"))?;
        let text = String::from_utf8_lossy(&bytes);
        let scan = match a.column {
            Some(c) => benford::scan_csv(&text, c, a.delimiter.unwrap_or(',')),
            None => benford::scan_text(&text),
        };
        return Ok(Source::File {
            name: path.clone(),
            column: a.column,
            scan,
        });
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

/// Too few trials to judge, under --check.
fn undecided(what: &str) -> String {
    format!("--check cannot judge {what}: too few trials (an average needs at least 30; benford needs 110 numbers)")
}

fn run(a: Args) -> Result<ExitCode, String> {
    let lang = a
        .lang
        .unwrap_or_else(|| Lang::from_env(|k| std::env::var(k).ok()));
    let seed = a.seed.unwrap_or_else(|| {
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        derived_seed(t, std::process::id() as u64)
    });
    if let Some(f) = a.given.iter().find(|f| !applies(&a, f)) {
        return Err(format!(
            "{f} does not apply to {:?} here; see --help",
            a.command
        ));
    }
    let opt = TextOptions {
        lang,
        chart: a.chart,
        width: a.width,
    };
    let points = a.width.max(60);
    let mut out = std::io::stdout().lock();
    let mut write = |s: &str| out.write_all(s.as_bytes()).map_err(|e| e.to_string());
    match a.command.as_str() {
        "list" => {
            for (name, about) in LIST {
                write(&format!("{name:<12} {about}\n"))?;
            }
            Ok(ExitCode::SUCCESS)
        }
        "benford" => {
            let src = source(&a)?;
            let analysis = benford::analyze(src.counts());
            if analysis.total == 0 {
                return Err("no nonzero numbers found".into());
            }
            let text = if a.json {
                benford_json(&src, &analysis) + "\n"
            } else {
                benford_report(&src, &analysis, &opt)
            };
            write(&text)?;
            if !a.check {
                return Ok(ExitCode::SUCCESS);
            }
            match analysis.p_value {
                Some(p) if p >= 0.001 => Ok(ExitCode::SUCCESS),
                Some(_) => Ok(ExitCode::FAILURE),
                None => Err(undecided("benford")),
            }
        }
        "all" => {
            let commands = ["monty-hall", "birthday", "coupon", "secretary", "buffon"];
            let mut outcomes = Vec::new();
            for (i, c) in commands.iter().enumerate() {
                // Each paradox gets its own random stream, derived from the seed.
                outcomes.push(simulate(
                    &paradox(&a, c)?,
                    a.trials,
                    derived_seed(seed, i as u64),
                    points,
                ));
            }
            let src = Source::PowersOfTwo(a.trials);
            let analysis = benford::analyze(src.counts());
            let text = if a.json {
                format!(
                    "{{\"seed\":{seed},\"results\":[{}],\"benford\":{}}}\n",
                    outcomes
                        .iter()
                        .map(|o| outcome_json(o, &[]))
                        .collect::<Vec<_>>()
                        .join(","),
                    benford_json(&src, &analysis)
                )
            } else {
                summary(&outcomes, Some((&src, &analysis)), lang)
                    + &match lang {
                        Lang::En => format!(
                            "\n{} trials each, seed {seed} (each paradox gets its own stream from it; see --json)\n",
                            probsim::output::grouped(a.trials)
                        ),
                        Lang::Ja => format!(
                            "\n各{}回、seed {seed}(各パラドックスはここから導いた別々の乱数列。--jsonに各seedを出力)\n",
                            probsim::output::grouped(a.trials)
                        ),
                    }
            };
            write(&text)?;
            if !a.check {
                return Ok(ExitCode::SUCCESS);
            }
            let mut ok = true;
            for o in &outcomes {
                match o.consistent(Z999) {
                    Some(c) => ok &= c,
                    None => return Err(undecided(o.paradox.name())),
                }
            }
            match analysis.p_value {
                Some(p) => ok &= p >= 0.001,
                None => return Err(undecided("benford")),
            }
            Ok(if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        c => {
            let p = paradox(&a, c)?;
            // --explain replays the first trials of the same seed with their
            // stories; the simulation below repeats them and goes on.
            let mut stories = Vec::new();
            if a.explain > 0 {
                let mut rng = Rng::new(seed);
                let mut scratch = Vec::new();
                for _ in 0..a.explain.min(a.trials.min(u32::MAX as u64) as u32) {
                    stories.push(p.explain(&mut rng, &mut scratch, lang));
                }
            }
            let o: Outcome = simulate(&p, a.trials, seed, points);
            if o.counted == 0 {
                return Err("no trial counted; run more trials".into());
            }
            let text = if a.json {
                outcome_json(&o, &stories) + "\n"
            } else {
                let mut t = report(&o, &opt);
                if !stories.is_empty() {
                    t = format!("{}\n{t}", explain_block(&p, &stories, lang));
                }
                t
            };
            write(&text)?;
            if !a.check {
                return Ok(ExitCode::SUCCESS);
            }
            match o.consistent(Z999) {
                Some(true) => Ok(ExitCode::SUCCESS),
                Some(false) => Ok(ExitCode::FAILURE),
                None => Err(undecided(c)),
            }
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
