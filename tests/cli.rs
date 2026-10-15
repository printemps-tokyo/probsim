use std::io::Write;
use std::process::{Command, Stdio};

fn run(args: &[&str], stdin: Option<&[u8]>) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_probsim"))
        .args(args)
        .env("LANG", "C")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(bytes) = stdin {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn probsim(args: &[&str]) -> (i32, String, String) {
    run(args, None)
}

fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("probsim-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn help_version_and_list() {
    let (code, out, _) = probsim(&["--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("probsim sweep <paradox> --vary NAME"));
    assert_eq!(
        probsim(&["-V"]).1,
        format!("probsim {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(probsim(&["list"]).1.lines().count(), 10);
}

#[test]
fn usage_errors_exit_2() {
    for args in [
        &["nope"][..],
        &[][..],
        &["birthday", "--doors", "4"][..],
        &["monty-hall", "--random-host=false"][..],
        &["monty-hall", "--seed", "9007199254740992"][..],
        &["secretary", "--candidates", "10", "--skip", "10"][..],
        &["buffon", "--length", "3", "--spacing", "2"][..],
        &["benford", "--file", "/nonexistent/x.csv"][..],
        &["benford", "--column", "2"][..],
        &["benford", "--digits", "3"][..],
        &["list", "--json"][..],
        &["all", "--explain", "3"][..],
        &["base-rate", "--prevalence", "2"][..],
        &["base-rate", "--prevalence", "0", "--specificity", "1"][..],
        &["penney", "--first", "HHT", "--second", "TH"][..],
        &["penney", "--first", "HHT", "--second", "HHT"][..],
        &["penney", "--first", "HXT"][..],
        &["bertrand", "--method", "diameter"][..],
        &["coupon", "--weights", "1,0,1"][..],
        &["coupon", "--weights", "1,1", "--items", "3"][..],
        &["coupon", "--weights", "1,2", "--weights-file", "x"][..],
        &["coupon", "--budget", "100000000", "--items", "10000"][..],
        &[
            "sweep", "birthday", "--vary", "doors", "--from", "1", "--to", "5",
        ][..],
        &["sweep", "birthday", "--vary", "people", "--from", "1"][..],
        &[
            "sweep", "penney", "--vary", "first", "--from", "1", "--to", "2",
        ][..],
        &["coverage", "benford"][..],
        &["play", "birthday"][..],
        &["monty-hall", "--json", "--csv"][..],
        &[
            "sweep", "birthday", "--vary", "people", "--from", "1", "--to", "5", "--check",
        ][..],
    ] {
        let (code, _, err) = probsim(args);
        assert_eq!(code, 2, "{args:?}: {err}");
        assert!(err.starts_with("probsim: "), "{err}");
    }
}

#[test]
fn a_run_prints_the_seed_and_is_repeatable() {
    let a = probsim(&["birthday", "--trials", "5000", "--seed", "11", "--no-chart"]);
    assert_eq!(a.0, 0);
    assert!(a.1.contains("seed 11"));
    assert_eq!(
        a.1,
        probsim(&["birthday", "--trials=5000", "--seed=11", "--no-chart"]).1
    );
}

#[test]
fn new_paradoxes_run() {
    for args in [
        &["base-rate", "--prevalence", "1%", "--trials", "20000"][..],
        &["penney", "--first", "HTH", "--trials", "20000"][..],
        &["boy-girl", "--trials", "20000"][..],
        &["boy-girl", "--any-day", "--trials", "20000"][..],
        &["bertrand", "--method", "radius", "--trials", "20000"][..],
        &["coupon", "--weights", "1,1,1,0.2", "--trials", "20000"][..],
        &["gacha", "--budget", "40", "--trials", "20000"][..],
    ] {
        let mut a = args.to_vec();
        a.extend(["--seed", "3", "--check", "--no-chart"]);
        let (code, out, err) = probsim(&a);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
    }
    let days = temp("days.txt", "1\n".repeat(364).as_bytes());
    let mut weights = std::fs::read_to_string(&days).unwrap();
    weights.push_str("5\n");
    std::fs::write(&days, weights).unwrap();
    let (code, out, _) = probsim(&[
        "birthday",
        "--day-weights",
        days.to_str().unwrap(),
        "--json",
        "--trials",
        "2000",
    ]);
    assert_eq!(code, 0);
    assert!(
        out.contains("\"days\":365,\"dayWeights\":\"given\""),
        "{out}"
    );
}

#[test]
fn explain_tells_the_first_trials() {
    let (code, out, _) = probsim(&[
        "monty-hall",
        "--explain",
        "3",
        "--trials",
        "100",
        "--seed",
        "4",
        "--no-chart",
    ]);
    assert_eq!(code, 0);
    assert!(
        out.starts_with("The first 3 games, step by step:\n  Game 1: car behind door "),
        "{out}"
    );
    let (_, j, _) = probsim(&[
        "penney",
        "--explain",
        "2",
        "--trials",
        "100",
        "--seed",
        "4",
        "--json",
    ]);
    assert!(j.contains("\"explained\":[{\"result\":"), "{j}");
}

#[test]
fn sweep_coverage_csv_and_svg() {
    let (code, out, _) = probsim(&[
        "sweep", "birthday", "--vary", "people", "--from", "10", "--to", "40", "--steps", "4",
        "--trials", "2000", "--seed", "1",
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("as people changes") && out.contains("seed 1"));
    let (_, csv, _) = probsim(&[
        "sweep",
        "secretary",
        "--vary",
        "skip",
        "--from",
        "0",
        "--to",
        "90",
        "--steps",
        "4",
        "--trials",
        "1000",
        "--csv",
    ]);
    assert!(
        csv.starts_with("skip,exact,estimate,counted,ci_low,ci_high\n0,0.01,"),
        "{csv}"
    );
    assert_eq!(csv.lines().count(), 5);
    let (code, cov, _) = probsim(&[
        "coverage",
        "monty-hall",
        "--runs",
        "20",
        "--trials",
        "200",
        "--seed",
        "2",
    ]);
    assert_eq!(code, 0);
    assert!(flat(&cov).contains("of 20 intervals"));
    let (_, ccsv, _) = probsim(&[
        "coverage", "coupon", "--runs", "5", "--trials", "50", "--csv",
    ]);
    assert_eq!(ccsv.lines().count(), 6);
    let svg = std::env::temp_dir().join(format!("probsim-{}-chart.svg", std::process::id()));
    let (code, _, _) = probsim(&[
        "buffon",
        "--trials",
        "2000",
        "--svg",
        svg.to_str().unwrap(),
        "--no-chart",
    ]);
    assert_eq!(code, 0);
    assert!(std::fs::read_to_string(&svg).unwrap().contains("<polyline"));
    std::fs::remove_file(&svg).unwrap();
    let (_, rcsv, _) = probsim(&["monty-hall", "--trials", "100", "--csv"]);
    assert!(rcsv.starts_with("n,estimate,exact,band_low,band_high\n"));
}

#[test]
fn play_reads_answers_from_standard_input() {
    let (code, out, _) = run(
        &["play", "monty-hall", "--seed", "4"],
        Some(b"1\ny\n2\nn\nnine\n3\nmaybe\ny\nq\n"),
    );
    assert_eq!(code, 0);
    assert!(out.contains("Monty Hall with 3 doors"));
    assert!(out.contains("Type a number from 1 to 3."));
    assert!(
        out.lines().last().unwrap().starts_with("Switching won "),
        "{out}"
    );
    let (code, _, _) = run(&["play", "monty-hall"], Some(b""));
    assert_eq!(code, 0, "the end of input ends the game");
}

#[test]
fn check_passes_on_good_runs_and_fails_on_data_that_is_not_benford() {
    assert_eq!(
        probsim(&["all", "--trials", "20000", "--seed", "8", "--check"]).0,
        0
    );
    assert_eq!(
        probsim(&[
            "birthday",
            "--people",
            "2",
            "--days",
            "1",
            "--trials",
            "1000",
            "--check",
            "--no-chart"
        ])
        .0,
        0
    );
    let text: String = (0..900).map(|i| format!("{} ", i % 9 + 1)).collect();
    let f = temp("flat.txt", text.as_bytes());
    let (code, out, _) = probsim(&["benford", "--file", f.to_str().unwrap(), "--check"]);
    assert_eq!(code, 1, "{out}");
    assert!(
        flat(&out).contains("differs from Benford's law") && flat(&out).contains("nonconformity")
    );
    let (code, _, err) = probsim(&[
        "coupon",
        "--items",
        "2",
        "--trials",
        "2",
        "--check",
        "--no-chart",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("too few trials"), "{err}");
}

#[test]
fn benford_from_files_columns_and_standard_input() {
    let csv = temp("sales.csv", b"id,amount\n1,250\n2,\"1,300\"\n3,0\n");
    let (code, out, _) = probsim(&[
        "benford",
        "--file",
        csv.to_str().unwrap(),
        "--column",
        "2",
        "--json",
    ]);
    assert_eq!(code, 0);
    assert!(
        out.contains("\"column\":2,\"numbersFound\":3,\"zeros\":1"),
        "{out}"
    );
    let numbers: String = (1..=3000).map(|i| format!("{}\n", i * 7)).collect();
    let (code, out, _) = run(
        &["benford", "--file", "-", "--json"],
        Some(numbers.as_bytes()),
    );
    assert_eq!(code, 0);
    assert!(
        out.contains("\"file\":\"-\"") && out.contains("\"total\":3000"),
        "{out}"
    );
    let (_, text, _) = run(&["benford", "--file", "-"], Some(numbers.as_bytes()));
    assert!(text.contains("standard input"));
    let (code, two, _) = probsim(&["benford", "--digits", "2", "--trials", "20000", "--json"]);
    assert_eq!(code, 0);
    assert!(two.contains("\"firstDigits\":2") && two.contains("\"df\":89"));
}

#[test]
fn japanese_output() {
    let (_, out, _) = probsim(&[
        "secretary",
        "--trials",
        "3000",
        "--seed",
        "2",
        "--lang",
        "ja",
        "--no-chart",
    ]);
    assert!(out.starts_with("秘書問題(100人の候補者)"));
    let (_, bg, _) = probsim(&["boy-girl", "--trials", "3000", "--lang", "ja", "--no-chart"]);
    assert!(bg.starts_with("男女の謎(火曜日生まれの男の子)"));
}

#[test]
fn weights_lists_are_read_field_by_field() {
    let (code, out, _) = probsim(&[
        "gacha",
        "--weights",
        "500,300,150,50",
        "--trials",
        "100",
        "--json",
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("\"items\":4"), "{out}");
    let (_, b, _) = probsim(&[
        "gacha",
        "--weights",
        "1,100",
        "--budget",
        "2",
        "--trials",
        "100",
        "--json",
    ]);
    assert!(b.contains("\"items\":2"), "{b}");
    for bad in ["1,NaN", "1,-.5", "1,x"] {
        assert_eq!(probsim(&["gacha", "--weights", bad]).0, 2, "{bad}");
    }
}

#[test]
fn guards_against_runs_that_would_take_too_long() {
    let (code, _, err) = probsim(&["gacha", "--weights", "1,1e-6"]);
    assert_eq!(code, 2);
    assert!(
        err.contains("use --trials") && err.contains("or fewer"),
        "{err}"
    );
    let (code, _, err) = probsim(&[
        "coverage",
        "base-rate",
        "--prevalence",
        "1e-100",
        "--specificity",
        "1",
        "--runs",
        "3",
        "--trials",
        "1",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("no run could be judged"), "{err}");
}

#[test]
fn a_sweep_reads_standard_input_once() {
    let (code, out, err) = run(
        &[
            "sweep",
            "birthday",
            "--vary",
            "people",
            "--from",
            "1",
            "--to",
            "2",
            "--day-weights",
            "-",
            "--trials",
            "10",
            "--json",
        ],
        Some(b"1\n2\n"),
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("\"value\":2"), "{out}");
}

#[test]
fn play_explains_bad_answers_and_prints_the_seed() {
    let (_, out, _) = run(
        &["play", "monty-hall", "--seed", "4"],
        Some(b"1\nmaybe\ny\nq\n"),
    );
    assert!(
        out.contains("(seed 4)") && out.contains("Type y or n."),
        "{out}"
    );
}

#[test]
fn round_two_edge_cases() {
    // A huge budget costs only the draws it takes.
    assert_eq!(
        probsim(&[
            "gacha",
            "--items",
            "1",
            "--budget",
            "100000000",
            "--trials",
            "1000",
            "--json"
        ])
        .0,
        0
    );
    // Nothing counted: the JSON estimate is null.
    let (_, out, _) = probsim(&[
        "sweep",
        "base-rate",
        "--vary",
        "prevalence",
        "--from",
        "1e-100",
        "--to",
        "2e-100",
        "--specificity",
        "1",
        "--trials",
        "1",
        "--steps",
        "2",
        "--json",
    ]);
    assert!(out.contains("\"counted\":0,\"estimate\":null"), "{out}");
    // Empty or invalid sweep ranges are errors.
    for range in [["NaN", "10"], ["1.1", "1.2"]] {
        let (code, _, err) = probsim(&[
            "sweep", "birthday", "--vary", "people", "--from", range[0], "--to", range[1],
        ]);
        assert_eq!(code, 2, "{range:?}: {err}");
    }
    // The suggested --trials is per run and can be pasted.
    let (_, _, err) = probsim(&[
        "coverage",
        "gacha",
        "--weights",
        "1,1e-6",
        "--runs",
        "100",
        "--trials",
        "1000",
    ]);
    let suggested: u64 = err
        .split("--trials ")
        .nth(1)
        .unwrap()
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(suggested < 1000, "{err}");
}

#[test]
fn a_sweep_is_costed_as_a_whole() {
    let (code, _, err) = probsim(&[
        "sweep",
        "secretary",
        "--vary",
        "skip",
        "--from",
        "0",
        "--to",
        "99999",
        "--steps",
        "200",
        "--candidates",
        "100000",
        "--trials",
        "200000",
    ]);
    assert_eq!(code, 2);
    assert!(
        err.contains("this sweep would take about") && err.contains("per point"),
        "{err}"
    );
}

#[test]
fn the_exact_answer_counts_toward_the_cost() {
    let (code, _, err) = probsim(&[
        "sweep", "coupon", "--vary", "budget", "--from", "40000", "--to", "50000", "--steps",
        "200", "--items", "10000", "--trials", "1", "--json",
    ]);
    assert_eq!(code, 2);
    assert!(
        err.contains("the exact answers alone") && err.contains("--steps"),
        "{err}"
    );
}
