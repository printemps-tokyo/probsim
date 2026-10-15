use probsim::export;
use probsim::output::{
    chart, compact, coverage_json, coverage_report, explain_block, grouped, json_num, json_str,
    outcome_json, report, sweep_json, sweep_report, text_width, wrap, TextOptions,
};
use probsim::paradox::{coins, pct, Chord, Lang, Measure, Paradox};
use probsim::sim::{simulate, Checkpoint};
use probsim::study;

fn en(chart: bool) -> TextOptions {
    TextOptions {
        lang: Lang::En,
        chart,
        width: 40,
    }
}

fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn numbers() {
    assert_eq!(grouped(0), "0");
    assert_eq!(grouped(123_456), "123,456");
    assert_eq!(grouped(1_234_567), "1,234,567");
    assert_eq!(compact(1_000), "1k");
    assert_eq!(compact(2_500_000), "2.5M");
    assert_eq!(json_num(f64::NAN), "null");
    assert_eq!(json_str("a\"b\\c\n\u{1}"), "\"a\\\"b\\\\c\\n\\u0001\"");
    assert_eq!(pct(0.5), "50.0%");
    assert_eq!(pct(1.0 / 100_000.0), "0.0010%");
}

#[test]
fn wrapping_english_and_japanese() {
    let w = wrap("one two three four five six seven eight nine ten", 15, 2);
    assert!(w.lines().all(|l| text_width(l) <= 17));
    assert!(w.lines().skip(1).all(|l| l.starts_with("  ")));
    for width in 20..40 {
        let w = wrap(
            "司会者がたまたま車を開けなかったゲームに限ると、変えて当たるのは1/2です。",
            width,
            0,
        );
        assert!(w.contains("1/2"), "width {width}: {w}");
        assert!(w
            .lines()
            .all(|l| !l.starts_with('。') && !l.starts_with('、')));
    }
}

#[test]
fn chart_has_a_theory_line_and_a_band_only_where_it_holds() {
    let pts = |v: &[(u64, f64)]| {
        v.iter()
            .map(|&(n, e)| Checkpoint { n, estimate: e })
            .collect::<Vec<_>>()
    };
    let plot = |l: &String| {
        l.split_once('|')
            .map_or(String::new(), |(_, p)| p.to_string())
    };
    let lines = chart(
        &pts(&[
            (1, 1.0),
            (10, 0.7),
            (100, 0.68),
            (1000, 0.667),
            (10000, 0.6667),
        ]),
        2.0 / 3.0,
        Measure::Probability,
        40,
        9,
        Some((2.0f64 / 9.0).sqrt()),
    );
    assert_eq!(lines.len(), 11);
    assert!(lines
        .iter()
        .any(|l| l.contains("0.66667 |") && l.contains("---")));
    assert!(lines.iter().take(9).any(|l| plot(l).contains('.')));
    let rare = chart(
        &pts(&[(1, 0.0), (10, 0.0), (100, 0.01)]),
        0.001,
        Measure::Probability,
        30,
        9,
        Some(0.0316),
    );
    assert!(
        rare.iter().take(9).all(|l| !plot(l).contains('.')),
        "{rare:#?}"
    );
    let certain = chart(
        &pts(&[(10, 1.0), (100, 1.0)]),
        1.0,
        Measure::Probability,
        30,
        7,
        Some(0.0),
    );
    assert!(
        certain[0].starts_with("1.00000 |"),
        "a probability axis stays within [0, 1]"
    );
}

#[test]
fn reports_read_well() {
    let o = simulate(
        &Paradox::MontyHall {
            doors: 3,
            host_knows: true,
        },
        20_000,
        4,
        30,
    );
    let t = report(&o, &en(true));
    assert!(
        t.contains("Switching wins 2/3 of the time") && t.contains("Convergence: switching wins")
    );
    assert!(report(
        &o,
        &TextOptions {
            lang: Lang::Ja,
            chart: false,
            width: 40
        }
    )
    .contains("変えると2/3の確率"));
    let b = simulate(
        &Paradox::Buffon {
            length: 1.0,
            spacing: 2.0,
        },
        20_000,
        4,
        30,
    );
    assert!(report(&b, &en(false)).contains("pi estimate 3."));
    let few = simulate(
        &Paradox::Coupon {
            items: 3,
            weights: None,
            budget: None,
        },
        5,
        4,
        30,
    );
    assert!(report(&few, &en(false)).contains("too few trials"));
    let crowd = simulate(
        &Paradox::Birthday {
            people: 400,
            days: 365,
            weights: None,
        },
        100,
        1,
        10,
    );
    assert!(flat(&report(&crowd, &en(false))).contains("pigeonhole"));
    let g = simulate(
        &Paradox::Coupon {
            items: 10,
            weights: None,
            budget: None,
        },
        50_000,
        1,
        10,
    );
    assert!(flat(&report(&g, &en(false))).contains("Half of the collectors were done within"));
    assert!(flat(&report(&g, &en(false))).contains("(exact 27)"));
    let br = simulate(
        &Paradox::BaseRate {
            prevalence: 0.01,
            sensitivity: 0.99,
            specificity: 0.95,
        },
        20_000,
        1,
        10,
    );
    let brt = flat(&report(&br, &en(false)));
    assert!(
        brt.contains("Only 16.7%") && brt.contains("tested negative were not counted"),
        "{brt}"
    );
    let pg = simulate(
        &Paradox::Penney {
            first: coins("HHH").unwrap(),
            second: coins("THH").unwrap(),
        },
        2_000,
        1,
        10,
    );
    assert!(flat(&report(&pg, &en(false))).contains("wins 87.5% of the time (7/8)"));
    let bg = simulate(&Paradox::BoyGirl { tuesday: true }, 20_000, 1, 10);
    assert!(flat(&report(&bg, &en(false))).contains("families without such a boy were not counted"));
    let bt = simulate(
        &Paradox::Bertrand {
            method: Chord::Midpoint,
        },
        2_000,
        1,
        10,
    );
    assert!(flat(&report(
        &bt,
        &TextOptions {
            lang: Lang::Ja,
            chart: false,
            width: 40
        }
    ))
    .contains("この引き方なら1/4"));
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
    assert!(explain_block(&p, &stories[..1], Lang::En).starts_with("The first game, step by step:"));
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
        "\"ci95\":[",
    ] {
        assert!(j.contains(key), "{key} in {j}");
    }
    let few = simulate(
        &Paradox::Coupon {
            items: 2,
            weights: None,
            budget: None,
        },
        3,
        2,
        10,
    );
    let fj = outcome_json(&few, &[]);
    assert!(
        fj.contains("\"ci95\":null") && fj.contains("\"consistent95\":null"),
        "{fj}"
    );
    let pg = simulate(
        &Paradox::Penney {
            first: coins("HHT").unwrap(),
            second: coins("THH").unwrap(),
        },
        100,
        2,
        10,
    );
    assert!(outcome_json(&pg, &[]).contains("\"params\":{\"first\":\"HHT\",\"second\":\"THH\"}"));
}

#[test]
fn sweep_and_coverage_output() {
    let pts = study::sweep(
        &[10.0, 30.0],
        |v| {
            Ok(Paradox::Birthday {
                people: v as u32,
                days: 365,
                weights: None,
            })
        },
        2_000,
        1,
    )
    .unwrap();
    let t = sweep_report(&pts, "people", &en(true));
    assert!(t.contains("birthday: a shared birthday as people changes"));
    assert!(t.contains("o exact   * simulated"));
    assert!(sweep_json(&pts, "people", 1)
        .starts_with("{\"sweep\":\"people\",\"seed\":1,\"points\":[{\"value\":10,"));
    let p = Paradox::MontyHall {
        doors: 3,
        host_knows: true,
    };
    let runs = study::coverage(&p, 30, 500, 1);
    let c = coverage_report(&p, &runs, 30, 500, &en(true));
    assert!(flat(&c).contains("intervals") && c.contains('|'));
    assert!(coverage_json(&p, &runs, 30, 500).contains("\"runs\":30,\"judged\":30"));
}

#[test]
fn csv_and_svg_export() {
    let o = simulate(
        &Paradox::MontyHall {
            doors: 3,
            host_knows: true,
        },
        5_000,
        1,
        30,
    );
    let csv = export::csv(&o);
    assert!(csv.starts_with("n,estimate,exact,band_low,band_high\n1,"));
    assert_eq!(csv.lines().count(), o.checkpoints.len() + 1);
    // The band appears once n p and n (1 - p) reach 5.
    assert!(csv
        .lines()
        .last()
        .unwrap()
        .split(',')
        .nth(3)
        .is_some_and(|v| !v.is_empty()));
    let svg = export::svg(&o, "a < b & \"c\"", Lang::En);
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("a &lt; b &amp; &quot;c&quot;"));
    assert!(
        svg.contains("<polygon") && svg.contains("<polyline") && svg.trim_end().ends_with("</svg>")
    );
    let c = simulate(
        &Paradox::Coupon {
            items: 10,
            weights: None,
            budget: None,
        },
        5_000,
        1,
        30,
    );
    assert!(
        export::svg(&c, "x", Lang::En).contains("<polygon"),
        "the coupon band (exact variance) is drawn"
    );
}

#[test]
fn texts_follow_the_numbers() {
    let opt = en(false);
    // A common disease: the positives are mostly true, and the text says so.
    let common = simulate(
        &Paradox::BaseRate {
            prevalence: 0.5,
            sensitivity: 0.99,
            specificity: 0.95,
        },
        2_000,
        1,
        10,
    );
    let t = flat(&report(&common, &opt));
    assert!(
        !t.contains("Only") && t.contains("usually right here"),
        "{t}"
    );
    // A very rare disease: enough people that the counts do not round to 0.
    let rare = simulate(
        &Paradox::BaseRate {
            prevalence: 1e-5,
            sensitivity: 0.99,
            specificity: 0.95,
        },
        2_000,
        1,
        10,
    );
    assert!(!flat(&report(&rare, &opt)).contains("about 0 sick"));
    // Two flips: no reply has an edge.
    let short = simulate(
        &Paradox::Penney {
            first: coins("HT").unwrap(),
            second: coins("TH").unwrap(),
        },
        1_000,
        1,
        10,
    );
    assert!(flat(&report(&short, &opt)).contains("no reply wins more than half the time"));
    // A budget below the number of items.
    let b = simulate(
        &Paradox::Coupon {
            items: 10,
            weights: None,
            budget: Some(5),
        },
        100,
        1,
        10,
    );
    assert!(flat(&report(&b, &opt)).contains("some items can never come up"));
}

#[test]
fn a_descending_sweep_spreads_across_the_chart() {
    let pts = study::sweep(
        &[5.0, 3.0, 1.0],
        |v| {
            Ok(Paradox::Birthday {
                people: v as u32,
                days: 30,
                weights: None,
            })
        },
        500,
        1,
    )
    .unwrap();
    let t = sweep_report(&pts, "people", &en(true));
    let plot: Vec<&str> = t.lines().filter(|l| l.contains(" |")).collect();
    let columns: std::collections::HashSet<usize> = plot
        .iter()
        .filter_map(|l| l.split_once('|').and_then(|(_, p)| p.find(['o', '@'])))
        .collect();
    assert!(columns.len() >= 2, "{t}");
}

#[test]
fn svg_labels_have_enough_digits_and_the_axis_follows_the_language() {
    let o = simulate(
        &Paradox::BaseRate {
            prevalence: 1e-5,
            sensitivity: 0.99,
            specificity: 0.95,
        },
        20_000,
        1,
        30,
    );
    let ja = export::svg(&o, "x", Lang::Ja);
    assert!(ja.contains("試行回数(対数目盛り)"));
    let labels: Vec<&str> = ja
        .lines()
        .filter(|l| l.contains("text-anchor=\"end\""))
        .collect();
    let mut texts: Vec<String> = labels
        .iter()
        .map(|l| {
            l.split('>')
                .nth(1)
                .unwrap()
                .split('<')
                .next()
                .unwrap()
                .to_string()
        })
        .collect();
    let n = texts.len();
    texts.dedup();
    assert_eq!(texts.len(), n, "labels are distinct: {texts:?}");
}
