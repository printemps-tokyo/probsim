//! Benford's law: how often each leading digit (1-9) or pair of leading
//! digits (10-99) appears, in a sequence or in your data.

use crate::output::grouped as grouped_number;
use crate::paradox::Lang;
use crate::stats::chi_square_p;

/// Expected share of leading digits d (1-9, or 10-99 for the first two):
/// log10(1 + 1/d).
pub fn expected(d: usize) -> f64 {
    (1.0 + 1.0 / d as f64).log10()
}

/// The range of leading-digit values for a test of the first `digits` digits.
pub fn range(digits: usize) -> std::ops::RangeInclusive<usize> {
    if digits == 2 {
        10..=99
    } else {
        1..=9
    }
}

/// Numbers needed before the chi-square test is run: every value's expected
/// count must reach 5, the usual rule of thumb for the approximation (110 for
/// the first digit, where 9 is expected 4.6% of the time; 1,146 for the
/// first two digits).
pub fn min_for_test(digits: usize) -> u64 {
    let rarest = expected(*range(digits).end());
    (5.0 / rarest).ceil() as u64
}

/// Nigrini's mean absolute deviation thresholds (close, acceptable,
/// marginal conformity; above them, nonconformity), from Nigrini (2012),
/// Benford's Law. Empirical rules, not statistical tests.
pub fn mad_thresholds(digits: usize) -> [f64; 3] {
    if digits == 2 {
        [0.0012, 0.0018, 0.0022]
    } else {
        [0.006, 0.012, 0.015]
    }
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

/// The first two significant digits (10-99) of a number as written, before
/// any exponent; a number with one significant digit counts as d0 (7 is
/// 7.0, so 70). None for zero. Exact for any length, with no conversion to
/// a float.
pub fn leading_two_of(number: &str) -> Option<usize> {
    let mut digits = number
        .bytes()
        .take_while(|b| *b != b'e' && *b != b'E')
        .filter(u8::is_ascii_digit)
        .skip_while(|b| *b == b'0');
    let first = digits.next()?;
    let second = digits.next().unwrap_or(b'0');
    Some(((first - b'0') * 10 + (second - b'0')) as usize)
}

/// The leading digit (1-9) of a number as written.
pub fn leading_digit_of(number: &str) -> Option<usize> {
    leading_two_of(number).map(|v| v / 10)
}

/// The first two digits of 10^f for f in [0, 1): the largest d in 10..=99
/// with log10(d) - 1 <= f. The small tolerance keeps a value exactly on a
/// boundary (such as log10(8) computed as 3 * log10(2)) on the right side.
fn two_of_fraction(f: f64) -> usize {
    10 + (11..=99)
        .filter(|&d| (d as f64).log10() - 1.0 <= f + 1e-12)
        .count()
}

fn first_two_u128(v: u128) -> usize {
    let s = v.to_string();
    let b = s.as_bytes();
    ((b[0] - b'0') * 10 + b.get(1).map_or(0, |d| d - b'0')) as usize
}

/// First two digits of 2^k (k >= 1): exact while 2^k fits in a u128
/// (k <= 127), then from the fractional part of k * log10(2). In double
/// precision that part is off by about k * 1e-17, so in a run of millions of
/// terms a rare term lying that close to a boundary may get its neighbor.
pub fn power_of_two_two(k: u64) -> usize {
    if k <= 127 {
        first_two_u128(1u128 << k)
    } else {
        let x = k as f64 * 2f64.log10();
        two_of_fraction(x - x.floor())
    }
}

pub fn power_of_two_digit(k: u64) -> usize {
    power_of_two_two(k) / 10
}

pub fn powers_of_two(n: u64) -> Vec<usize> {
    (1..=n).map(power_of_two_digit).collect()
}

/// First two digits of the Fibonacci numbers F_1, F_2, ...: exact while they
/// fit in a u128 (up to F_186), then from log10 F_k = k log10(phi) - log10(sqrt 5).
pub struct Fibonacci {
    k: u64,
    a: u128,
    b: u128,
}

impl Fibonacci {
    pub fn new() -> Fibonacci {
        Fibonacci { k: 0, a: 1, b: 1 }
    }
}

impl Default for Fibonacci {
    fn default() -> Self {
        Self::new()
    }
}

impl Iterator for Fibonacci {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        self.k += 1;
        if self.k <= 186 {
            let d = first_two_u128(self.a);
            let next = self.a.saturating_add(self.b);
            self.a = self.b;
            self.b = next;
            Some(d)
        } else {
            let phi = (1.0 + 5f64.sqrt()) / 2.0;
            let x = self.k as f64 * phi.log10() - 5f64.sqrt().log10();
            Some(two_of_fraction(x - x.floor()))
        }
    }
}

pub fn fibonacci(n: u64) -> Vec<usize> {
    Fibonacci::new().take(n as usize).map(|v| v / 10).collect()
}

/// A number found in text, as written (thousands commas removed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    /// It contained thousands commas ("1,234").
    pub grouped: bool,
}

/// Numbers in free text: digits with an optional sign, decimal point and
/// exponent. A sign counts only when it does not follow a letter, digit or
/// dot ("2024-01-15" is three numbers, not a subtraction). Commas that group
/// thousands ("1,234,567") stay inside the number; other commas separate.
pub fn number_tokens(text: &str) -> Vec<Token> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        let after_word = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'.');
        let mut negative = false;
        if (b[i] == b'-' || b[i] == b'+')
            && !after_word
            && i + 1 < b.len()
            && b[i + 1].is_ascii_digit()
        {
            negative = b[i] == b'-';
            i += 1;
        }
        let digit_start = i < b.len() && b[i].is_ascii_digit();
        let dot_start = i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() && !after_word;
        if !(digit_start || dot_start) {
            i = start + 1;
            continue;
        }
        let mut s = String::new();
        if negative {
            s.push('-');
        }
        let mut grouped = false;
        let int_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        s.push_str(&text[int_start..i]);
        if i > int_start && i - int_start <= 3 {
            // ",ddd" not followed by another digit continues the number.
            while i + 3 < b.len()
                && b[i] == b','
                && b[i + 1..i + 4].iter().all(u8::is_ascii_digit)
                && (i + 4 == b.len() || !b[i + 4].is_ascii_digit())
            {
                s.push_str(&text[i + 1..i + 4]);
                grouped = true;
                i += 4;
            }
        }
        // A decimal point, with or without digits after it ("1.5", "1.e3").
        if i < b.len()
            && b[i] == b'.'
            && (i + 1 < b.len() && b[i + 1].is_ascii_digit() || i > int_start)
        {
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
        out.push(Token { text: s, grouped });
    }
    out
}

/// CSV records: fields split at `delimiter` outside double quotes, records
/// at line ends outside quotes (so a quoted field may span lines), and ""
/// inside quotes standing for a quote (RFC 4180).
pub fn csv_records(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut fields = vec![String::new()];
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                fields.last_mut().unwrap().push('"');
            }
            '"' => quoted = !quoted,
            '\r' if !quoted && chars.peek() == Some(&'\n') => {}
            '\n' if !quoted => records.push(std::mem::replace(&mut fields, vec![String::new()])),
            c if c == delimiter && !quoted => fields.push(String::new()),
            c => fields.last_mut().unwrap().push(c),
        }
    }
    if fields.len() > 1 || !fields[0].is_empty() {
        records.push(fields);
    }
    records
}

/// The fields of one CSV line (see `csv_records`).
pub fn csv_fields(line: &str, delimiter: char) -> Vec<String> {
    csv_records(line, delimiter)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// What a scan of a file found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scan {
    /// counts[d - 10] for the first two digits d (10-99).
    pub counts: Vec<u64>,
    /// Numbers found, zeros included.
    pub numbers: u64,
    /// Numbers that are zero, which have no leading digit.
    pub zeros: u64,
    /// Numbers read with thousands commas.
    pub grouped: u64,
}

impl Default for Scan {
    fn default() -> Self {
        Scan {
            counts: vec![0; 90],
            numbers: 0,
            zeros: 0,
            grouped: 0,
        }
    }
}

impl Scan {
    fn add(&mut self, t: &Token) {
        self.numbers += 1;
        if t.grouped {
            self.grouped += 1;
        }
        match leading_two_of(&t.text) {
            Some(d) => self.counts[d - 10] += 1,
            None => self.zeros += 1,
        }
    }
}

/// Every number in free text.
pub fn scan_text(text: &str) -> Scan {
    let mut s = Scan::default();
    for t in number_tokens(text) {
        s.add(&t);
    }
    s
}

/// The numbers in one column (1-based) of CSV text. Records without that
/// column, and fields without a number (a header), are skipped.
pub fn scan_csv(text: &str, column: usize, delimiter: char) -> Scan {
    let mut s = Scan::default();
    for record in csv_records(text, delimiter) {
        if let Some(field) = record.get(column - 1) {
            for t in number_tokens(field) {
                s.add(&t);
            }
        }
    }
    s
}

/// Text from a file's bytes: UTF-16 with a byte order mark is decoded;
/// anything else is read as an ASCII-compatible encoding (UTF-8, Shift_JIS,
/// EUC-JP, Latin-1...), where only the ASCII digits matter. NUL bytes without
/// a byte order mark (UTF-16 without one, or a binary file) are refused,
/// since the digits would be read apart.
pub fn decode(bytes: &[u8]) -> Result<String, String> {
    let utf16 = |be: bool| {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| {
                if be {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    u16::from_le_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, ..] => Ok(utf16(false)),
        [0xFE, 0xFF, ..] => Ok(utf16(true)),
        _ if bytes.contains(&0) => Err(
            "the file contains NUL bytes: UTF-16 without a byte order mark, or not text; save it as UTF-8".into(),
        ),
        _ => Ok(String::from_utf8_lossy(bytes).into_owned()),
    }
}

/// Where the numbers come from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    PowersOfTwo(u64),
    Fibonacci(u64),
    /// A file's name ("-" for standard input), the column read (if any), and
    /// what was found.
    File {
        name: String,
        column: Option<usize>,
        scan: Scan,
    },
}

impl Source {
    /// Counts of the first `digits` digits (9 values or 90), without keeping
    /// the sequence in memory.
    pub fn counts(&self, digits: usize) -> Vec<u64> {
        let mut two = vec![0u64; 90];
        match self {
            Source::PowersOfTwo(n) => (1..=*n).for_each(|k| two[power_of_two_two(k) - 10] += 1),
            Source::Fibonacci(n) => Fibonacci::new()
                .take(*n as usize)
                .for_each(|d| two[d - 10] += 1),
            Source::File { scan, .. } => two.clone_from(&scan.counts),
        }
        if digits == 2 {
            two
        } else {
            (0..9)
                .map(|d| two[d * 10..d * 10 + 10].iter().sum())
                .collect()
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Source::PowersOfTwo(_) => "powers2",
            Source::Fibonacci(_) => "fibonacci",
            Source::File { .. } => "file",
        }
    }
}

/// How closely the data follows Benford's law by Nigrini's MAD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conformity {
    Close,
    Acceptable,
    Marginal,
    Nonconformity,
}

/// Observed counts and the tests against Benford.
#[derive(Clone, Debug)]
pub struct Analysis {
    /// 1 or 2: the first digit, or the first two.
    pub digits: usize,
    /// counts[i] for the i-th value of `range(digits)`.
    pub counts: Vec<u64>,
    pub total: u64,
    pub chi_square: f64,
    pub df: u32,
    /// None below `min_for_test(digits)` numbers.
    pub p_value: Option<f64>,
    /// Mean absolute deviation of the shares from Benford's.
    pub mad: f64,
    pub conformity: Conformity,
}

pub fn analyze(counts: Vec<u64>, digits: usize) -> Analysis {
    let total: u64 = counts.iter().sum();
    let values: Vec<usize> = range(digits).collect();
    let share = |i: usize| {
        if total == 0 {
            0.0
        } else {
            counts[i] as f64 / total as f64
        }
    };
    let chi_square = if total == 0 {
        0.0
    } else {
        values
            .iter()
            .enumerate()
            .map(|(i, &d)| {
                let e = expected(d) * total as f64;
                let o = counts[i] as f64;
                (o - e) * (o - e) / e
            })
            .sum()
    };
    let mad = values
        .iter()
        .enumerate()
        .map(|(i, &d)| (share(i) - expected(d)).abs())
        .sum::<f64>()
        / values.len() as f64;
    let [close, acceptable, marginal] = mad_thresholds(digits);
    let conformity = if mad <= close {
        Conformity::Close
    } else if mad <= acceptable {
        Conformity::Acceptable
    } else if mad <= marginal {
        Conformity::Marginal
    } else {
        Conformity::Nonconformity
    };
    let df = values.len() as u32 - 1;
    Analysis {
        digits,
        counts,
        total,
        chi_square,
        df,
        p_value: (total >= min_for_test(digits)).then(|| chi_square_p(chi_square, df)),
        mad,
        conformity,
    }
}

/// Counts of a list of first digits (for tests and small inputs).
pub fn count_digits(digits: &[usize]) -> Vec<u64> {
    let mut c = vec![0u64; 9];
    for &d in digits {
        if (1..=9).contains(&d) {
            c[d - 1] += 1;
        }
    }
    c
}

/// Title, question, intuition, truth and why, for the report.
pub fn texts(source: &Source, digits: usize, lang: Lang) -> [String; 5] {
    let what_en = match source {
        Source::PowersOfTwo(n) => format!("the powers of two 2^1 .. 2^{n}"),
        Source::Fibonacci(n) => format!("the first {} Fibonacci numbers", grouped_number(*n)),
        Source::File {
            name,
            column: Some(c),
            ..
        } => format!("column {c} of {}", display_name(name)),
        Source::File { name, .. } => format!("the numbers in {}", display_name(name)),
    };
    let what_ja = match source {
        Source::PowersOfTwo(n) => format!("2のべき乗 2^1 .. 2^{n}"),
        Source::Fibonacci(n) => format!("最初の{}個のフィボナッチ数", grouped_number(*n)),
        Source::File {
            name,
            column: Some(c),
            ..
        } => format!("{}の{c}列目", display_name(name)),
        Source::File { name, .. } => format!("{}に含まれる数値", display_name(name)),
    };
    match (lang, digits) {
        (Lang::En, 2) => [
            format!("Benford's law, first two digits: {what_en}"),
            format!("How often does each pair of leading digits 10-99 come first in {what_en}?"),
            "Every pair should lead about equally often, 1/90 (1.1%) each.".into(),
            "In data that follows Benford's law, the first two digits d come with probability \
             log10(1 + 1/d): 4.1% for 10, down to 0.44% for 99."
                .into(),
            "The same log-scale reason as for the first digit, one level finer; the first-two-digits \
             test is the sharper one for finding where data departs from the law."
                .into(),
        ],
        (Lang::Ja, 2) => [
            format!("ベンフォードの法則、先頭2桁: {what_ja}"),
            format!("{what_ja}で、先頭2桁10から99はそれぞれどのくらいの割合?"),
            "どの2桁も同じくらい、1/90(1.1%)ずつのはず。".into(),
            "ベンフォードの法則に従うデータでは、先頭2桁がdになる確率はlog10(1 + 1/d)。10は4.1%、99はわずか0.44%です。"
                .into(),
            "先頭1桁と同じく対数の目盛りによるもので、それを1段細かく見ています。法則からどこが外れているかを探すには\
             先頭2桁の検定のほうが鋭敏です。"
                .into(),
        ],
        (Lang::En, _) => [
            format!("Benford's law: leading digits of {what_en}"),
            format!("How often does each digit 1-9 come first in {what_en}?"),
            "Every digit should lead about equally often, 1/9 (11.1%) each.".into(),
            "In data that follows Benford's law, digit d leads with probability log10(1 + 1/d): \
             30.1% for 1, down to 4.6% for 9."
                .into(),
            "Quantities that grow by multiplying spread evenly on a log scale, and on a log scale the \
             stretch from 1 to 2 is much longer than the stretch from 9 to 10."
                .into(),
        ],
        (Lang::Ja, _) => [
            format!("ベンフォードの法則: {what_ja}の先頭の数字"),
            format!("{what_ja}で、先頭に来る数字1から9はそれぞれどのくらいの割合?"),
            "どの数字も同じくらい、1/9(11.1%)ずつのはず。".into(),
            "ベンフォードの法則に従うデータでは、先頭がdになる確率はlog10(1 + 1/d)。1は30.1%、9はわずか4.6%です。"
                .into(),
            "掛け算で増えていく量は対数の目盛りの上で均等に散らばり、対数の目盛りでは1から2までの幅が\
             9から10までよりずっと広いからです。"
                .into(),
        ],
    }
}

fn display_name(name: &str) -> String {
    if name == "-" {
        "standard input".to_string()
    } else {
        name.to_string()
    }
}
