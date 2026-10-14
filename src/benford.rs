//! Benford's law: how often each digit 1-9 leads, in a sequence or in your data.

use crate::output::grouped as grouped_number;
use crate::paradox::Lang;
use crate::stats::chi_square_p;

/// Below this many numbers the chi-square test is not run: digit 9's share
/// is 4.6%, so 110 numbers is where every expected count reaches 5, the usual
/// rule of thumb for the chi-square approximation.
pub const MIN_FOR_TEST: u64 = 110;

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

/// The leading digit of a number as written: the first nonzero digit before
/// any exponent. Exact for any length ("9999999999999999999" leads with 9,
/// "1e400" with 1), with no conversion to a float. None for zero.
pub fn leading_digit_of(number: &str) -> Option<usize> {
    number
        .bytes()
        .take_while(|b| *b != b'e' && *b != b'E')
        .find(|b| (b'1'..=b'9').contains(b))
        .map(|b| (b - b'0') as usize)
}

/// Leading digit of 10^f for f in [0, 1): the largest d with log10(d) <= f.
/// The small tolerance keeps a value that is exactly on a boundary (such as
/// log10(8) computed as 3 * log10(2)) on the right side of it.
fn digit_of_fraction(f: f64) -> usize {
    1 + (2..=9).filter(|&d| (d as f64).log10() <= f + 1e-12).count()
}

fn first_digit_u128(v: u128) -> usize {
    (v.to_string().as_bytes()[0] - b'0') as usize
}

/// Leading digit of 2^k (k >= 1): exact while 2^k fits in a u128 (k <= 127),
/// then from the fractional part of k * log10(2). In double precision that
/// part is off by about k * 1e-17, so in a run of millions of terms a rare
/// term lying that close to a digit boundary may get the neighboring digit.
pub fn power_of_two_digit(k: u64) -> usize {
    if k <= 127 {
        first_digit_u128(1u128 << k)
    } else {
        let x = k as f64 * 2f64.log10();
        digit_of_fraction(x - x.floor())
    }
}

pub fn powers_of_two(n: u64) -> Vec<usize> {
    (1..=n).map(power_of_two_digit).collect()
}

/// Leading digits of the Fibonacci numbers F_1, F_2, ...: exact while they
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
            let d = first_digit_u128(self.a);
            let next = self.a.saturating_add(self.b);
            self.a = self.b;
            self.b = next;
            Some(d)
        } else {
            let phi = (1.0 + 5f64.sqrt()) / 2.0;
            let x = self.k as f64 * phi.log10() - 5f64.sqrt().log10();
            Some(digit_of_fraction(x - x.floor()))
        }
    }
}

pub fn fibonacci(n: u64) -> Vec<usize> {
    Fibonacci::new().take(n as usize).collect()
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// counts[d - 1] for leading digits 1-9.
    pub counts: [u64; 9],
    /// Numbers found, zeros included.
    pub numbers: u64,
    /// Numbers that are zero, which have no leading digit.
    pub zeros: u64,
    /// Numbers read with thousands commas.
    pub grouped: u64,
}

impl Scan {
    fn add(&mut self, t: &Token) {
        self.numbers += 1;
        if t.grouped {
            self.grouped += 1;
        }
        match leading_digit_of(&t.text) {
            Some(d) => self.counts[d - 1] += 1,
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

/// Where the numbers come from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    PowersOfTwo(u64),
    Fibonacci(u64),
    /// A file's name, the column read (if any), and what was found.
    File {
        name: String,
        column: Option<usize>,
        scan: Scan,
    },
}

impl Source {
    /// Leading-digit counts, without keeping the sequence in memory.
    pub fn counts(&self) -> [u64; 9] {
        let mut c = [0u64; 9];
        match self {
            Source::PowersOfTwo(n) => (1..=*n).for_each(|k| c[power_of_two_digit(k) - 1] += 1),
            Source::Fibonacci(n) => Fibonacci::new()
                .take(*n as usize)
                .for_each(|d| c[d - 1] += 1),
            Source::File { scan, .. } => c = scan.counts,
        }
        c
    }

    pub fn name(&self) -> &'static str {
        match self {
            Source::PowersOfTwo(_) => "powers2",
            Source::Fibonacci(_) => "fibonacci",
            Source::File { .. } => "file",
        }
    }
}

/// Observed counts and the test against Benford.
#[derive(Clone, Debug)]
pub struct Analysis {
    /// counts[d - 1] for digits 1-9.
    pub counts: [u64; 9],
    pub total: u64,
    pub chi_square: f64,
    /// None below `MIN_FOR_TEST` numbers.
    pub p_value: Option<f64>,
}

pub fn analyze(counts: [u64; 9]) -> Analysis {
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
        p_value: (total >= MIN_FOR_TEST).then(|| chi_square_p(chi_square, 8)),
    }
}

/// Counts of a list of digits (for tests and small inputs).
pub fn count_digits(digits: &[usize]) -> [u64; 9] {
    let mut c = [0u64; 9];
    for &d in digits {
        if (1..=9).contains(&d) {
            c[d - 1] += 1;
        }
    }
    c
}

/// Title, question, intuition, truth and why, for the report.
pub fn texts(source: &Source, lang: Lang) -> [String; 5] {
    let what_en = match source {
        Source::PowersOfTwo(n) => format!("the powers of two 2^1 .. 2^{n}"),
        Source::Fibonacci(n) => format!("the first {} Fibonacci numbers", grouped_number(*n)),
        Source::File {
            name,
            column: Some(c),
            ..
        } => format!("column {c} of {name}"),
        Source::File { name, .. } => format!("the numbers in {name}"),
    };
    let what_ja = match source {
        Source::PowersOfTwo(n) => format!("2のべき乗 2^1 .. 2^{n}"),
        Source::Fibonacci(n) => format!("最初の{}個のフィボナッチ数", grouped_number(*n)),
        Source::File {
            name,
            column: Some(c),
            ..
        } => format!("{name}の{c}列目"),
        Source::File { name, .. } => format!("{name}に含まれる数値"),
    };
    match lang {
        Lang::En => [
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
        Lang::Ja => [
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
