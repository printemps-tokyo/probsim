use std::process::Command;

fn probsim(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_probsim"))
        .args(args)
        .env("LANG", "C")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
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
    assert!(out.contains("Usage: probsim <paradox> [options]"));
    assert_eq!(
        probsim(&["-V"]).1,
        format!("probsim {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(probsim(&["list"]).1.lines().count(), 6);
}

#[test]
fn usage_errors_exit_2() {
    for args in [
        &["nope"][..],
        &[][..],
        &["birthday", "--doors", "4"][..],
        &["birthday", "--people", "0"][..],
        &["monty-hall", "--trials", "0"][..],
        &["monty-hall", "--random-host=false"][..],
        &["monty-hall", "--json=no"][..],
        &["monty-hall", "--seed", "9007199254740992"][..],
        &["secretary", "--candidates", "10", "--skip", "10"][..],
        &["buffon", "--length", "3", "--spacing", "2"][..],
        &["buffon", "--length", "1e-300", "--spacing", "1e300"][..],
        &["benford", "--source", "primes"][..],
        &["benford", "--file", "/nonexistent/x.csv"][..],
        &["benford", "--column", "2"][..],
        &["list", "--json"][..],
        &["all", "--explain", "3"][..],
        &["birthday", "--lang", "fr"][..],
        &["birthday", "--bogus"][..],
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
    assert!(out.contains("  Game 3: "));
    let (_, ja, _) = probsim(&[
        "secretary",
        "--explain",
        "1",
        "--trials",
        "10",
        "--seed",
        "4",
        "--lang",
        "ja",
        "--no-chart",
    ]);
    assert!(
        ja.starts_with("最初の1回を一つずつ:\n  1回目: 最初の37人で基準はスコア"),
        "{ja}"
    );
    let (_, j, _) = probsim(&[
        "coupon",
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
fn json_output() {
    let (code, out, _) = probsim(&[
        "coupon", "--items", "5", "--trials", "2000", "--seed", "1", "--json",
    ]);
    assert_eq!(code, 0);
    assert!(out.starts_with("{\"paradox\":\"coupon\",\"params\":{\"items\":5}"));
    let (_, all, _) = probsim(&["all", "--trials", "2000", "--seed", "1", "--json"]);
    assert!(all.starts_with("{\"seed\":1,\"results\":[{\"paradox\":\"monty-hall\""));
    assert!(all.contains("\"benford\":{\"paradox\":\"benford\",\"source\":\"powers2\""));
}

#[test]
fn check_passes_on_good_runs_and_fails_on_data_that_is_not_benford() {
    assert_eq!(
        probsim(&["all", "--trials", "20000", "--seed", "8", "--check"]).0,
        0
    );
    // A certain outcome never fails the check.
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
    let flat_file = temp("flat.txt", text.as_bytes());
    let (code, out, _) = probsim(&["benford", "--file", flat_file.to_str().unwrap(), "--check"]);
    assert_eq!(code, 1, "{out}");
    assert!(flat(&out).contains("differs from Benford's law"), "{out}");
    assert_eq!(
        probsim(&["benford", "--file", flat_file.to_str().unwrap()]).0,
        0,
        "without --check"
    );
}

#[test]
fn check_refuses_to_judge_too_few_trials() {
    let (code, _, err) = probsim(&[
        "coupon",
        "--items",
        "2",
        "--trials",
        "2",
        "--seed",
        "2",
        "--check",
        "--no-chart",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("too few trials"), "{err}");
    let few = temp("few.txt", b"1 2 3");
    let (code, out, err) = probsim(&["benford", "--file", few.to_str().unwrap(), "--check"]);
    assert_eq!(code, 2, "{err}");
    assert!(
        flat(&out).contains("too few for the chi-square test"),
        "{out}"
    );
}

#[test]
fn benford_reads_csv_columns_and_any_encoding() {
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
    let (_, text, _) = probsim(&["benford", "--file", csv.to_str().unwrap()]);
    assert!(
        flat(&text).contains("use --column to read one column"),
        "{text}"
    );
    // Shift_JIS bytes around the numbers are not UTF-8; the numbers still count.
    let sjis = temp("sjis.csv", b"\x94\x84\x8f\xe3,123\n\x94\x84\x8f\xe3,456\n");
    let (code, out, err) = probsim(&["benford", "--file", sjis.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("\"total\":2"), "{out}");
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
    assert!(out.contains("最初の37人(37.0%)は見送り"));
}
