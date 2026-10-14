use std::process::Command;

fn probsim(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_probsim"))
        .args(args)
        .env("LANG", "C")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
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
    let (_, list, _) = probsim(&["list"]);
    assert_eq!(list.lines().count(), 6);
}

#[test]
fn usage_errors_exit_2() {
    for args in [
        &["nope"][..],
        &[][..],
        &["birthday", "--doors", "4"][..],
        &["birthday", "--people", "0"][..],
        &["monty-hall", "--trials", "0"][..],
        &["secretary", "--candidates", "10", "--skip", "10"][..],
        &["buffon", "--length", "3", "--spacing", "2"][..],
        &["benford", "--source", "primes"][..],
        &["benford", "--file", "/nonexistent/x.csv"][..],
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
    let dir = std::env::temp_dir().join(format!("probsim-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let flat = dir.join("flat.csv");
    let text: String = (0..900).map(|i| format!("{},", i % 9 + 1)).collect();
    std::fs::write(&flat, text).unwrap();
    let (code, out, _) = probsim(&["benford", "--file", flat.to_str().unwrap(), "--check"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("differs from Benford's"), "{out}");
    let (code, _, _) = probsim(&["benford", "--file", flat.to_str().unwrap()]);
    assert_eq!(code, 0, "without --check a result is not a failure");
    std::fs::remove_dir_all(&dir).unwrap();
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
