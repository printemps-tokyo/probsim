//! Benford's law: how often each digit 1-9 leads, in a sequence or in your data.

use crate::stats::chi_square_p;

/// Expected share of leading digit d (1-9): log10(1 + 1/d).
pub fn expected(d: usize) -> f64 {
    (1.0 + 1.0 / d as f64).log10()
}

/// The leading digit (1-9) of a nonzero finite number, or None.
pub fn leading_digit(x: f64) -> Option<usize> {
    if !x.is_finite() || x == 0.0 {
        return None;
    }
    // Scientific notation puts the leading digit first, exactly.
    let s = format!("{:e}", x.abs());
    s.chars()
        .next()?
        .to_digit(10)
        .map(|d| d as usize)
        .filter(|&d| d >= 1)
}

/// Leading digit of 10^f for f in [0, 1): the largest d with log10(d) <= f.
/// The small tolerance keeps a value that is exactly on a boundary (such as
/// log10(8) computed as 3 * log10(2)) on the right side of it.
fn digit_of_fraction(f: f64) -> usize {
    1 + (2..=9).filter(|&d| (d as f64).log10() <= f + 1e-12).count()
}

/// Leading digits of 2^1 .. 2^n: exact while 2^k fits in a u128 (k <= 127),
/// then from the fractional part of k * log10(2). In double precision that
/// part is off by about k * 1e-17, so in a run of millions of terms a rare
/// term lying that close to a digit boundary may get the neighboring digit.
pub fn powers_of_two(n: u64) -> Vec<usize> {
    let l = 2f64.log10();
    (1..=n)
        .map(|k| {
            if k <= 127 {
                let v = 1u128 << k;
                (v.to_string().as_bytes()[0] - b'0') as usize
            } else {
                let x = k as f64 * l;
                digit_of_fraction(x - x.floor())
            }
        })
        .collect()
}

/// Leading digits of the Fibonacci numbers F_1 .. F_n: exact while they fit
/// in a u128 (up to F_186), then from log10 F_k = k log10(phi) - log10(sqrt 5),
/// which is off by far less than one digit's width from there on.
pub fn fibonacci(n: u64) -> Vec<usize> {
    let mut out = Vec::with_capacity(n as usize);
    let (mut a, mut b) = (1u128, 1u128);
    let phi = (1.0 + 5f64.sqrt()) / 2.0;
    let (lphi, lsq5) = (phi.log10(), 5f64.sqrt().log10());
    for k in 1..=n {
        if k <= 186 {
            out.push((a.to_string().as_bytes()[0] - b'0') as usize);
            let next = a.checked_add(b);
            a = b;
            b = next.unwrap_or(u128::MAX);
        } else {
            let x = k as f64 * lphi - lsq5;
            out.push(digit_of_fraction(x - x.floor()));
        }
    }
    out
}

/// Numbers in free text: runs of digits with an optional sign, decimal point
/// and exponent. Commas that group thousands ("1,234,567") are part of the
/// number; any other comma separates numbers.
pub fn numbers_in(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        if (b[i] == b'-' || b[i] == b'+')
            && i + 1 < b.len()
            && (b[i + 1].is_ascii_digit() || b[i + 1] == b'.')
        {
            i += 1;
        }
        if i < b.len()
            && (b[i].is_ascii_digit()
                || (b[i] == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()))
        {
            let mut s = String::new();
            if b[start] == b'-' {
                s.push('-');
            }
            // Integer part with optional thousands groups.
            let int_start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            s.push_str(&text[int_start..i]);
            if i - int_start <= 3 {
                // ",ddd" not followed by another digit continues the number.
                while i + 3 < b.len()
                    && b[i] == b','
                    && b[i + 1..i + 4].iter().all(u8::is_ascii_digit)
                    && (i + 4 == b.len() || !b[i + 4].is_ascii_digit())
                {
                    s.push_str(&text[i + 1..i + 4]);
                    i += 4;
                }
            }
            if i < b.len() && b[i] == b'.' {
                let f = i;
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                s.push_str(&text[f..i]);
            }
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                let mut j = i + 1;
                if j < b.len() && (b[j] == b'-' || b[j] == b'+') {
                    j += 1;
                }
                if j < b.len() && b[j].is_ascii_digit() {
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                    s.push_str(&text[i..j]);
                    i = j;
                }
            }
            if let Ok(x) = s.parse::<f64>() {
                out.push(x);
            }
        } else {
            i = start + 1;
            // Skip the rest of a multi-byte character.
            while i < b.len() && (b[i] & 0xC0) == 0x80 {
                i += 1;
            }
        }
    }
    out
}

/// Observed counts and the test against Benford.
#[derive(Clone, Debug)]
pub struct Analysis {
    /// counts[d - 1] for digits 1-9.
    pub counts: [u64; 9],
    pub total: u64,
    pub chi_square: f64,
    pub p_value: f64,
}

pub fn analyze(digits: &[usize]) -> Analysis {
    let mut counts = [0u64; 9];
    for &d in digits {
        if (1..=9).contains(&d) {
            counts[d - 1] += 1;
        }
    }
    let total: u64 = counts.iter().sum();
    let chi_square = if total == 0 {
        0.0
    } else {
        (1..=9)
            .map(|d| {
                let e = expected(d) * total as f64;
                let o = counts[d - 1] as f64;
                (o - e) * (o - e) / e
            })
            .sum()
    };
    Analysis {
        counts,
        total,
        chi_square,
        p_value: if total == 0 {
            1.0
        } else {
            chi_square_p(chi_square, 8)
        },
    }
}

/// Where the numbers come from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    PowersOfTwo(u64),
    Fibonacci(u64),
    /// A file's name and the numbers found in it.
    File(String, Vec<f64>),
}

impl Source {
    pub fn digits(&self) -> Vec<usize> {
        match self {
            Source::PowersOfTwo(n) => powers_of_two(*n),
            Source::Fibonacci(n) => fibonacci(*n),
            Source::File(_, xs) => xs.iter().filter_map(|&x| leading_digit(x)).collect(),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Source::PowersOfTwo(_) => "powers2",
            Source::Fibonacci(_) => "fibonacci",
            Source::File(..) => "file",
        }
    }
}

/// Title, question, intuition, truth and why, for the report.
pub fn texts(source: &Source, lang: crate::paradox::Lang) -> [String; 5] {
    use crate::paradox::Lang;
    let what_en = match source {
        Source::PowersOfTwo(n) => format!("the powers of two 2^1 .. 2^{n}"),
        Source::Fibonacci(n) => {
            format!("the first {} Fibonacci numbers", crate::output::grouped(*n))
        }
        Source::File(f, _) => format!("the numbers in {f}"),
    };
    let what_ja = match source {
        Source::PowersOfTwo(n) => format!("2のべき乗 2^1 .. 2^{n}"),
        Source::Fibonacci(n) => format!("最初の{}個のフィボナッチ数", crate::output::grouped(*n)),
        Source::File(f, _) => format!("{f}に含まれる数値"),
    };
    match lang {
        Lang::En => [
            format!("Benford's law: leading digits of {what_en}"),
            format!("How often does each digit 1-9 come first in {what_en}?"),
            "Every digit should lead about equally often, 1/9 (11.1%) each.".into(),
            "Digit d leads with probability log10(1 + 1/d): 30.1% for 1, down to 4.6% for 9.".into(),
            "Quantities that grow by multiplying spread evenly on a log scale, and on a log scale the \
             stretch from 1 to 2 is much longer than the stretch from 9 to 10."
                .into(),
        ],
        Lang::Ja => [
            format!("ベンフォードの法則: {what_ja}の先頭の数字"),
            format!("{what_ja}で、先頭に来る数字1から9はそれぞれどのくらいの割合?"),
            "どの数字も同じくらい、1/9(11.1%)ずつのはず。".into(),
            "先頭がdになる確率はlog10(1 + 1/d)。1は30.1%、9はわずか4.6%です。".into(),
            "掛け算で増えていく量は対数の目盛りの上で均等に散らばり、対数の目盛りでは1から2までの幅が\
             9から10までよりずっと広いからです。"
                .into(),
        ],
    }
}
