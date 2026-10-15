use std::io::{Read, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use probsim::benford::{self, Source};
use probsim::exact::MAX_WEIGHTED_BUDGET_ITEMS;
use probsim::export;
use probsim::output::{
    benford_json, benford_report, coverage_json, coverage_report, explain_block, outcome_json,
    report, summary, sweep_json, sweep_report, TextOptions,
};
use probsim::paradox::{coins, secretary_best_skip, Chord, Lang, Paradox, Weights};
use probsim::rng::Rng;
use probsim::sim::{derived_seed, simulate, Outcome};
use probsim::study;

const HELP: &str = "\
probsim: watch probability paradoxes converge, next to their exact answers.

Usage: probsim <paradox> [options]
       probsim all [options]
       probsim sweep <paradox> --vary NAME --from A --to B [--steps N] [options]
                 (--steps: every whole value when there are under 40, else 21)
       probsim coverage <paradox> [--runs N (100)] [options]
       probsim play monty-hall [--doors N]
       probsim list

Paradoxes:
  monty-hall   Switch doors or not?           --doors N (3), --random-host
  birthday     A shared birthday in a group   --people N (23), --days N (365),
                                              --day-weights PATH (one weight per day)
  coupon       Draws to complete a gacha      --items N (10), --weights LIST or
               (alias: gacha)                 --weights-file PATH, --budget N
  secretary    Hire the best on the spot      --candidates N (100), --skip K (best)
  buffon       Buffon's needle, and pi        --length L (1), --spacing T (2), L <= T
  base-rate    A positive test: sick?         --prevalence P (0.01), --sensitivity P
                                              (0.99), --specificity P (0.95)
  penney       Penney's coin game             --first HHT, --second SEQ (best reply)
  boy-girl     Two children, one a boy...     --any-day (drop the Tuesday)
  bertrand     A random chord                 --method endpoints|radius|midpoint
  benford      Leading digits                 --source powers2|fibonacci,
                                              --file PATH (- for stdin), --column N,
                                              --delimiter C, --digits 1|2

Options:
  --trials N     Trials, 1 to 100,000,000 (100,000; sweep 10,000; coverage
                 1,000); for benford, terms
  --explain N    Tell the first N trials step by step, 1 to 100 (same seed, same trials)
  --seed N       Seed for a repeatable run, 0 to 9007199254740991 (random when
                 omitted; always printed)
  --check        Exit 1 if an exact answer is outside the 99.9% interval
                 (benford: p < 0.001); exit 2 if there are too few trials to judge
  --json         JSON instead of text
  --csv          CSV instead of text (a run's checkpoints, a sweep's points,
                 a coverage study's runs)
  --svg PATH     Also write the convergence chart as an SVG file
  --no-chart     Leave out the chart
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

/// The paradoxes that simulate (not benford), in `list` and `all` order.
const SIMS: &[&str] = &[
    "monty-hall",
    "birthday",
    "coupon",
    "secretary",
    "buffon",
    "base-rate",
    "penney",
    "boy-girl",
    "bertrand",
];

#[derive(Clone)]
struct Args {
    command: String,
    target: Option<String>,
    trials: Option<u64>,
    explain: u32,
    seed: Option<u64>,
    check: bool,
    json: bool,
    csv: bool,
    svg: Option<String>,
    chart: bool,
    width: usize,
    lang: Option<Lang>,
    doors: u32,
    random_host: bool,
    people: u32,
    days: Option<u32>,
    day_weights: Option<String>,
    /// The --day-weights file's contents, read once (so a sweep or standard
    /// input works).
    day_weights_text: Option<String>,
    items: Option<u32>,
    weights: Option<String>,
    weights_file: Option<String>,
    /// The --weights-file contents, read once.
    weights_text: Option<String>,
    budget: Option<u64>,
    candidates: u32,
    skip: Option<u32>,
    length: f64,
    spacing: f64,
    prevalence: f64,
    sensitivity: f64,
    specificity: f64,
    first: String,
    second: Option<String>,
    any_day: bool,
    method: Chord,
    source: Option<String>,
    file: Option<String>,
    column: Option<usize>,
    delimiter: Option<char>,
    digits: usize,
    vary: Option<String>,
    from: Option<f64>,
    to: Option<f64>,
    steps: usize,
    runs: u64,
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

/// A probability: 0.05 or 5%.
fn fraction(flag: &str, v: Option<String>) -> Result<f64, String> {
    let v = v.ok_or_else(|| format!("{flag} needs a value"))?;
    let x = match v.strip_suffix('%') {
        Some(p) => p.trim().parse::<f64>().map(|x| x / 100.0),
        None => v.parse::<f64>(),
    }
    .map_err(|_| format!("{flag}: {v:?} is not a probability (like 0.05 or 5%)"))?;
    if (0.0..=1.0).contains(&x) {
        Ok(x)
    } else {
        Err(format!("{flag} must be from 0 to 1 (or 0% to 100%)"))
    }
}

/// Options that take no value.
const SWITCHES: &[&str] = &[
    "--check",
    "--json",
    "--csv",
    "--no-chart",
    "--random-host",
    "--any-day",
    "--help",
    "--version",
];

fn parse(argv: Vec<String>) -> Result<Option<Args>, String> {
    let mut it = argv.into_iter();
    let mut a = Args {
        command: String::new(),
        target: None,
        trials: None,
        explain: 0,
        seed: None,
        check: false,
        json: false,
        csv: false,
        svg: None,
        chart: true,
        width: 60,
        lang: None,
        doors: 3,
        random_host: false,
        people: 23,
        days: None,
        day_weights: None,
        day_weights_text: None,
        items: None,
        weights: None,
        weights_file: None,
        weights_text: None,
        budget: None,
        candidates: 100,
        skip: None,
        length: 1.0,
        spacing: 2.0,
        prevalence: 0.01,
        sensitivity: 0.99,
        specificity: 0.95,
        first: "HHT".into(),
        second: None,
        any_day: false,
        method: Chord::Endpoints,
        source: None,
        file: None,
        column: None,
        delimiter: None,
        digits: 1,
        vary: None,
        from: None,
        to: None,
        steps: 0,
        runs: 100,
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
            "--trials" => a.trials = Some(in_range(&flag, number(&flag, val())?, 1, 100_000_000)?),
            "--explain" => a.explain = in_range(&flag, number(&flag, val())?, 1, 100)?,
            "--seed" => a.seed = Some(in_range(&flag, number(&flag, val())?, 0, MAX_SEED)?),
            "--check" => a.check = true,
            "--json" => a.json = true,
            "--csv" => a.csv = true,
            "--svg" => a.svg = Some(val().ok_or("--svg needs a file name")?),
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
            "--days" => a.days = Some(in_range(&flag, number(&flag, val())?, 1, 100_000)?),
            "--day-weights" => {
                a.day_weights = Some(val().ok_or("--day-weights needs a file name")?)
            }
            "--items" => a.items = Some(in_range(&flag, number(&flag, val())?, 1, 10_000)?),
            "--weights" => a.weights = Some(val().ok_or("--weights needs a list, like 1,1,1,0.2")?),
            "--weights-file" => {
                a.weights_file = Some(val().ok_or("--weights-file needs a file name")?)
            }
            "--budget" => a.budget = Some(in_range(&flag, number(&flag, val())?, 1, 100_000_000)?),
            "--candidates" => a.candidates = in_range(&flag, number(&flag, val())?, 1, 100_000)?,
            "--skip" => a.skip = Some(number(&flag, val())?),
            "--length" => a.length = number(&flag, val())?,
            "--spacing" => a.spacing = number(&flag, val())?,
            "--prevalence" => a.prevalence = fraction(&flag, val())?,
            "--sensitivity" => a.sensitivity = fraction(&flag, val())?,
            "--specificity" => a.specificity = fraction(&flag, val())?,
            "--first" => a.first = val().ok_or("--first needs a sequence, like HHT")?,
            "--second" => a.second = Some(val().ok_or("--second needs a sequence, like THH")?),
            "--any-day" => a.any_day = true,
            "--method" => {
                a.method = match val().as_deref() {
                    Some("endpoints") => Chord::Endpoints,
                    Some("radius") => Chord::Radius,
                    Some("midpoint") => Chord::Midpoint,
                    _ => return Err("--method must be endpoints, radius or midpoint".into()),
                }
            }
            "--source" => a.source = Some(val().ok_or("--source needs a value")?),
            "--file" => {
                a.file = Some(val().ok_or("--file needs a file name, or - for standard input")?)
            }
            "--column" => a.column = Some(in_range(&flag, number(&flag, val())?, 1, 10_000)?),
            "--delimiter" => {
                a.delimiter = Some(match val().as_deref() {
                    Some("tab") | Some("\t") => '\t',
                    Some(s) if s.chars().count() == 1 => s.chars().next().unwrap(),
                    _ => return Err("--delimiter must be one character, or tab".into()),
                })
            }
            "--digits" => a.digits = in_range(&flag, number(&flag, val())?, 1, 2)?,
            "--vary" => a.vary = Some(val().ok_or("--vary needs a parameter name")?),
            "--from" => a.from = Some(number(&flag, val())?),
            "--to" => a.to = Some(number(&flag, val())?),
            "--steps" => a.steps = in_range(&flag, number(&flag, val())?, 2, 200)?,
            "--runs" => a.runs = in_range(&flag, number(&flag, val())?, 1, 100_000)?,
            f if f.starts_with('-') && f != "-" => {
                return Err(format!("unknown option {f:?}; see --help"))
            }
            _ if a.command.is_empty() => {
                a.command = arg;
                continue;
            }
            _ if a.target.is_none()
                && matches!(a.command.as_str(), "sweep" | "coverage" | "play") =>
            {
                a.target = Some(arg);
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

/// The paradox a command runs: the command itself, or the one after
/// sweep, coverage or play.
fn subject(a: &Args) -> &str {
    a.target.as_deref().unwrap_or(&a.command)
}

/// Whether an option means anything here.
fn applies(a: &Args, flag: &str) -> bool {
    let (c, p) = (a.command.as_str(), subject(a));
    let study = matches!(c, "sweep" | "coverage");
    match flag {
        _ if c == "list" => false,
        _ if c == "play" => matches!(flag, "--doors" | "--seed" | "--lang"),
        "--doors" | "--random-host" => p == "monty-hall",
        "--people" | "--days" | "--day-weights" => p == "birthday",
        "--items" | "--weights" | "--weights-file" | "--budget" => p == "coupon" || p == "gacha",
        "--candidates" | "--skip" => p == "secretary",
        "--length" | "--spacing" => p == "buffon",
        "--prevalence" | "--sensitivity" | "--specificity" => p == "base-rate",
        "--first" | "--second" => p == "penney",
        "--any-day" => p == "boy-girl",
        "--method" => p == "bertrand",
        "--source" | "--file" | "--digits" => c == "benford",
        "--column" | "--delimiter" => c == "benford" && a.file.is_some(),
        // A file has as many numbers as it has; --trials sets a sequence's length.
        "--trials" => c != "benford" || a.file.is_none(),
        "--explain" | "--svg" => SIMS.contains(&c) || c == "gacha",
        "--vary" | "--from" | "--to" | "--steps" => c == "sweep",
        "--runs" => c == "coverage",
        "--check" => c != "sweep" && c != "coverage",
        "--csv" => c != "all" && c != "benford",
        "--no-chart" | "--width" => c != "all" || study,
        _ => true,
    }
}

/// Numbers in a list or a file ("1, 1, 0.5" or one per line).
/// Weights from a list or a file: numbers separated by commas, semicolons
/// or white space ("1,1,0.5", one per line). Each field is a whole number on
/// its own, so "1,250" is two weights, never one thousand two hundred fifty.
fn numbers_from(text: &str) -> Result<Vec<f64>, String> {
    text.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|f| !f.is_empty())
        .map(|f| {
            f.parse::<f64>()
                .map_err(|_| format!("{f:?} is not a number"))
        })
        .collect()
}

fn read_file(path: &str) -> Result<String, String> {
    let bytes = if path == "-" {
        let mut b = Vec::new();
        std::io::stdin()
            .read_to_end(&mut b)
            .map_err(|e| format!("cannot read standard input: {e}"))?;
        b
    } else {
        std::fs::read(path).map_err(|e| format!("cannot read {path:?}: {e}"))?
    };
    // Not necessarily UTF-8 (a Shift_JIS CSV, say): see benford::decode.
    benford::decode(&bytes).map_err(|e| format!("{path:?}: {e}"))
}

fn paradox(a: &Args, name: &str) -> Result<Paradox, String> {
    Ok(match name {
        "monty-hall" => Paradox::MontyHall {
            doors: a.doors,
            host_knows: !a.random_host,
        },
        "birthday" => {
            let weights = match &a.day_weights_text {
                Some(text) => {
                    let w = Weights::new(&numbers_from(text)?)
                        .map_err(|e| format!("--day-weights: {e}"))?;
                    if w.len() > 100_000 {
                        return Err("--day-weights: at most 100,000 days".into());
                    }
                    Some(Arc::new(w))
                }
                None => None,
            };
            let days = match (&weights, a.days) {
                (Some(w), Some(d)) if d as usize != w.len() => {
                    return Err(format!(
                        "--days {d} does not match the {} day weights",
                        w.len()
                    ))
                }
                (Some(w), _) => w.len() as u32,
                (None, d) => d.unwrap_or(365),
            };
            if let Some(w) = &weights {
                let positive = w.probs.iter().filter(|&&p| p > 0.0).count();
                if (a.people as usize) <= positive && a.people as f64 * w.len() as f64 > 5e8 {
                    return Err("--people x the number of day weights is too large to compute exactly (at most 500,000,000)".into());
                }
            }
            Paradox::Birthday {
                people: a.people,
                days,
                weights,
            }
        }
        "coupon" | "gacha" => {
            let list = match (&a.weights, &a.weights_file) {
                (Some(_), Some(_)) => {
                    return Err("give --weights or --weights-file, not both".into())
                }
                (Some(l), None) => Some(numbers_from(l)?),
                (None, Some(_)) => Some(numbers_from(a.weights_text.as_deref().unwrap_or(""))?),
                (None, None) => None,
            };
            let weights = match list {
                Some(l) => {
                    let w = Weights::new(&l).map_err(|e| format!("--weights: {e}"))?;
                    if w.probs.contains(&0.0) {
                        return Err(
                            "--weights: an item with weight 0 can never be collected".into()
                        );
                    }
                    if w.len() > 10_000 {
                        return Err("--weights: at most 10,000 items".into());
                    }
                    Some(Arc::new(w))
                }
                None => None,
            };
            let items = match (&weights, a.items) {
                (Some(w), Some(n)) if n as usize != w.len() => {
                    return Err(format!(
                        "--items {n} does not match the {} weights",
                        w.len()
                    ))
                }
                (Some(w), _) => w.len() as u32,
                (None, n) => n.unwrap_or(10),
            };
            let weights = weights.filter(|w| !w.uniform());
            if a.budget.is_none() && weights.as_ref().is_some_and(|w| w.coupon_mean().is_none()) {
                return Err("--weights: the exact mean did not converge for these rates".into());
            }
            if let Some(b) = a.budget {
                if weights
                    .as_ref()
                    .is_some_and(|w| w.len() > MAX_WEIGHTED_BUDGET_ITEMS)
                {
                    return Err(format!(
                        "--budget with unequal --weights is computed exactly for up to {MAX_WEIGHTED_BUDGET_ITEMS} items"
                    ));
                }
                if weights.is_none() && items as u64 * b > 500_000_000 {
                    return Err(
                        "--items x --budget is too large to compute exactly (at most 500,000,000)"
                            .into(),
                    );
                }
            }
            Paradox::Coupon {
                items,
                weights,
                budget: a.budget,
            }
        }
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
        "base-rate" => {
            let positive =
                a.prevalence * a.sensitivity + (1.0 - a.prevalence) * (1.0 - a.specificity);
            if positive <= 0.0 {
                return Err("with these values nobody ever tests positive".into());
            }
            Paradox::BaseRate {
                prevalence: a.prevalence,
                sensitivity: a.sensitivity,
                specificity: a.specificity,
            }
        }
        "penney" => {
            let first = coins(&a.first).map_err(|e| format!("--first: {e}"))?;
            let second = match &a.second {
                Some(s) => coins(s).map_err(|e| format!("--second: {e}"))?,
                None => probsim::exact::penney_best_reply(&first),
            };
            if first.len() != second.len() {
                return Err("--first and --second must be the same length".into());
            }
            if first == second {
                return Err("--first and --second must differ".into());
            }
            Paradox::Penney { first, second }
        }
        "boy-girl" => Paradox::BoyGirl {
            tuesday: !a.any_day,
        },
        "bertrand" => Paradox::Bertrand { method: a.method },
        other => return Err(format!("unknown paradox {other:?}; see probsim list")),
    })
}

/// Set a parameter by name, for a sweep; true when it is a whole number.
fn set_param(a: &mut Args, name: &str, v: f64) -> Result<(), String> {
    let whole = |lo: f64, hi: f64| -> Result<u64, String> {
        if v.fract() != 0.0 || v < lo || v > hi {
            Err(format!("{name} must be a whole number from {lo} to {hi}"))
        } else {
            Ok(v as u64)
        }
    };
    let prob = || {
        if (0.0..=1.0).contains(&v) {
            Ok(v)
        } else {
            Err(format!("{name} must be from 0 to 1"))
        }
    };
    match name {
        "doors" => a.doors = whole(3.0, 1000.0)? as u32,
        "people" => a.people = whole(1.0, 100_000.0)? as u32,
        "days" => a.days = Some(whole(1.0, 100_000.0)? as u32),
        "items" => a.items = Some(whole(1.0, 10_000.0)? as u32),
        "budget" => a.budget = Some(whole(1.0, 100_000_000.0)?),
        "candidates" => a.candidates = whole(1.0, 100_000.0)? as u32,
        "skip" => a.skip = Some(whole(0.0, 100_000.0)? as u32),
        "length" => a.length = v,
        "spacing" => a.spacing = v,
        "prevalence" => a.prevalence = prob()?,
        "sensitivity" => a.sensitivity = prob()?,
        "specificity" => a.specificity = prob()?,
        _ => {
            return Err(format!(
                "--vary {name}: not a number parameter of this paradox"
            ))
        }
    }
    Ok(())
}

fn integer_param(name: &str) -> bool {
    matches!(
        name,
        "doors" | "people" | "days" | "items" | "budget" | "candidates" | "skip"
    )
}

/// Which parameters each paradox can sweep.
fn sweepable(p: &str) -> &'static [&'static str] {
    match p {
        "monty-hall" => &["doors"],
        "birthday" => &["people", "days"],
        "coupon" | "gacha" => &["items", "budget"],
        "secretary" => &["candidates", "skip"],
        "buffon" => &["length", "spacing"],
        "base-rate" => &["prevalence", "sensitivity", "specificity"],
        _ => &[],
    }
}

fn source(a: &Args, trials: u64) -> Result<Source, String> {
    if let Some(path) = &a.file {
        if a.source.as_deref().is_some_and(|s| s != "file") {
            return Err("--file reads your numbers; leave out --source".into());
        }
        let text = read_file(path)?;
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
        None | Some("powers2") => Ok(Source::PowersOfTwo(trials)),
        Some("fibonacci") => Ok(Source::Fibonacci(trials)),
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
    (
        "birthday",
        "How likely is a shared birthday in a group? (also uneven birthdays)",
    ),
    (
        "coupon",
        "How many draws to complete a gacha? (alias: gacha; rates, budget)",
    ),
    (
        "secretary",
        "Hire the best candidate, deciding on the spot: the 37% rule",
    ),
    ("buffon", "Buffon's needle: estimate pi by dropping needles"),
    (
        "base-rate",
        "A positive test for a rare disease: how likely are you sick?",
    ),
    (
        "penney",
        "Penney's game: the second player can always pick a better sequence",
    ),
    (
        "boy-girl",
        "Two children, one a boy born on a Tuesday: both boys?",
    ),
    (
        "bertrand",
        "Bertrand's paradox: three ways to draw a random chord, three answers",
    ),
    (
        "benford",
        "Why 1 leads 30% of the time: Benford's law, also on your own data",
    ),
];

/// Too few trials to judge, under --check.
fn undecided(what: &str) -> String {
    format!("--check cannot judge {what}: too few trials (an average needs at least 30; benford needs enough numbers)")
}

/// Roughly how many basic steps (draws, flips, shuffled candidates) a run
/// takes, to refuse one that would run for a very long time.
fn work(p: &Paradox, trials: u64) -> f64 {
    let per_trial = match p {
        // A collector stops when complete, so a large budget costs no more
        // than the draws it takes on average.
        Paradox::Coupon {
            items,
            weights,
            budget: Some(b),
        } => {
            let mean = match weights {
                Some(w) => w.coupon_mean().unwrap_or(*b as f64),
                None => probsim::exact::coupon_mean_equal(*items),
            };
            (*b as f64).min(mean)
        }
        Paradox::Coupon { .. } => p.theory(),
        Paradox::Birthday { people, .. } => *people as f64,
        Paradox::Secretary { candidates, .. } => *candidates as f64,
        // The waiting time for a sequence of n flips is at most 2^(n+1).
        Paradox::Penney { first, .. } => 2f64.powi(first.len() as i32 + 1),
        _ => 1.0,
    };
    per_trial * trials as f64 + probsim::paradox::theory_cost(p)
}

/// About a minute of simulation on an ordinary machine.
const MAX_WORK: f64 = 2e10;

/// Refuse a run of `runs` x `trials` that would take too long, suggesting
/// a --trials (per run) that fits, as a plain number ready to paste.
fn affordable(p: &Paradox, trials: u64, runs: u64) -> Result<(), String> {
    let exact_cost = probsim::paradox::theory_cost(p);
    if exact_cost > MAX_WORK {
        return Err(format!("the exact answer alone would take about {exact_cost:.0e} steps; use smaller parameters"));
    }
    // The exact answer is computed once; the trials run `runs` times.
    let w = (work(p, trials) - exact_cost) * runs as f64 + exact_cost;
    if w > MAX_WORK {
        let per_trial = (w - exact_cost) / (trials as f64 * runs as f64);
        let suggest = (((MAX_WORK - exact_cost) / (per_trial * runs as f64)).floor() as u64).max(1);
        return Err(format!(
            "this would take about {w:.0e} steps; use --trials {suggest} or fewer"
        ));
    }
    Ok(())
}

fn run(mut a: Args) -> Result<ExitCode, String> {
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
    // Read weight files once, so every point of a sweep sees the same data
    // and standard input can be used.
    if let Some(path) = a.day_weights.clone() {
        a.day_weights_text = Some(read_file(&path)?);
    }
    if let Some(path) = a.weights_file.clone() {
        a.weights_text = Some(read_file(&path)?);
    }
    if a.json && a.csv {
        return Err("choose --json or --csv, not both".into());
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
        "play" => {
            if a.target.as_deref() != Some("monty-hall") {
                return Err("play supports monty-hall: probsim play monty-hall".into());
            }
            // The standard output lock is reentrant, so the game can take it too.
            let stdin = std::io::stdin();
            probsim::play::monty_hall(
                a.doors,
                seed,
                lang,
                &mut stdin.lock(),
                &mut std::io::stdout().lock(),
            )
            .map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }
        "benford" => {
            let src = source(&a, a.trials.unwrap_or(100_000))?;
            let analysis = benford::analyze(src.counts(a.digits), a.digits);
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
        "sweep" => {
            let target = a
                .target
                .clone()
                .ok_or("sweep needs a paradox: probsim sweep birthday --vary people ...")?;
            let name = a.vary.clone().ok_or("sweep needs --vary NAME")?;
            if !sweepable(&target).contains(&name.as_str()) {
                let options = sweepable(&target);
                return Err(if options.is_empty() {
                    format!("{target} has no number parameter to sweep")
                } else {
                    format!("--vary for {target} must be one of: {}", options.join(", "))
                });
            }
            let (from, to) = (
                a.from.ok_or("sweep needs --from")?,
                a.to.ok_or("sweep needs --to")?,
            );
            let integer = integer_param(&name);
            let steps = if a.steps > 0 {
                a.steps
            } else if integer && (to - from).abs() < 40.0 {
                (to - from).abs() as usize + 1
            } else {
                21
            };
            if !(from.is_finite() && to.is_finite()) {
                return Err("--from and --to must be numbers".into());
            }
            let values = study::sweep_values(from, to, steps, integer);
            if values.is_empty() {
                return Err(format!("no value of {name} lies between {from} and {to}"));
            }
            let trials = a.trials.unwrap_or(10_000);
            let make = |v: f64| -> Result<Paradox, String> {
                let mut b = a.clone();
                set_param(&mut b, &name, v)?;
                paradox(&b, &target)
            };
            // Every point is built and the whole sweep's cost checked before
            // anything runs.
            let prepared = study::prepare(&values, make)?;
            let total: f64 = prepared.iter().map(|(_, p)| work(p, trials)).sum();
            let exact_cost: f64 = prepared
                .iter()
                .map(|(_, p)| probsim::paradox::theory_cost(p))
                .sum();
            if exact_cost > MAX_WORK {
                return Err(format!(
                    "the exact answers alone would take about {exact_cost:.0e} steps; use fewer --steps or a smaller range"
                ));
            }
            if total > MAX_WORK {
                let per_trial = (total - exact_cost) / (trials as f64 * prepared.len() as f64);
                let suggest = (((MAX_WORK - exact_cost) / (per_trial * prepared.len() as f64))
                    .floor() as u64)
                    .max(1);
                return Err(format!(
                    "this sweep would take about {total:.0e} steps; use --trials {suggest} or fewer (per point)"
                ));
            }
            let pts = study::run_sweep(&prepared, trials, seed);
            let text = if a.json {
                sweep_json(&pts, &name, seed) + "\n"
            } else if a.csv {
                let mut s = format!("{name},exact,estimate,counted,ci_low,ci_high\n");
                for p in &pts {
                    let (lo, hi) = p
                        .outcome
                        .ci95
                        .map_or((String::new(), String::new()), |(l, h)| {
                            (l.to_string(), h.to_string())
                        });
                    // No estimate where nothing counted: an empty field, like the interval.
                    let est = if p.outcome.counted == 0 {
                        String::new()
                    } else {
                        p.outcome.estimate.to_string()
                    };
                    s += &format!(
                        "{},{},{est},{},{lo},{hi}\n",
                        p.value, p.outcome.theory, p.outcome.counted
                    );
                }
                s
            } else {
                sweep_report(&pts, &name, &opt)
                    + &match lang {
                        Lang::En => {
                            format!("seed {seed} (each point gets its own stream from it)\n")
                        }
                        Lang::Ja => format!("seed {seed}(各点はここから導いた別々の乱数列)\n"),
                    }
            };
            write(&text)?;
            Ok(ExitCode::SUCCESS)
        }
        "coverage" => {
            let target = a
                .target
                .clone()
                .ok_or("coverage needs a paradox: probsim coverage monty-hall")?;
            if target == "benford" {
                return Err("coverage needs a simulated paradox, not benford".into());
            }
            let p = paradox(&a, &target)?;
            let trials = a.trials.unwrap_or(1_000);
            affordable(&p, trials, a.runs)?;
            let runs = study::coverage(&p, a.runs, trials, seed);
            if runs.is_empty() {
                return Err(format!(
                    "no run could be judged: {} (an average needs at least 30 trials per run, and a \
                     conditional question needs some trials that match)",
                    "too few trials per run"
                ));
            }
            let text = if a.json {
                coverage_json(&p, &runs, a.runs, trials) + "\n"
            } else if a.csv {
                let mut s = String::from("seed,estimate,ci_low,ci_high,hit\n");
                for r in &runs {
                    s += &format!(
                        "{},{},{},{},{}\n",
                        r.seed, r.estimate, r.interval.0, r.interval.1, r.hit
                    );
                }
                s
            } else {
                coverage_report(&p, &runs, a.runs, trials, &opt)
            };
            write(&text)?;
            Ok(ExitCode::SUCCESS)
        }
        "all" => {
            let trials = a.trials.unwrap_or(100_000);
            let mut outcomes = Vec::new();
            for (i, c) in SIMS.iter().enumerate() {
                // Each paradox gets its own random stream, derived from the seed.
                let p = paradox(&a, c)?;
                affordable(&p, trials, 1)?;
                outcomes.push(simulate(&p, trials, derived_seed(seed, i as u64), points));
            }
            let src = Source::PowersOfTwo(trials);
            let analysis = benford::analyze(src.counts(1), 1);
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
                            probsim::output::grouped(trials)
                        ),
                        Lang::Ja => format!(
                            "\n各{}回、seed {seed}(各パラドックスはここから導いた別々の乱数列。--jsonに各seedを出力)\n",
                            probsim::output::grouped(trials)
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
            let trials = a.trials.unwrap_or(100_000);
            affordable(&p, trials, 1)?;
            // --explain replays the first trials of the same seed with their
            // stories; the simulation below repeats them and goes on.
            let mut stories = Vec::new();
            if a.explain > 0 {
                let mut rng = Rng::new(seed);
                let mut scratch = Vec::new();
                for _ in 0..(a.explain as u64).min(trials) {
                    stories.push(p.explain(&mut rng, &mut scratch, lang));
                }
            }
            let o: Outcome = simulate(&p, trials, seed, points);
            if o.counted == 0 {
                return Err("no trial counted; run more trials".into());
            }
            if let Some(path) = &a.svg {
                let title = format!(
                    "{} ({}, seed {seed})",
                    p.texts(lang).title,
                    p.texts(lang).quantity
                );
                std::fs::write(path, export::svg(&o, &title, lang))
                    .map_err(|e| format!("cannot write {path:?}: {e}"))?;
            }
            let text = if a.json {
                outcome_json(&o, &stories) + "\n"
            } else if a.csv {
                export::csv(&o)
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
