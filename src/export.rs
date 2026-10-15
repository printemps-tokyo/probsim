//! CSV and SVG output of a run's convergence, for spreadsheets and slides.

use std::fmt::Write as _;

use crate::paradox::Measure;
use crate::sim::Outcome;
use crate::stats::{MIN_FOR_MEAN, Z95};

/// The exact +- 1.96 sd / sqrt(n) band at n, where its normal approximation
/// holds (n p and n (1 - p) >= 5 for a probability, 30 trials for an average).
pub fn band(o: &Outcome, n: u64) -> Option<(f64, f64)> {
    band_with(o, o.variance().sqrt(), n)
}

/// `band` with the standard deviation of one trial already computed.
fn band_with(o: &Outcome, sd: f64, n: u64) -> Option<(f64, f64)> {
    let nf = n as f64;
    let holds = match o.paradox.measure() {
        Measure::Probability => nf * o.theory >= 5.0 && nf * (1.0 - o.theory) >= 5.0,
        Measure::Mean => n >= MIN_FOR_MEAN,
    };
    (holds && sd.is_finite() && sd > 0.0).then(|| {
        let h = Z95 * sd / nf.sqrt();
        (o.theory - h, o.theory + h)
    })
}

/// Checkpoints as CSV: n, estimate, exact, band_low, band_high.
pub fn csv(o: &Outcome) -> String {
    let sd = o.variance().sqrt();
    let mut s = String::from("n,estimate,exact,band_low,band_high\n");
    for c in &o.checkpoints {
        let (lo, hi) = band_with(o, sd, c.n).map_or((String::new(), String::new()), |(l, h)| {
            (l.to_string(), h.to_string())
        });
        let _ = writeln!(s, "{},{},{},{},{}", c.n, c.estimate, o.theory, lo, hi);
    }
    s
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The convergence chart as a standalone SVG: the running estimate on a log
/// scale of trials, the exact answer, and the band where it holds.
pub fn svg(o: &Outcome, title: &str, lang: crate::paradox::Lang) -> String {
    let sd = o.variance().sqrt();
    let (w, h) = (720.0, 400.0);
    let (left, right, top, bottom) = (70.0, 20.0, 40.0, 50.0);
    let pts = &o.checkpoints;
    let mut s = String::new();
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" \
         font-family=\"sans-serif\" font-size=\"12\">\n<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n\
         <text x=\"{left}\" y=\"24\" font-size=\"15\" fill=\"#1d1d1f\">{}</text>\n",
        xml(title)
    );
    if pts.is_empty() {
        s.push_str("</svg>\n");
        return s;
    }
    let (n0, n1) = (pts[0].n.max(1) as f64, pts[pts.len() - 1].n.max(1) as f64);
    let settled: Vec<f64> = pts
        .iter()
        .filter(|c| c.n >= 30)
        .map(|c| c.estimate)
        .collect();
    let pool: Vec<f64> = if settled.is_empty() {
        pts.iter().map(|c| c.estimate).collect()
    } else {
        settled
    };
    let spread = pool
        .iter()
        .map(|e| (e - o.theory).abs())
        .fold(0.0, f64::max)
        .max(1e-4 * o.theory.abs().max(1.0))
        * 1.2;
    let (mut ymax, mut ymin) = (o.theory + spread, o.theory - spread);
    if o.paradox.measure() == Measure::Probability {
        ymax = ymax.min(1.0);
        ymin = ymin.max(0.0);
    }
    let x = |n: f64| {
        if n1 <= n0 {
            w - right
        } else {
            left + (n.ln() - n0.ln()) / (n1.ln() - n0.ln()) * (w - left - right)
        }
    };
    let y = |v: f64| top + (ymax - v.clamp(ymin, ymax)) / (ymax - ymin) * (h - top - bottom);
    // The band, as a polygon over the checkpoints where it holds.
    let banded: Vec<(f64, (f64, f64))> = pts
        .iter()
        .filter_map(|c| band_with(o, sd, c.n).map(|b| (c.n as f64, b)))
        .collect();
    if banded.len() > 1 {
        let upper: Vec<String> = banded
            .iter()
            .map(|(n, (_, hi))| format!("{:.1},{:.1}", x(*n), y(*hi)))
            .collect();
        let lower: Vec<String> = banded
            .iter()
            .rev()
            .map(|(n, (lo, _))| format!("{:.1},{:.1}", x(*n), y(*lo)))
            .collect();
        let _ = writeln!(
            s,
            "<polygon points=\"{} {}\" fill=\"#cfe3ff\" opacity=\"0.7\"/>",
            upper.join(" "),
            lower.join(" ")
        );
    }
    let _ = writeln!(
        s,
        "<line x1=\"{left}\" y1=\"{0:.1}\" x2=\"{1}\" y2=\"{0:.1}\" stroke=\"#c2185b\" stroke-width=\"1.5\" stroke-dasharray=\"6 4\"/>",
        y(o.theory),
        w - right
    );
    let line: Vec<String> = pts
        .iter()
        .map(|c| format!("{:.1},{:.1}", x(c.n.max(1) as f64), y(c.estimate)))
        .collect();
    let _ = writeln!(
        s,
        "<polyline points=\"{}\" fill=\"none\" stroke=\"#1a73e8\" stroke-width=\"2\"/>",
        line.join(" ")
    );
    // Axes and labels.
    let _ = writeln!(
        s,
        "<line x1=\"{left}\" y1=\"{0}\" x2=\"{1}\" y2=\"{0}\" stroke=\"#888\"/>\n<line x1=\"{left}\" y1=\"{top}\" x2=\"{left}\" y2=\"{0}\" stroke=\"#888\"/>",
        h - bottom,
        w - right
    );
    // Enough decimals to tell the labels apart, even for a tiny probability.
    let decimals = ((-(ymax - ymin).log10()).ceil() as i64 + 2).clamp(2, 12) as usize;
    let mut shown: Vec<String> = Vec::new();
    for v in [ymax, o.theory, ymin] {
        let label = format!("{v:.decimals$}");
        if shown.contains(&label) {
            continue;
        }
        let _ = writeln!(
            s,
            "<text x=\"{}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"#444\">{}</text>",
            left - 6.0,
            y(v) + 4.0,
            label
        );
        shown.push(label);
    }
    let mut n = 1u64;
    while (n as f64) <= n1 {
        if n as f64 >= n0 {
            let _ = writeln!(
                s,
                "<text x=\"{:.1}\" y=\"{}\" text-anchor=\"middle\" fill=\"#444\">{}</text>",
                x(n as f64),
                h - bottom + 18.0,
                crate::output::compact(n)
            );
        }
        n = n.saturating_mul(10);
        if n == u64::MAX {
            break;
        }
    }
    let _ = writeln!(
        s,
        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"#444\">{}</text>",
        (left + w - right) / 2.0,
        h - 10.0,
        match lang {
            crate::paradox::Lang::En => "trials (log scale)",
            crate::paradox::Lang::Ja => "試行回数(対数目盛り)",
        }
    );
    s.push_str("</svg>\n");
    s
}
