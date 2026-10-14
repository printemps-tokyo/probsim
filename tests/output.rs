use probsim::output::{
    chart, compact, grouped, json_num, json_str, outcome_json, report, text_width, wrap,
    TextOptions,
};
use probsim::paradox::{Lang, Measure, Paradox};
use probsim::sim::{simulate, Checkpoint};

#[test]
fn numbers() {
    assert_eq!(grouped(0), "0");
    assert_eq!(grouped(1_234_567), "1,234,567");
    assert_eq!(compact(1), "1");
    assert_eq!(compact(1_000), "1k");
    assert_eq!(compact(316), "316");
    assert_eq!(compact(2_500_000), "2.5M");
    assert_eq!(json_num(f64::NAN), "null");
    assert_eq!(json_str("a\"b\\c\n\u{1}"), "\"a\\\"b\\\\c\\n\\u0001\"");
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
}

#[test]
fn chart_has_the_requested_size_and_a_theory_line() {
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
    let lines = chart(&pts, 2.0 / 3.0, Measure::Probability, 40, 9);
    assert_eq!(lines.len(), 9 + 2);
    assert!(lines
        .iter()
        .any(|l| l.contains("0.66667 |") && l.contains("---")));
    assert!(
        lines[0].contains('^'),
        "the wild first point is drawn at the edge"
    );
    assert!(lines.last().unwrap().contains("10k"));
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
    assert!(en.contains("exact answer is inside the 95% interval") || en.contains("outside"));
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
    let j = outcome_json(&o);
    for key in [
        "\"paradox\":\"secretary\"",
        "\"params\":{\"candidates\":100,\"skip\":37}",
        "\"seed\":2",
        "\"trials\":1000",
        "\"ci95\":[",
        "\"checkpoints\":[[1,",
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
    assert!(outcome_json(&m).contains("\"host\":\"random\""));
}
