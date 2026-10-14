//! Text and JSON output. Pure: everything returns a String.

use std::fmt::Write as _;

use crate::benford::{self, Analysis, Scan, Source, MIN_FOR_TEST};
use crate::paradox::{Lang, Measure, Paradox};
use crate::sim::{Checkpoint, Outcome};
use crate::stats::{MIN_FOR_MEAN, Z95};

/// Display width of a character: 2 for East Asian wide characters, else 1.
fn char_width(c: char) -> usize {
    let u = c as u32;
    let wide = matches!(u,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6);
    if wide {
        2
    } else {
        1
    }
}

pub fn text_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Pad on the right to `w` display columns.
pub fn pad_right(s: &str, w: usize) -> String {
    format!("{s}{}", " ".repeat(w.saturating_sub(text_width(s))))
}

/// Pad on the left to `w` display columns.
pub fn pad_left(s: &str, w: usize) -> String {
    format!("{}{s}", " ".repeat(w.saturating_sub(text_width(s))))
}

/// Wrap text into lines of `width` columns, each line after the first
/// indented by `indent` spaces (so the caller places the first). A run of
/// ASCII characters without spaces ("1/2", "log10(1") is never split, in
/// English or in Japanese; Japanese breaks anywhere else, but never right
/// before 。 or 、.
pub fn wrap(text: &str, width: usize, indent: usize) -> String {
    let width = width.max(10);
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut col = 0;
    for c in text.chars() {
        let w = char_width(c);
        if col + w > width && !matches!(c, '。' | '、' | ')' | '）') {
            if c == ' ' {
                lines.push(std::mem::take(&mut line));
                col = 0;
                continue;
            }
            // Carry the trailing ASCII word over, unless it is the whole line.
            let run_start = if c.is_ascii() {
                line.char_indices()
                    .rev()
                    .take_while(|(_, ch)| ch.is_ascii() && *ch != ' ')
                    .last()
                    .map(|(i, _)| i)
            } else {
                None
            };
            match run_start {
                Some(i) if i > 0 => {
                    let rest = line[i..].to_string();
                    line.truncate(i);
                    lines.push(std::mem::take(&mut line));
                    line = rest;
                    col = text_width(&line);
                }
                _ => {
                    lines.push(std::mem::take(&mut line));
                    col = 0;
                }
            }
        }
        line.push(c);
        col += w;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    let pad = " ".repeat(indent);
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                l.trim_end().to_string()
            } else {
                format!("{pad}{}", l.trim())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 1234567 -> "1,234,567".
pub fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut parts = Vec::new();
    let mut end = s.len();
    while end > 3 {
        parts.push(&s[end - 3..end]);
        end -= 3;
    }
    parts.push(&s[..end]);
    parts.reverse();
    parts.join(",")
}

/// 1 -> "1", 1000 -> "1k", 2500000 -> "2.5M".
pub fn compact(n: u64) -> String {
    let f = |v: f64, unit: &str| {
        if (v - v.round()).abs() < 1e-9 {
            format!("{}{unit}", v.round() as u64)
        } else {
            format!("{v:.1}{unit}")
        }
    };
    if n >= 1_000_000_000 {
        f(n as f64 / 1e9, "G")
    } else if n >= 1_000_000 {
        f(n as f64 / 1e6, "M")
    } else if n >= 1_000 {
        f(n as f64 / 1e3, "k")
    } else {
        n.to_string()
    }
}

fn value(m: Measure, x: f64) -> String {
    match m {
        Measure::Probability => format!("{x:.5}"),
        Measure::Mean => format!("{x:.2}"),
    }
}

/// The running estimate against the exact answer, on a log scale of trials.
/// With `sd` (the exact standard deviation of one trial), dots mark the band
/// exact +- 1.96 sd / sqrt(n), where by the normal approximation about 95% of
/// runs are at each n: it narrows like 1/sqrt(n).
pub fn chart(
    points: &[Checkpoint],
    theory: f64,
    m: Measure,
    width: usize,
    height: usize,
    sd: Option<f64>,
) -> Vec<String> {
    let (width, height) = (width.max(10), height.max(5));
    if points.is_empty() {
        return Vec::new();
    }
    let (n0, n1) = (points[0].n as f64, points[points.len() - 1].n as f64);
    // Scale to the later points; the first handful of trials can be wild and
    // are drawn at the edge instead.
    let settled: Vec<&Checkpoint> = points.iter().filter(|c| c.n >= 30).collect();
    let pool: Vec<&Checkpoint> = if settled.is_empty() {
        points.iter().collect()
    } else {
        settled
    };
    let spread = pool
        .iter()
        .map(|c| (c.estimate - theory).abs())
        .fold(0.0, f64::max);
    let floor = 1e-4 * theory.abs().max(1.0);
    let half = spread.max(floor) * 1.15;
    let (mut top, mut bottom) = (theory + half, theory - half);
    if m == Measure::Probability {
        // A probability axis stays within [0, 1].
        top = top.min(1.0);
        bottom = bottom.max(0.0);
        if top - bottom < 1e-9 {
            top = (theory + 1e-4).min(1.0);
            bottom = (theory - 1e-4).max(0.0);
        }
    }
    let row_of = |v: f64| -> usize {
        let r = ((top - v) / (top - bottom) * (height - 1) as f64).round();
        r.clamp(0.0, (height - 1) as f64) as usize
    };
    let col_of = |n: f64| -> usize {
        if n1 <= n0 {
            width - 1
        } else {
            ((((n.ln() - n0.ln()) / (n1.ln() - n0.ln())) * (width - 1) as f64).round() as usize)
                .min(width - 1)
        }
    };
    let n_at = |col: usize| -> f64 {
        if n1 <= n0 {
            n1
        } else {
            (n0.ln() + (n1.ln() - n0.ln()) * col as f64 / (width - 1) as f64).exp()
        }
    };
    let mut grid = vec![vec![' '; width]; height];
    if let Some(sd) = sd.filter(|s| s.is_finite() && *s > 0.0) {
        for (col, n) in (0..width).map(|c| (c, n_at(c))) {
            // The band is a normal approximation: drawn only where it holds
            // (n p and n (1 - p) at least 5 for a probability, 30 trials for
            // an average).
            let holds = match m {
                Measure::Probability => n * theory >= 5.0 && n * (1.0 - theory) >= 5.0,
                Measure::Mean => n >= MIN_FOR_MEAN as f64,
            };
            if !holds {
                continue;
            }
            let h = Z95 * sd / n.sqrt();
            for v in [theory + h, theory - h] {
                if v <= top && v >= bottom {
                    grid[row_of(v)][col] = '.';
                }
            }
        }
    }
    let theory_row = row_of(theory);
    for c in grid[theory_row].iter_mut() {
        *c = '-';
    }
    for p in points {
        let col = col_of(p.n as f64);
        let ch = if p.estimate > top {
            '^'
        } else if p.estimate < bottom {
            'v'
        } else {
            '*'
        };
        grid[row_of(p.estimate)][col] = ch;
    }
    let label = |r: usize| -> String {
        if r == 0 {
            value(m, top)
        } else if r == theory_row {
            value(m, theory)
        } else if r == height - 1 {
            value(m, bottom)
        } else {
            String::new()
        }
    };
    let lw = (0..height).map(|r| label(r).len()).max().unwrap_or(0);
    let mut out: Vec<String> = (0..height)
        .map(|r| {
            format!(
                "{:>lw$} |{}",
                label(r),
                grid[r].iter().collect::<String>().trim_end()
            )
        })
        .collect();
    out.push(format!("{:>lw$} +{}", "", "-".repeat(width)));
    let (a, b) = (compact(n0 as u64), compact(n1 as u64));
    let mid_n = (n0 * n1).sqrt().round() as u64;
    let mid = compact(mid_n);
    let mut axis = vec![' '; width + 2];
    let put = |axis: &mut Vec<char>, at: usize, s: &str| {
        let start = at.min(axis.len().saturating_sub(s.len()));
        for (i, ch) in s.chars().enumerate() {
            if let Some(slot) = axis.get_mut(start + i) {
                *slot = ch;
            }
        }
    };
    put(&mut axis, 1, &a);
    if width > a.len() + b.len() + mid.len() + 6 {
        put(
            &mut axis,
            1 + col_of(mid_n as f64).saturating_sub(mid.len() / 2),
            &mid,
        );
    }
    put(&mut axis, width + 1 - b.len().min(width), &b);
    out.push(format!(
        "{:>lw$} {}",
        "",
        axis.iter().collect::<String>().trim_end()
    ));
    out
}

struct Labels {
    intuition: &'static str,
    truth: &'static str,
    simulated: &'static str,
    why: &'static str,
    inside: &'static str,
    outside: &'static str,
    too_few: &'static str,
    convergence: &'static str,
    legend: &'static str,
}

fn labels(lang: Lang) -> Labels {
    match lang {
        Lang::En => Labels {
            intuition: "Intuition: ",
            truth: "Truth:     ",
            simulated: "Simulated: ",
            why: "Why:       ",
            inside: "the exact answer is inside the 95% interval",
            outside:
                "the exact answer is outside the 95% interval (with enough trials, this happens \
                      in about 1 run in 20)",
            too_few: "too few trials for an interval for an average (need at least 30)",
            convergence: "Convergence",
            legend: "- exact answer   * this run   . where about 95% of runs stay (normal approximation, \
                     drawn where it holds)",
        },
        Lang::Ja => Labels {
            intuition: "直感:      ",
            truth: "正解:      ",
            simulated: "結果:      ",
            why: "理由:      ",
            inside: "正解は95%信頼区間の内側です",
            outside: "正解が95%信頼区間の外側です(試行が十分多ければ、20回に1回ほどは起こります)",
            too_few: "平均の信頼区間を出すには試行が少なすぎます(30回以上必要)",
            convergence: "収束",
            legend: "- 正解   * 今回の結果   . 各時点で約95%の実行が収まる範囲(正規近似、近似が成り立つ範囲のみ)",
        },
    }
}

/// Options for text output.
pub struct TextOptions {
    pub lang: Lang,
    pub chart: bool,
    pub width: usize,
}

const WRAP: usize = 78;

fn field(label: &str, text: &str) -> String {
    let w = text_width(label) + 2;
    format!("  {label}{}", wrap(text, WRAP - w, w))
}

/// pi from a crossing rate, for Buffon's needle; None when nothing crossed.
fn pi_from(length: f64, spacing: f64, p: f64) -> Option<f64> {
    let r = length / spacing;
    (p > 0.0).then(|| 2.0 * r / p)
}

/// The stories of the first trials (--explain), numbered.
pub fn explain_block(p: &Paradox, stories: &[(Option<f64>, String)], lang: Lang) -> String {
    let t = p.texts(lang);
    let mut s = String::new();
    let heading = match lang {
        Lang::En if stories.len() == 1 => {
            format!("The first {}, step by step:", t.one.to_lowercase())
        }
        Lang::En => format!("The first {} {}, step by step:", stories.len(), t.unit),
        Lang::Ja => format!("最初の{}{}を一つずつ:", stories.len(), t.unit),
    };
    let _ = writeln!(s, "{heading}");
    for (i, (_, story)) in stories.iter().enumerate() {
        let label = match lang {
            Lang::En => format!("{} {}: ", t.one, i + 1),
            Lang::Ja => format!("{}{}: ", i + 1, t.one),
        };
        let w = text_width(&label) + 2;
        let _ = writeln!(s, "  {label}{}", wrap(story, WRAP - w, w));
    }
    s
}

/// The report for one paradox.
pub fn report(o: &Outcome, opt: &TextOptions) -> String {
    let t = o.paradox.texts(opt.lang);
    let l = labels(opt.lang);
    let m = o.paradox.measure();
    let mut s = String::new();
    let _ = writeln!(s, "{}\n", t.title);
    let _ = writeln!(s, "{}\n", wrap(&t.question, WRAP, 0));
    let _ = writeln!(s, "{}", field(l.intuition, &t.intuition));
    let _ = writeln!(s, "{}", field(l.truth, &t.truth));
    let unit = &t.unit;
    let ci = match o.ci95 {
        Some((lo, hi)) => match opt.lang {
            Lang::En => format!("  95% CI [{}, {}]", value(m, lo), value(m, hi)),
            Lang::Ja => format!("  95%信頼区間 [{}, {}]", value(m, lo), value(m, hi)),
        },
        None => String::new(),
    };
    let sim = match opt.lang {
        Lang::En => format!(
            "{}{ci}  ({} {unit}, seed {})",
            value(m, o.estimate),
            grouped(o.counted),
            o.seed
        ),
        Lang::Ja => format!(
            "{}{ci}({}{unit}、seed {})",
            value(m, o.estimate),
            grouped(o.counted),
            o.seed
        ),
    };
    let _ = writeln!(s, "{}", field(l.simulated, &sim));
    let pad = " ".repeat(text_width(l.simulated) + 2);
    let verdict = match o.consistent(Z95) {
        Some(true) => l.inside,
        Some(false) => l.outside,
        None => l.too_few,
    };
    let _ = writeln!(s, "{pad}{}", wrap(verdict, WRAP - pad.len(), pad.len()));
    if o.counted < o.attempted {
        let skipped = o.attempted - o.counted;
        let note = match opt.lang {
            Lang::En => format!(
                "{} games where the host revealed the car were not counted",
                grouped(skipped)
            ),
            Lang::Ja => format!(
                "司会者が車を開けてしまった{}回は数えていません",
                grouped(skipped)
            ),
        };
        let _ = writeln!(s, "{pad}{note}");
    }
    if let Paradox::Buffon { length, spacing } = o.paradox {
        let f = |x: Option<f64>| x.map_or("-".to_string(), |v| format!("{v:.5}"));
        let est = f(pi_from(length, spacing, o.estimate));
        let (lo, hi) = o.ci95.map_or(("-".into(), "-".into()), |(lo, hi)| {
            (
                f(pi_from(length, spacing, hi)),
                f(pi_from(length, spacing, lo)),
            )
        });
        let line = match opt.lang {
            Lang::En => format!("pi estimate {est}  95% CI [{lo}, {hi}]"),
            Lang::Ja => format!("円周率の推定値 {est}  95%信頼区間 [{lo}, {hi}]"),
        };
        let _ = writeln!(s, "{pad}{line}");
    }
    let _ = writeln!(s, "{}", field(l.why, &t.why));
    if opt.chart && o.checkpoints.len() > 1 {
        let title = match opt.lang {
            Lang::En => format!(
                "{}: {}, by number of {unit} (log scale)",
                l.convergence, t.quantity
            ),
            Lang::Ja => format!("{}: {}({}数、対数目盛り)", l.convergence, t.quantity, unit),
        };
        let _ = writeln!(s, "\n{title}");
        let sd = o.paradox.theory_variance().sqrt();
        for line in chart(&o.checkpoints, o.theory, m, opt.width, 11, Some(sd)) {
            let _ = writeln!(s, "  {line}");
        }
        let _ = writeln!(s, "  {}", l.legend);
    }
    s
}

/// Notes about what a file scan read or skipped.
fn scan_notes(scan: &Scan, column: Option<usize>, lang: Lang) -> Vec<String> {
    let mut notes = Vec::new();
    if scan.zeros > 0 {
        notes.push(match lang {
            Lang::En => format!(
                "{} zeros have no leading digit and were left out.",
                grouped(scan.zeros)
            ),
            Lang::Ja => format!(
                "0の{}個は先頭の数字がないため除きました。",
                grouped(scan.zeros)
            ),
        });
    }
    if scan.grouped > 0 && column.is_none() {
        notes.push(match lang {
            Lang::En => format!(
                "{} numbers were read with thousands commas (like 1,234). In a CSV file, \"1,250\" may be \
                 two columns instead; use --column to read one column.",
                grouped(scan.grouped)
            ),
            Lang::Ja => format!(
                "{}個の数値を桁区切りのカンマ付き(1,234など)として読みました。CSVファイルでは\"1,250\"が\
                 2つの列のこともあるので、--columnで列を指定してください。",
                grouped(scan.grouped)
            ),
        });
    }
    notes
}

/// The Benford report.
pub fn benford_report(source: &Source, a: &Analysis, opt: &TextOptions) -> String {
    let [title, question, intuition, truth, why] = benford::texts(source, opt.lang);
    let l = labels(opt.lang);
    let mut s = String::new();
    let _ = writeln!(s, "{title}\n");
    let _ = writeln!(s, "{}\n", wrap(&question, WRAP, 0));
    let _ = writeln!(s, "{}", field(l.intuition, &intuition));
    let _ = writeln!(s, "{}", field(l.truth, &truth));
    let _ = writeln!(s, "{}\n", field(l.why, &why));
    let (h_digit, h_exp, h_obs) = match opt.lang {
        Lang::En => ("digit", "Benford", "observed"),
        Lang::Ja => ("数字", "法則", "実測"),
    };
    let _ = writeln!(
        s,
        "  {}{}{}",
        pad_right(h_digit, 6),
        pad_left(h_exp, 9),
        pad_left(h_obs, 10)
    );
    let share = |d: usize| {
        if a.total == 0 {
            0.0
        } else {
            a.counts[d - 1] as f64 / a.total as f64
        }
    };
    // Bars fit the chart width, on one scale for the observed and the expected share.
    let longest = (1..=9)
        .map(|d| share(d).max(benford::expected(d)))
        .fold(0.0, f64::max);
    let room = opt.width.saturating_sub(10).max(10) as f64;
    let scale = room / longest;
    for d in 1..=9 {
        let (e, o) = (benford::expected(d), share(d));
        let (bar_o, mark) = (
            (o * scale).round() as usize,
            (e * scale).round().max(1.0) as usize,
        );
        let mut bar: Vec<char> = "#".repeat(bar_o).chars().collect();
        if bar.len() < mark {
            bar.resize(mark, ' ');
        }
        bar[mark - 1] = if bar_o >= mark { '|' } else { ':' };
        let _ = writeln!(
            s,
            "  {:<6}{:>8.1}%{:>9.1}%  {}",
            d,
            e * 100.0,
            o * 100.0,
            bar.iter().collect::<String>().trim_end()
        );
    }
    let legend = match opt.lang {
        Lang::En => "(# observed, | or : where Benford's law puts it)",
        Lang::Ja => "(#が実測、|または:が法則どおりの位置)",
    };
    let _ = writeln!(s, "  {legend}\n");
    let line = match (a.p_value, opt.lang) {
        (Some(p), Lang::En) => format!(
            "{} numbers, chi-square {:.3} (8 df), p = {:.4}: {}",
            grouped(a.total),
            a.chi_square,
            p,
            if p >= 0.05 { "consistent with Benford's law (p >= 0.05)" } else { "differs from Benford's law (p < 0.05)" }
        ),
        (Some(p), Lang::Ja) => format!(
            "{}個、カイ二乗 {:.3}(自由度8)、p = {:.4}: {}",
            grouped(a.total),
            a.chi_square,
            p,
            if p >= 0.05 { "ベンフォードの法則と矛盾しません(p >= 0.05)" } else { "ベンフォードの法則とずれています(p < 0.05)" }
        ),
        (None, Lang::En) => format!(
            "{} numbers: too few for the chi-square test (it needs at least {MIN_FOR_TEST}, so that every \
             digit is expected at least 5 times).",
            grouped(a.total)
        ),
        (None, Lang::Ja) => format!(
            "{}個: カイ二乗検定には少なすぎます(どの数字も5回以上期待できる{MIN_FOR_TEST}個以上が必要)。",
            grouped(a.total)
        ),
    };
    let _ = writeln!(s, "  {}", wrap(&line, WRAP - 2, 2));
    if let Source::File { scan, column, .. } = source {
        for note in scan_notes(scan, *column, opt.lang) {
            let _ = writeln!(s, "  {}", wrap(&note, WRAP - 2, 2));
        }
    }
    s
}

/// One line per paradox for `probsim all`.
pub fn summary(outcomes: &[Outcome], benford: Option<(&Source, &Analysis)>, lang: Lang) -> String {
    let mut s = String::new();
    let (h_name, h_exact, h_sim, h_ci, ok, ng, few) = match lang {
        Lang::En => (
            "paradox",
            "exact",
            "simulated",
            "95% CI",
            "ok",
            "outside",
            "too few",
        ),
        Lang::Ja => (
            "パラドックス",
            "正解",
            "結果",
            "95%信頼区間",
            "ok",
            "区間外",
            "試行不足",
        ),
    };
    let _ = writeln!(
        s,
        "{}  {}  {}  {}",
        pad_right(h_name, 12),
        pad_left(h_exact, 10),
        pad_left(h_sim, 10),
        h_ci
    );
    for o in outcomes {
        let m = o.paradox.measure();
        let ci = o.ci95.map_or("-".to_string(), |(lo, hi)| {
            format!("[{}, {}]", value(m, lo), value(m, hi))
        });
        let verdict = match o.consistent(Z95) {
            Some(true) => ok,
            Some(false) => ng,
            None => few,
        };
        let _ = writeln!(
            s,
            "{:<12}  {:>10}  {:>10}  {:<22}  {}",
            o.paradox.name(),
            value(m, o.theory),
            value(m, o.estimate),
            ci,
            verdict
        );
    }
    if let Some((src, a)) = benford {
        let (p, verdict) = match a.p_value {
            Some(p) => (format!("p={p:.3}"), if p >= 0.05 { ok } else { ng }),
            None => ("-".to_string(), few),
        };
        let _ = writeln!(
            s,
            "{:<12}  {:>10}  {:>10}  {:<22}  {}",
            "benford",
            "-",
            p,
            format!("{} {}", src.name(), grouped(a.total)),
            verdict
        );
    }
    s
}

/// A JSON string literal.
pub fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A JSON number; NaN and infinities become null.
pub fn json_num(x: f64) -> String {
    if x.is_finite() {
        format!("{x}")
    } else {
        "null".to_string()
    }
}

fn json_opt(x: Option<f64>) -> String {
    x.map_or("null".to_string(), json_num)
}

fn json_pair(x: Option<(f64, f64)>) -> String {
    x.map_or("null".to_string(), |(a, b)| {
        format!("[{},{}]", json_num(a), json_num(b))
    })
}

fn json_param(v: &str) -> String {
    if v.parse::<f64>().is_ok_and(f64::is_finite) {
        v.to_string()
    } else {
        json_str(v)
    }
}

pub fn outcome_json(o: &Outcome, stories: &[(Option<f64>, String)]) -> String {
    let params = o
        .paradox
        .params()
        .iter()
        .map(|(k, v)| format!("{}:{}", json_str(k), json_param(v)))
        .collect::<Vec<_>>()
        .join(",");
    let checkpoints = o
        .checkpoints
        .iter()
        .map(|c| format!("[{},{}]", c.n, json_num(c.estimate)))
        .collect::<Vec<_>>()
        .join(",");
    let mut s = format!(
        "{{\"paradox\":{},\"params\":{{{}}},\"measure\":{},\"seed\":{},\"trials\":{},\"counted\":{},\
         \"estimate\":{},\"standardError\":{},\"ci95\":{},\"theory\":{},\"exact\":{},\
         \"consistent95\":{},\"checkpoints\":[{}]",
        json_str(o.paradox.name()),
        params,
        json_str(match o.paradox.measure() {
            Measure::Probability => "probability",
            Measure::Mean => "mean",
        }),
        o.seed,
        o.attempted,
        o.counted,
        json_num(o.estimate),
        json_num(o.standard_error),
        json_pair(o.ci95),
        json_num(o.theory),
        o.paradox.exact().map_or("null".to_string(), |e| json_str(&e)),
        o.consistent(Z95).map_or("null".to_string(), |b| b.to_string()),
        checkpoints
    );
    if let Paradox::Buffon { length, spacing } = o.paradox {
        let pi = |p: f64| pi_from(length, spacing, p);
        let _ = write!(
            s,
            ",\"piEstimate\":{},\"piCi95\":{}",
            json_opt(pi(o.estimate)),
            o.ci95
                .and_then(|(lo, hi)| Some((pi(hi)?, pi(lo)?)))
                .map_or("null".to_string(), |p| json_pair(Some(p)))
        );
    }
    if !stories.is_empty() {
        let list = stories
            .iter()
            .map(|(r, st)| format!("{{\"result\":{},\"story\":{}}}", json_opt(*r), json_str(st)))
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(s, ",\"explained\":[{list}]");
    }
    s.push('}');
    s
}

pub fn benford_json(source: &Source, a: &Analysis) -> String {
    let file = match source {
        Source::File { name, column, scan } => format!(
            ",\"file\":{},\"column\":{},\"numbersFound\":{},\"zeros\":{},\"readWithThousandsCommas\":{}",
            json_str(name),
            column.map_or("null".to_string(), |c| c.to_string()),
            scan.numbers,
            scan.zeros,
            scan.grouped
        ),
        _ => String::new(),
    };
    let digits = (1..=9)
        .map(|d| {
            format!(
                "{{\"digit\":{d},\"count\":{},\"share\":{},\"benford\":{}}}",
                a.counts[d - 1],
                json_num(if a.total == 0 {
                    0.0
                } else {
                    a.counts[d - 1] as f64 / a.total as f64
                }),
                json_num(benford::expected(d))
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"paradox\":\"benford\",\"source\":{}{file},\"total\":{},\"chiSquare\":{},\"df\":8,\"pValue\":{},\"digits\":[{}]}}",
        json_str(source.name()),
        a.total,
        json_num(a.chi_square),
        json_opt(a.p_value),
        digits
    )
}

/// Minimum trials for an average, re-exported for the CLI's messages.
pub const MIN_TRIALS_FOR_MEAN: u64 = MIN_FOR_MEAN;
