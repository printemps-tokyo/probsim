use probsim::output::{
    chart, compact, explain_block, grouped, json_num, json_str, outcome_json, report, text_width,
    wrap, TextOptions,
};
use probsim::paradox::{pct, Lang, Measure, Paradox};
use probsim::sim::{simulate, Checkpoint};

#[test]
fn numbers() {
    assert_eq!(grouped(0), "0");
    assert_eq!(grouped(123), "123");
    assert_eq!(grouped(123_456), "123,456");
    assert_eq!(grouped(1_234_567), "1,234,567");
    assert_eq!(compact(1), "1");
    assert_eq!(compact(1_000), "1k");
    assert_eq!(compact(2_500_000), "2.5M");
    assert_eq!(json_num(f64::NAN), "null");
    assert_eq!(json_str("a\"b\\c\n\u{1}"), "\"a\\\"b\\\\c\\n\\u0001\"");
    assert_eq!(pct(0.5), "50.0%");
    assert_eq!(pct(1.0 / 100_000.0), "0.0010%");
    assert_eq!(pct(0.0), "0.0%");
}

#[test]
fn wrapping_english_and_japanese() {
    let w = wrap("one two three four five six seven eight nine ten", 15, 2);
    for line in w.lines() {
        assert!(text_width(line) <= 17, "{line:?}");
    }
    assert!(w.lines().skip(1).all(|l| l.starts_with("  ")));
    let ja = wrap(
        "最初の選択が当たっているのは1/3だけ。司会者は車のドアを開けないので、",
        20,
        0,
    );
    for line in ja.lines() {
        assert!(text_width(line) <= 21, "{line:?}");
        assert!(
            !line.starts_with('。') && !line.starts_with('、'),
            "{line:?}"
        );
    }
    // An ASCII run such as "1/2" is never split, even in Japanese.
    for width in 20..40 {
        let w = wrap(
            "司会者がたまたま車を開けなかったゲームに限ると、変えて当たるのは1/2です。",
            width,
            0,
        );
        assert!(w.contains("1/2"), "width {width}: {w}");
    }
}

#[test]
fn chart_has_the_requested_size_a_theory_line_and_a_band() {
    let pts: Vec<Checkpoint> = [
        (1, 1.0),
        (10, 0.7),
        (100, 0.68),
        (1000, 0.667),
        (10000, 0.6667),
    ]
    .iter()
    .map(|&(n, e)| Checkpoint { n, estimate: e })
    .collect();
    let lines = chart(
        &pts,
        2.0 / 3.0,
        Measure::Probability,
        40,
        9,
        Some((2.0f64 / 9.0).sqrt()),
    );
    assert_eq!(lines.len(), 9 + 2);
    assert!(lines
        .iter()
        .any(|l| l.contains("0.66667 |") && l.contains("---")));
    assert!(
        lines.iter().any(|l| l.contains('.')),
        "the 95% band is drawn"
    );
    assert!(lines.last().unwrap().contains("10k"));
}

#[test]
fn a_probability_axis_stays_within_0_and_1() {
    let pts: Vec<Checkpoint> = (1..=5)
        .map(|i| Checkpoint {
            n: 10u64.pow(i),
            estimate: 1.0,
        })
        .collect();
    let lines = chart(&pts, 1.0, Measure::Probability, 30, 7, Some(0.0));
    assert!(lines[0].starts_with("1.00000 |"), "{:?}", lines[0]);
}

#[test]
fn report_says_whether_the_exact_answer_is_inside() {
    let o = simulate(
        &Paradox::MontyHall {
            doors: 3,
            host_knows: true,
        },
        20_000,
        4,
        30,
    );
    let en = report(
        &o,
        &TextOptions {
            lang: Lang::En,
            chart: true,
            width: 40,
        },
    );
    assert!(en.contains("Switching wins 2/3 of the time"));
    assert!(en.contains("inside the 95% interval") || en.contains("outside the 95% interval"));
    assert!(en.contains("Convergence: switching wins"));
    let ja = report(
        &o,
        &TextOptions {
            lang: Lang::Ja,
            chart: false,
            width: 40,
        },
    );
    assert!(ja.contains("変えると2/3の確率"));
    assert!(!ja.contains("収束"));
    let b = simulate(
        &Paradox::Buffon {
            length: 1.0,
            spacing: 2.0,
        },
        20_000,
        4,
        30,
    );
    assert!(report(
        &b,
        &TextOptions {
            lang: Lang::En,
            chart: false,
            width: 40
        }
    )
    .contains("pi estimate 3."));
    let few = simulate(&Paradox::Coupon { items: 3 }, 5, 4, 30);
    assert!(report(
        &few,
        &TextOptions {
            lang: Lang::En,
            chart: false,
            width: 40
        }
    )
    .contains("too few trials"));
}

/// The text with line breaks and indentation folded into single spaces.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn edge_parameters_read_well() {
    let opt = TextOptions {
        lang: Lang::En,
        chart: false,
        width: 40,
    };
    let crowd = simulate(
        &Paradox::Birthday {
            people: 400,
            days: 365,
        },
        100,
        1,
        10,
    );
    let t = report(&crowd, &opt);
    assert!(
        flat(&t).contains("pigeonhole") && !t.contains("109.6%"),
        "{t}"
    );
    let one = simulate(
        &Paradox::Birthday {
            people: 1,
            days: 365,
        },
        100,
        1,
        10,
    );
    assert!(flat(&report(&one, &opt)).contains("a single person makes no pairs"));
    let tiny = simulate(
        &Paradox::Buffon {
            length: 1e-320,
            spacing: 1e-319,
        },
        100,
        1,
        10,
    );
    assert!(report(&tiny, &opt).lines().next().unwrap().len() < 80);
}

#[test]
fn explain_block_numbers_the_trials() {
    let p = Paradox::MontyHall {
        doors: 3,
        host_knows: true,
    };
    let stories = vec![(Some(1.0), "a".to_string()), (Some(0.0), "b".to_string())];
    let en = explain_block(&p, &stories, Lang::En);
    assert!(
        en.starts_with("The first 2 games, step by step:\n  Game 1: a\n  Game 2: b\n"),
        "{en}"
    );
    assert!(explain_block(&p, &stories, Lang::Ja).contains("1ゲーム目: a"));
}

#[test]
fn json_has_the_fields() {
    let o = simulate(
        &Paradox::Secretary {
            candidates: 100,
            skip: 37,
        },
        1_000,
        2,
        10,
    );
    let j = outcome_json(&o, &[]);
    for key in [
        "\"paradox\":\"secretary\"",
        "\"params\":{\"candidates\":100,\"skip\":37}",
        "\"seed\":2",
        "\"trials\":1000",
        "\"ci95\":[",
        "\"consistent95\":",
    ] {
        assert!(j.contains(key), "{key} in {j}");
    }
    let m = simulate(
        &Paradox::MontyHall {
            doors: 3,
            host_knows: false,
        },
        100,
        2,
        10,
    );
    assert!(outcome_json(&m, &[(None, "x".into())])
        .contains("\"explained\":[{\"result\":null,\"story\":\"x\"}]"));
    let few = simulate(&Paradox::Coupon { items: 2 }, 3, 2, 10);
    let fj = outcome_json(&few, &[]);
    assert!(
        fj.contains("\"ci95\":null") && fj.contains("\"consistent95\":null"),
        "{fj}"
    );
}
