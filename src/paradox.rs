//! The paradoxes: their parameters, exact answers and one random trial.
//! The words around them are in `texts`, the longer exact answers in `exact`.

use std::fmt::Write as _;
use std::sync::Arc;

use crate::exact;
pub use crate::exact::{birthday_probability, secretary_best_skip, secretary_success};
use crate::rng::Rng;
pub use crate::texts::{num, pct, Texts};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Ja,
    En,
}

impl Lang {
    /// Japanese when the first set of LC_ALL, LC_MESSAGES and LANG starts with "ja".
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Lang {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|k| get(k))
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        if locale.starts_with("ja") {
            Lang::Ja
        } else {
            Lang::En
        }
    }
}

/// What a trial produces and how the estimate is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// Each trial is 1 (it happened) or 0; the estimate is a probability.
    Probability,
    /// Each trial is a number; the estimate is its mean.
    Mean,
}

/// Unequal chances (of birthdays on each day, of each gacha item),
/// normalized, with the running sums for sampling and a cache for the exact
/// answers that take a while (the gacha integral, inclusion-exclusion).
#[derive(Clone, Debug)]
pub struct Weights {
    pub probs: Vec<f64>,
    cumulative: Vec<f64>,
    mean: std::sync::OnceLock<Option<f64>>,
    within: std::sync::OnceLock<(u64, f64)>,
    shared: std::sync::OnceLock<(u32, f64)>,
}

impl PartialEq for Weights {
    fn eq(&self, other: &Self) -> bool {
        self.probs == other.probs
    }
}

/// The smallest ratio of two positive weights accepted: below it, the rare
/// item's share of the running sums can vanish in rounding, so it would never
/// be drawn.
pub const MIN_WEIGHT_RATIO: f64 = 1e-12;

impl Weights {
    /// From non-negative weights with a positive sum.
    pub fn new(raw: &[f64]) -> Result<Weights, String> {
        if raw.is_empty() {
            return Err("no weights given".into());
        }
        if raw.iter().any(|w| !w.is_finite() || *w < 0.0) {
            return Err("weights must be non-negative numbers".into());
        }
        let total: f64 = raw.iter().sum();
        if !(total > 0.0 && total.is_finite()) {
            return Err("weights must add up to a positive number".into());
        }
        let max = raw.iter().copied().fold(0.0, f64::max);
        if raw.iter().any(|&w| w > 0.0 && w / max < MIN_WEIGHT_RATIO) {
            return Err(format!(
                "weights are too uneven: the smallest positive one must be at least {MIN_WEIGHT_RATIO:e} of the largest"
            ));
        }
        let probs: Vec<f64> = raw.iter().map(|w| w / total).collect();
        let mut acc = 0.0;
        let cumulative: Vec<f64> = probs
            .iter()
            .map(|p| {
                acc += p;
                acc
            })
            .collect();
        Ok(Weights {
            probs,
            cumulative,
            mean: Default::default(),
            within: Default::default(),
            shared: Default::default(),
        })
    }

    /// An index drawn with these chances (binary search on the running sums).
    pub fn sample(&self, rng: &mut Rng) -> usize {
        let u = rng.f64();
        let i = self.cumulative.partition_point(|&c| c <= u);
        // Rounding can leave the last sum a hair under 1; never return an
        // index with no chance, and never run past the end.
        let mut i = i.min(self.probs.len() - 1);
        while self.probs[i] == 0.0 && i > 0 {
            i -= 1;
        }
        i
    }

    pub fn len(&self) -> usize {
        self.probs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.probs.is_empty()
    }

    /// Whether all chances are equal.
    pub fn uniform(&self) -> bool {
        let first = self.probs[0];
        self.probs.iter().all(|&p| (p - first).abs() <= 1e-15)
    }

    /// The expected draws to collect every item, computed once (None if the
    /// integral did not converge).
    pub fn coupon_mean(&self) -> Option<f64> {
        *self
            .mean
            .get_or_init(|| exact::coupon_mean_weighted(&self.probs))
    }

    /// P(a shared birthday among `people`) with these day chances, cached for
    /// the first group size asked.
    pub fn birthday(&self, people: u32) -> f64 {
        let &(n, p) = self.shared.get_or_init(|| {
            (
                people,
                exact::birthday_probability_weighted(people, &self.probs),
            )
        });
        if n == people {
            p
        } else {
            exact::birthday_probability_weighted(people, &self.probs)
        }
    }

    /// The chance of collecting every item within `draws`, cached for the
    /// first budget asked.
    pub fn complete_within(&self, draws: u64) -> f64 {
        let &(b, p) = self.within.get_or_init(|| {
            (
                draws,
                exact::coupon_complete_within_weighted(&self.probs, draws),
            )
        });
        if b == draws {
            p
        } else {
            exact::coupon_complete_within_weighted(&self.probs, draws)
        }
    }
}

/// The equal-rate chance of completing within a budget, remembered per
/// (items, budget) on this thread: the Markov chain costs items x budget, and
/// a report asks for the exact answer more than once.
fn complete_within_equal_cached(items: u32, draws: u64) -> f64 {
    thread_local! {
        static MEMO: std::cell::RefCell<std::collections::HashMap<(u32, u64), f64>> = Default::default();
    }
    MEMO.with(|m| {
        *m.borrow_mut()
            .entry((items, draws))
            .or_insert_with(|| exact::coupon_complete_within_equal(items, draws))
    })
}

/// Roughly how many basic steps computing the exact answer takes, so that
/// cost guards can count it too.
pub fn theory_cost(p: &Paradox) -> f64 {
    match p {
        Paradox::Coupon {
            items,
            weights: None,
            budget: Some(b),
        } => *items as f64 * *b as f64,
        Paradox::Coupon {
            weights: Some(w),
            budget: Some(_),
            ..
        } => (1u64 << w.len()) as f64 * w.len() as f64,
        Paradox::Birthday {
            people,
            weights: Some(w),
            ..
        } => *people as f64 * w.len() as f64,
        _ => 0.0,
    }
}

/// How a "random chord" is drawn, in Bertrand's paradox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chord {
    /// Two random points on the circle.
    Endpoints,
    /// A random point on a random radius, the chord at right angles to it.
    Radius,
    /// A random point in the disk as the chord's midpoint.
    Midpoint,
}

impl Chord {
    pub fn name(self) -> &'static str {
        match self {
            Chord::Endpoints => "endpoints",
            Chord::Radius => "radius",
            Chord::Midpoint => "midpoint",
        }
    }
}

/// A sequence of coin flips for Penney's game, true for heads.
pub fn coins(s: &str) -> Result<Vec<bool>, String> {
    if s.is_empty() || s.len() > 12 {
        return Err("a coin sequence is 1 to 12 flips of H and T".into());
    }
    s.chars()
        .map(|c| match c.to_ascii_uppercase() {
            'H' => Ok(true),
            'T' => Ok(false),
            _ => Err(format!("{s:?}: use H and T only")),
        })
        .collect()
}

pub fn coins_text(c: &[bool]) -> String {
    c.iter().map(|&h| if h { 'H' } else { 'T' }).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub enum Paradox {
    MontyHall {
        doors: u32,
        host_knows: bool,
    },
    /// With `weights`, `days` is their number.
    Birthday {
        people: u32,
        days: u32,
        weights: Option<Arc<Weights>>,
    },
    /// With `budget`, the question is the chance of completing within it.
    Coupon {
        items: u32,
        weights: Option<Arc<Weights>>,
        budget: Option<u64>,
    },
    Secretary {
        candidates: u32,
        skip: u32,
    },
    /// A needle of `length` on lines `spacing` apart; only the ratio matters.
    Buffon {
        length: f64,
        spacing: f64,
    },
    BaseRate {
        prevalence: f64,
        sensitivity: f64,
        specificity: f64,
    },
    Penney {
        first: Vec<bool>,
        second: Vec<bool>,
    },
    /// Two children; at least one is a boy (born on a Tuesday, if `tuesday`).
    BoyGirl {
        tuesday: bool,
    },
    Bertrand {
        method: Chord,
    },
    /// Flip until heads; k flips pay 2^k, but never more than `bank`.
    StPetersburg {
        bank: u64,
    },
    /// Recoveries out of patients for treatment A and B in two groups:
    /// `[(A1 successes, A1 patients), (A2 ..), (B1 ..), (B2 ..)]`. Each trial
    /// is one patient on A minus one on B (1, 0 or -1). Crude: each from its
    /// own arm's mix of groups; adjusted: both from one group drawn from the
    /// pooled mix.
    Simpson {
        counts: [(u32, u32); 4],
        adjusted: bool,
    },
    /// Bet one unit at a time from `start`, winning each bet with `win`,
    /// until `goal` or ruin.
    Gambler {
        start: u32,
        goal: u32,
        win: f64,
    },
}

/// The St. Petersburg game with a bank: K = floor(log2 bank), the largest
/// number of flips whose prize 2^K the bank still pays in full.
pub fn st_petersburg_k(bank: u64) -> u32 {
    63 - bank.max(1).leading_zeros()
}

/// Gambler's ruin: the chance of reaching `goal` from `start` before 0,
/// winning each unit bet with probability `p` (Feller, ch. XIV). Written
/// with expm1 so it stays accurate and finite for long games and for p near
/// 1/2: with r = q/p, (1 - r^i) / (1 - r^N), and i/N for a fair game.
pub fn gambler_success(start: u32, goal: u32, p: f64) -> f64 {
    let (i, n) = (f64::from(start), f64::from(goal));
    let q = 1.0 - p;
    if p == q {
        return i / n;
    }
    let lr = (q / p).ln();
    if lr > 0.0 {
        // r > 1: r^(i-N) (1 - r^-i) / (1 - r^-N), all terms at most 1.
        ((i - n) * lr).exp() * (-(-i * lr).exp_m1()) / (-(-n * lr).exp_m1())
    } else {
        (i * lr).exp_m1() / (n * lr).exp_m1()
    }
}

/// The expected number of bets in gambler's ruin: i (N - i) for a fair game,
/// (i - N P) / (q - p) otherwise, P the chance of reaching N. Near a fair
/// game that difference cancels to nothing, so while N |ln(q/p)| is below
/// 1e-3 the fair value is used: there the two differ by well under 0.1%.
/// (Used to cost a run; the tests check it against the first-step equations.)
pub fn gambler_duration(start: u32, goal: u32, p: f64) -> f64 {
    let (i, n) = (f64::from(start), f64::from(goal));
    let q = 1.0 - p;
    let fair = i * (n - i);
    if p == q || n * (q / p).ln().abs() < 1e-3 {
        return fair;
    }
    let d = (i - n * gambler_success(start, goal, p)) / (q - p);
    if d.is_finite() && d > 0.0 {
        d
    } else {
        fair
    }
}

/// Recovery rates of a Simpson table: (A1, A2, B1, B2).
fn rates(counts: &[(u32, u32); 4]) -> [f64; 4] {
    counts.map(|(s, n)| f64::from(s) / f64::from(n))
}

/// The story of one trial, for --explain: a buffer and its language.
pub type Story<'a> = Option<(&'a mut String, Lang)>;

/// A uniform direction without using pi: a uniform point in the square kept
/// when it falls in the unit disk, scaled to length 1.
fn direction(rng: &mut Rng) -> (f64, f64) {
    loop {
        let (x, y) = (2.0 * rng.f64() - 1.0, 2.0 * rng.f64() - 1.0);
        let r2 = x * x + y * y;
        if r2 > 0.0 && r2 <= 1.0 {
            let r = r2.sqrt();
            return (x / r, y / r);
        }
    }
}

/// A uniform point in the unit disk (rejection from the square).
fn point_in_disk(rng: &mut Rng) -> (f64, f64) {
    loop {
        let (x, y) = (2.0 * rng.f64() - 1.0, 2.0 * rng.f64() - 1.0);
        if x * x + y * y <= 1.0 {
            return (x, y);
        }
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const DAYS_JA: [&str; 7] = ["月曜", "火曜", "水曜", "木曜", "金曜", "土曜", "日曜"];

impl Paradox {
    pub fn name(&self) -> &'static str {
        match self {
            Paradox::MontyHall { .. } => "monty-hall",
            Paradox::Birthday { .. } => "birthday",
            Paradox::Coupon { .. } => "coupon",
            Paradox::Secretary { .. } => "secretary",
            Paradox::Buffon { .. } => "buffon",
            Paradox::BaseRate { .. } => "base-rate",
            Paradox::Penney { .. } => "penney",
            Paradox::BoyGirl { .. } => "boy-girl",
            Paradox::Bertrand { .. } => "bertrand",
            Paradox::StPetersburg { .. } => "st-petersburg",
            Paradox::Simpson { .. } => "simpson",
            Paradox::Gambler { .. } => "gambler",
        }
    }

    /// Whether every trial is a whole number of at least 0, so the trials
    /// can be tallied for quantiles (draws to complete, St. Petersburg prizes).
    pub fn whole_trials(&self) -> bool {
        matches!(
            self,
            Paradox::Coupon { budget: None, .. } | Paradox::StPetersburg { .. }
        )
    }

    /// The fewest trials for which an interval is drawn and judged. For an
    /// average that is `stats::MIN_FOR_MEAN`, except for the St. Petersburg
    /// game: its largest prizes are so rare that, until the rarest one is
    /// expected about 30 times (30 x 2^K games), the sample variance misses
    /// them and the normal interval is wrong far more often than it says.
    pub fn min_for_interval(&self) -> u64 {
        match self {
            Paradox::StPetersburg { bank } => {
                30u64.saturating_mul(1u64 << st_petersburg_k(*bank).min(62))
            }
            _ => crate::stats::MIN_FOR_MEAN,
        }
    }

    /// Whether some trials may not count: the question is conditional (a
    /// random Monty Hall host, a positive test, a family with a boy).
    pub fn may_reject(&self) -> bool {
        matches!(
            self,
            Paradox::MontyHall {
                host_knows: false,
                ..
            } | Paradox::BaseRate { .. }
                | Paradox::BoyGirl { .. }
        )
    }

    pub fn measure(&self) -> Measure {
        match self {
            Paradox::Coupon { budget: None, .. }
            | Paradox::StPetersburg { .. }
            | Paradox::Simpson { .. } => Measure::Mean,
            _ => Measure::Probability,
        }
    }

    /// The exact answer.
    pub fn theory(&self) -> f64 {
        match self {
            Paradox::MontyHall { doors, host_knows } => {
                if *host_knows {
                    (doors - 1) as f64 / *doors as f64
                } else {
                    // Given that a random host revealed only goats: 1/2 for any n.
                    0.5
                }
            }
            Paradox::Birthday {
                people,
                days,
                weights,
            } => match weights {
                Some(w) => w.birthday(*people),
                None => exact::birthday_probability(*people, *days),
            },
            Paradox::Coupon {
                items,
                weights,
                budget,
            } => match (weights, budget) {
                (None, None) => exact::coupon_mean_equal(*items),
                (Some(w), None) => w.coupon_mean().unwrap_or(f64::NAN),
                (None, Some(b)) => complete_within_equal_cached(*items, *b),
                (Some(w), Some(b)) => w.complete_within(*b),
            },
            Paradox::Secretary { candidates, skip } => exact::secretary_success(*candidates, *skip),
            Paradox::Buffon { length, spacing } => 2.0 * (length / spacing) / std::f64::consts::PI,
            Paradox::BaseRate {
                prevalence: p,
                sensitivity: se,
                specificity: sp,
            } => {
                let positive = p * se + (1.0 - p) * (1.0 - sp);
                p * se / positive
            }
            Paradox::Penney { first, second } => exact::penney_second_wins(first, second),
            Paradox::BoyGirl { tuesday } => {
                if *tuesday {
                    13.0 / 27.0
                } else {
                    1.0 / 3.0
                }
            }
            Paradox::Bertrand { method } => match method {
                Chord::Endpoints => 1.0 / 3.0,
                Chord::Radius => 0.5,
                Chord::Midpoint => 0.25,
            },
            Paradox::StPetersburg { bank } => {
                // k = 1..K flips pay 2^k with chance 2^-k: 1 each. Every
                // longer game pays the bank, together with chance 2^-K.
                let k = st_petersburg_k(*bank);
                f64::from(k) + *bank as f64 / 2f64.powi(k as i32)
            }
            Paradox::Simpson { counts, adjusted } => {
                let [a1, a2, b1, b2] = rates(counts);
                if *adjusted {
                    let g1 = f64::from(counts[0].1 + counts[2].1);
                    let g2 = f64::from(counts[1].1 + counts[3].1);
                    (g1 * (a1 - b1) + g2 * (a2 - b2)) / (g1 + g2)
                } else {
                    let a =
                        f64::from(counts[0].0 + counts[1].0) / f64::from(counts[0].1 + counts[1].1);
                    let b =
                        f64::from(counts[2].0 + counts[3].0) / f64::from(counts[2].1 + counts[3].1);
                    a - b
                }
            }
            Paradox::Gambler { start, goal, win } => gambler_success(*start, *goal, *win),
        }
    }

    /// The exact variance of one trial: p(1 - p) for a probability; for the
    /// equal-rate coupon collector n^2 * sum 1/k^2 - n * H_n; NaN when not
    /// known in closed form (unequal rates).
    pub fn theory_variance(&self) -> f64 {
        match self {
            Paradox::Coupon {
                items,
                weights: None,
                budget: None,
            } => {
                let n = *items as f64;
                let squares: f64 = (1..=*items as u64)
                    .rev()
                    .map(|k| 1.0 / (k * k) as f64)
                    .sum();
                n * n * squares - exact::coupon_mean_equal(*items)
            }
            Paradox::Coupon { budget: None, .. } => f64::NAN,
            Paradox::StPetersburg { bank } => {
                // E[X^2] = sum_{k<=K} 4^k 2^-k + bank^2 2^-K.
                let k = st_petersburg_k(*bank);
                let two_k = 2f64.powi(k as i32);
                let w = *bank as f64;
                let second = 2.0 * two_k - 2.0 + w * w / two_k;
                let mean = self.theory();
                second - mean * mean
            }
            Paradox::Simpson { counts, adjusted } => {
                // X = A - B in {-1, 0, 1}: E[X^2] = P(A != B).
                let [a1, a2, b1, b2] = rates(counts);
                let differ = |a: f64, b: f64| a * (1.0 - b) + b * (1.0 - a);
                let second = if *adjusted {
                    let g1 = f64::from(counts[0].1 + counts[2].1);
                    let g2 = f64::from(counts[1].1 + counts[3].1);
                    (g1 * differ(a1, b1) + g2 * differ(a2, b2)) / (g1 + g2)
                } else {
                    let a =
                        f64::from(counts[0].0 + counts[1].0) / f64::from(counts[0].1 + counts[1].1);
                    let b =
                        f64::from(counts[2].0 + counts[3].0) / f64::from(counts[2].1 + counts[3].1);
                    differ(a, b)
                };
                let mean = self.theory();
                second - mean * mean
            }
            _ => {
                let p = self.theory();
                p * (1.0 - p)
            }
        }
    }

    /// The exact answer as a fraction or formula, when that reads better.
    pub fn exact(&self) -> Option<String> {
        match self {
            Paradox::MontyHall { doors, host_knows } => Some(if *host_knows {
                format!("{}/{}", doors - 1, doors)
            } else {
                "1/2".to_string()
            }),
            Paradox::Coupon {
                items,
                weights: None,
                budget: None,
            } => Some(format!("{items} x H_{items}")),
            Paradox::Buffon { .. } => Some("2l/(pi t)".to_string()),
            Paradox::BaseRate { .. } => Some("p se / (p se + (1 - p)(1 - sp))".to_string()),
            Paradox::Penney { first, second } => {
                let l = exact::leading_number;
                let for_b = l(first, first) - l(first, second);
                let for_a = l(second, second) - l(second, first);
                let g = gcd(for_b, for_b + for_a).max(1);
                Some(format!("{}/{}", for_b / g, (for_b + for_a) / g))
            }
            Paradox::BoyGirl { tuesday } => {
                Some(if *tuesday { "13/27" } else { "1/3" }.to_string())
            }
            Paradox::Bertrand { method } => Some(
                match method {
                    Chord::Endpoints => "1/3",
                    Chord::Radius => "1/2",
                    Chord::Midpoint => "1/4",
                }
                .to_string(),
            ),
            Paradox::StPetersburg { bank } => {
                let k = st_petersburg_k(*bank);
                Some(format!("{k} + {bank}/2^{k}"))
            }
            Paradox::Simpson {
                counts,
                adjusted: false,
            } => Some(format!(
                "{}/{} - {}/{}",
                counts[0].0 + counts[1].0,
                counts[0].1 + counts[1].1,
                counts[2].0 + counts[3].0,
                counts[2].1 + counts[3].1
            )),
            Paradox::Gambler { start, goal, win } if *win == 0.5 => {
                let g = gcd(u64::from(*start), u64::from(*goal)).max(1);
                Some(format!(
                    "{}/{}",
                    u64::from(*start) / g,
                    u64::from(*goal) / g
                ))
            }
            Paradox::Gambler { .. } => Some("(1 - r^i) / (1 - r^N), r = q/p".to_string()),
            _ => None,
        }
    }

    /// Parameters, for display and JSON.
    pub fn params(&self) -> Vec<(&'static str, String)> {
        match self {
            Paradox::MontyHall { doors, host_knows } => vec![
                ("doors", doors.to_string()),
                (
                    "host",
                    if *host_knows { "knows" } else { "random" }.to_string(),
                ),
            ],
            Paradox::Birthday {
                people,
                days,
                weights,
            } => {
                let mut v = vec![("people", people.to_string()), ("days", days.to_string())];
                if weights.is_some() {
                    v.push(("dayWeights", "given".to_string()));
                }
                v
            }
            Paradox::Coupon {
                items,
                weights,
                budget,
            } => {
                let mut v = vec![("items", items.to_string())];
                if weights.is_some() {
                    v.push(("weights", "given".to_string()));
                }
                if let Some(b) = budget {
                    v.push(("budget", b.to_string()));
                }
                v
            }
            Paradox::Secretary { candidates, skip } => vec![
                ("candidates", candidates.to_string()),
                ("skip", skip.to_string()),
            ],
            Paradox::Buffon { length, spacing } => {
                vec![
                    ("length", format!("{length:e}")),
                    ("spacing", format!("{spacing:e}")),
                ]
            }
            Paradox::BaseRate {
                prevalence,
                sensitivity,
                specificity,
            } => vec![
                ("prevalence", prevalence.to_string()),
                ("sensitivity", sensitivity.to_string()),
                ("specificity", specificity.to_string()),
            ],
            Paradox::Penney { first, second } => {
                vec![("first", coins_text(first)), ("second", coins_text(second))]
            }
            Paradox::BoyGirl { tuesday } => vec![(
                "condition",
                if *tuesday { "tuesday-boy" } else { "boy" }.to_string(),
            )],
            Paradox::Bertrand { method } => vec![("method", method.name().to_string())],
            Paradox::StPetersburg { bank } => vec![("bank", bank.to_string())],
            Paradox::Simpson { counts, adjusted } => vec![
                (
                    "counts",
                    counts
                        .iter()
                        .map(|(s, n)| format!("{s}/{n}"))
                        .collect::<Vec<_>>()
                        .join(","),
                ),
                (
                    "comparison",
                    if *adjusted { "adjusted" } else { "crude" }.to_string(),
                ),
            ],
            Paradox::Gambler { start, goal, win } => vec![
                ("start", start.to_string()),
                ("goal", goal.to_string()),
                ("winProb", win.to_string()),
            ],
        }
    }

    /// One trial, or None when the trial does not count (see `may_reject`).
    pub fn trial(&self, rng: &mut Rng, scratch: &mut Vec<u32>) -> Option<f64> {
        self.run(rng, scratch, None)
    }

    /// One trial with its story told: the same code and the same random
    /// numbers as `trial`, so the story is exactly what was simulated.
    pub fn explain(
        &self,
        rng: &mut Rng,
        scratch: &mut Vec<u32>,
        lang: Lang,
    ) -> (Option<f64>, String) {
        let mut s = String::new();
        let result = self.run(rng, scratch, Some((&mut s, lang)));
        (result, s)
    }

    fn run(&self, rng: &mut Rng, scratch: &mut Vec<u32>, mut story: Story) -> Option<f64> {
        match self {
            Paradox::MontyHall { doors, host_knows } => {
                let n = *doors as u64;
                let car = rng.below(n);
                let pick = rng.below(n);
                // The host opens every door except your pick and one other:
                // `kept` is the one he leaves closed for you to switch to.
                // A host who knows keeps the car closed when you missed it;
                // otherwise he keeps a door chosen at random.
                let other_than_pick = |k: u64| if k >= pick { k + 1 } else { k };
                let kept = if *host_knows && car != pick {
                    car
                } else {
                    other_than_pick(rng.below(n - 1))
                };
                let revealed = car != pick && car != kept;
                let wins = kept == car;
                if let Some((s, lang)) = story.as_mut() {
                    let (c, p, k, o) = (car + 1, pick + 1, kept + 1, n - 2);
                    let _ = match (lang, revealed) {
                        (Lang::En, true) => write!(
                            s,
                            "car behind door {c}, you pick door {p}; the host, opening doors at random, \
                             leaves door {k} closed and reveals the car: this game does not count"
                        ),
                        (Lang::En, false) => write!(
                            s,
                            "car behind door {c}, you pick door {p}; the host leaves door {k} closed and \
                             opens {}; switching to door {k} {}",
                            if o == 1 { "the other door".to_string() } else { format!("the other {o}") },
                            if wins { "wins" } else { "loses" }
                        ),
                        (Lang::Ja, true) => write!(
                            s,
                            "車はドア{c}、あなたはドア{p}を選択。でたらめに開ける司会者がドア{k}を残して開けたら\
                             車が出てしまった: このゲームは数えない"
                        ),
                        (Lang::Ja, false) => write!(
                            s,
                            "車はドア{c}、あなたはドア{p}を選択。司会者はドア{k}を残して他の{o}枚を開ける。\
                             ドア{k}に変えると{}",
                            if wins { "当たり" } else { "はずれ" }
                        ),
                    };
                }
                if revealed {
                    // Only possible with a random host: not the situation asked about.
                    return None;
                }
                Some(if wins { 1.0 } else { 0.0 })
            }
            Paradox::Birthday {
                people,
                days,
                weights,
            } => {
                // scratch[d] holds the generation that last saw day d, and
                // scratch[days] the current generation, so the days are not
                // cleared on every trial.
                let d_len = *days as usize;
                if scratch.len() != d_len + 1 {
                    scratch.clear();
                    scratch.resize(d_len + 1, 0);
                }
                let mut generation = scratch[d_len].wrapping_add(1);
                if generation == 0 {
                    scratch.iter_mut().for_each(|x| *x = 0);
                    generation = 1;
                }
                scratch[d_len] = generation;
                for person in 1..=*people {
                    let d = match weights {
                        Some(w) => w.sample(rng),
                        None => rng.below(*days as u64) as usize,
                    };
                    if scratch[d] == generation {
                        if let Some((s, lang)) = story.as_mut() {
                            let _ = match lang {
                                Lang::En => write!(
                                    s,
                                    "person {person} has day {} like someone before: a shared birthday",
                                    d + 1
                                ),
                                Lang::Ja => write!(s, "{person}人目の誕生日{}日目は先に来た人と同じ: 一致あり", d + 1),
                            };
                        }
                        return Some(1.0);
                    }
                    scratch[d] = generation;
                }
                if let Some((s, lang)) = story.as_mut() {
                    let _ = match lang {
                        Lang::En => write!(s, "all {people} birthdays differ: no shared birthday"),
                        Lang::Ja => write!(s, "{people}人全員の誕生日が異なる: 一致なし"),
                    };
                }
                Some(0.0)
            }
            Paradox::Coupon {
                items,
                weights,
                budget,
            } => {
                // Generation stamps, as for birthdays: scratch[i] is the
                // collector that last found item i, scratch[items] the
                // current one, so the items are not cleared on every trial.
                let n_items = *items as usize;
                if scratch.len() != n_items + 1 {
                    scratch.clear();
                    scratch.resize(n_items + 1, 0);
                }
                let mut generation = scratch[n_items].wrapping_add(1);
                if generation == 0 {
                    scratch.iter_mut().for_each(|x| *x = 0);
                    generation = 1;
                }
                scratch[n_items] = generation;
                let (mut missing, mut draws) = (*items, 0u64);
                let mut seen: Vec<u64> = Vec::new();
                let mut last = 0;
                while missing > 0 && budget.is_none_or(|b| draws < b) {
                    draws += 1;
                    let i = match weights {
                        Some(w) => w.sample(rng),
                        None => rng.below(*items as u64) as usize,
                    };
                    if story.is_some() && seen.len() < 40 {
                        seen.push(i as u64 + 1);
                    }
                    if scratch[i] != generation {
                        scratch[i] = generation;
                        missing -= 1;
                        last = i + 1;
                    }
                }
                if let Some((s, lang)) = story.as_mut() {
                    let list = seen
                        .iter()
                        .map(u64::to_string)
                        .collect::<Vec<_>>()
                        .join(" ");
                    let more = if draws as usize > seen.len() {
                        " ..."
                    } else {
                        ""
                    };
                    let _ = match (lang, missing) {
                        (Lang::En, 0) => write!(
                            s,
                            "drew {list}{more}: complete after {draws} draws, the last new item was {last}"
                        ),
                        (Lang::En, m) => write!(s, "drew {list}{more}: {m} items still missing after {draws} draws"),
                        (Lang::Ja, 0) => write!(
                            s,
                            "{list}{more} と引いて{draws}回でコンプリート。最後にそろったのは{last}番"
                        ),
                        (Lang::Ja, m) => write!(s, "{list}{more} と引いて{draws}回、まだ{m}種足りない"),
                    };
                }
                Some(match budget {
                    Some(_) => {
                        if missing == 0 {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    None => draws as f64,
                })
            }
            Paradox::Secretary { candidates, skip } => {
                // Scores 0..n in random order; n-1 is the best.
                scratch.clear();
                scratch.extend(0..*candidates);
                rng.shuffle(scratch);
                let k = *skip as usize;
                let bar = scratch[..k].iter().max().copied();
                let hired = scratch[k..]
                    .iter()
                    .position(|&s| bar.is_none_or(|b| s > b))
                    .map(|i| (k + i, scratch[k + i]));
                let best = hired.is_some_and(|(_, s)| s == candidates - 1);
                if let Some((s, lang)) = story.as_mut() {
                    let bar_text = match (*lang, bar) {
                        (Lang::En, Some(b)) => {
                            format!("the first {k} set the bar at score {}", b + 1)
                        }
                        (Lang::En, None) => "no one is skipped".to_string(),
                        (Lang::Ja, Some(b)) => format!("最初の{k}人で基準はスコア{}", b + 1),
                        (Lang::Ja, None) => "見送りなし".to_string(),
                    };
                    let _ = match (*lang, hired) {
                        (Lang::En, Some((i, sc))) => write!(
                            s,
                            "{bar_text}; candidate {} beats it with score {} of {candidates}: {}",
                            i + 1,
                            sc + 1,
                            if best { "the best, hired" } else { "hired, but not the best" }
                        ),
                        (Lang::En, None) => write!(
                            s,
                            "{bar_text}; no one after beats it, because the best was among the first {k}"
                        ),
                        (Lang::Ja, Some((i, sc))) => write!(
                            s,
                            "{bar_text}。{}人目がスコア{}(最高{candidates})で上回り採用: {}",
                            i + 1,
                            sc + 1,
                            if best { "最も優秀な人" } else { "最も優秀な人ではない" }
                        ),
                        (Lang::Ja, None) => {
                            write!(s, "{bar_text}。最も優秀な人が最初の{k}人にいたので、誰も上回れない")
                        }
                    };
                }
                Some(if best { 1.0 } else { 0.0 })
            }
            Paradox::Buffon { length, spacing } => {
                // In units of the line spacing, so only the ratio matters and
                // no product or quotient of the raw inputs can overflow.
                let r = length / spacing;
                // Distance from the needle's center to the nearest line.
                let y = rng.f64() / 2.0;
                // A uniform angle without using pi: a uniform point in the
                // quarter disk has a uniform angle.
                let sin = loop {
                    let (a, b) = (rng.f64(), rng.f64());
                    let r2 = a * a + b * b;
                    if r2 > 0.0 && r2 <= 1.0 {
                        break b / r2.sqrt();
                    }
                };
                let reach = r / 2.0 * sin;
                let crosses = y <= reach;
                if let Some((s, lang)) = story.as_mut() {
                    let _ =
                        match lang {
                            Lang::En => write!(
                            s,
                            "center {y:.3} spacings from a line, the needle reaches {reach:.3}: {}",
                            if crosses { "crosses" } else { "misses" }
                        ),
                            Lang::Ja => write!(
                            s,
                            "中心から線までの距離{y:.3}(間隔を1とする)、針が届く距離{reach:.3}: {}",
                            if crosses { "交わる" } else { "交わらない" }
                        ),
                        };
                }
                Some(if crosses { 1.0 } else { 0.0 })
            }
            Paradox::BaseRate {
                prevalence,
                sensitivity,
                specificity,
            } => {
                let sick = rng.f64() < *prevalence;
                let positive = if sick {
                    rng.f64() < *sensitivity
                } else {
                    rng.f64() >= *specificity
                };
                if let Some((s, lang)) = story.as_mut() {
                    let _ = match (lang, sick, positive) {
                        (Lang::En, _, false) => write!(
                            s,
                            "{} and tests negative: not counted (the question is about positives)",
                            if sick { "sick" } else { "healthy" }
                        ),
                        (Lang::En, true, true) => {
                            write!(s, "sick and tests positive: a true positive")
                        }
                        (Lang::En, false, true) => {
                            write!(s, "healthy but tests positive: a false alarm")
                        }
                        (Lang::Ja, _, false) => write!(
                            s,
                            "{}で陰性: 数えない(問われているのは陽性の人)",
                            if sick { "病気" } else { "健康" }
                        ),
                        (Lang::Ja, true, true) => write!(s, "病気で陽性: 本当の陽性"),
                        (Lang::Ja, false, true) => write!(s, "健康なのに陽性: 誤報"),
                    };
                }
                if !positive {
                    return None;
                }
                Some(if sick { 1.0 } else { 0.0 })
            }
            Paradox::Penney { first, second } => {
                let n = first.len();
                let bits = |c: &[bool]| c.iter().fold(0u32, |acc, &h| (acc << 1) | h as u32);
                let (a, b, mask) = (bits(first), bits(second), (1u32 << n) - 1);
                let (mut window, mut flips) = (0u32, 0u64);
                let mut seen = String::new();
                let second_wins = loop {
                    let h = rng.next_u64() >> 63 == 1;
                    window = ((window << 1) | h as u32) & mask;
                    flips += 1;
                    if story.is_some() && seen.len() < 60 {
                        seen.push(if h { 'H' } else { 'T' });
                    }
                    if flips >= n as u64 {
                        if window == b {
                            break true;
                        }
                        if window == a {
                            break false;
                        }
                    }
                };
                if let Some((s, lang)) = story.as_mut() {
                    let more = if flips as usize > seen.len() {
                        "..."
                    } else {
                        ""
                    };
                    let winner = coins_text(if second_wins { second } else { first });
                    let _ = match lang {
                        Lang::En => write!(
                            s,
                            "{seen}{more}: {winner} comes first after {flips} flips, the {} player wins",
                            if second_wins { "second" } else { "first" }
                        ),
                        Lang::Ja => write!(
                            s,
                            "{seen}{more}: {flips}回目で{winner}が先に出て、{}の勝ち",
                            if second_wins { "後手" } else { "先手" }
                        ),
                    };
                }
                Some(if second_wins { 1.0 } else { 0.0 })
            }
            Paradox::BoyGirl { tuesday } => {
                let child = |rng: &mut Rng| (rng.next_u64() >> 63 == 1, rng.below(7) as usize);
                let kids = [child(rng), child(rng)];
                let counts = |&(boy, day): &(bool, usize)| boy && (!*tuesday || day == 1);
                let qualifies = kids.iter().any(counts);
                let both_boys = kids.iter().all(|k| k.0);
                if let Some((s, lang)) = story.as_mut() {
                    let describe = |&(boy, day): &(bool, usize), lang: Lang| match lang {
                        Lang::En => format!(
                            "a {} born on a {}",
                            if boy { "boy" } else { "girl" },
                            DAYS[day]
                        ),
                        Lang::Ja => format!(
                            "{}生まれの{}",
                            DAYS_JA[day],
                            if boy { "男の子" } else { "女の子" }
                        ),
                    };
                    let (k1, k2) = (describe(&kids[0], *lang), describe(&kids[1], *lang));
                    let _ = match (*lang, qualifies) {
                        (Lang::En, false) => write!(s, "{k1} and {k2}: not counted (no such boy)"),
                        (Lang::En, true) => write!(
                            s,
                            "{k1} and {k2}: {}",
                            if both_boys {
                                "two boys"
                            } else {
                                "not two boys"
                            }
                        ),
                        (Lang::Ja, false) => {
                            write!(s, "{k1}と{k2}: 条件に合う男の子がいないので数えない")
                        }
                        (Lang::Ja, true) => write!(
                            s,
                            "{k1}と{k2}: {}",
                            if both_boys {
                                "2人とも男の子"
                            } else {
                                "2人とも男の子ではない"
                            }
                        ),
                    };
                }
                if !qualifies {
                    return None;
                }
                Some(if both_boys { 1.0 } else { 0.0 })
            }
            Paradox::Bertrand { method } => {
                // Unit circle; the inscribed equilateral triangle's side is
                // sqrt(3), so a chord is longer when its length squared > 3,
                // or equally when its distance from the center is < 1/2.
                let (longer, detail) = match method {
                    Chord::Endpoints => {
                        let (u, v) = (direction(rng), direction(rng));
                        let len2 = (u.0 - v.0).powi(2) + (u.1 - v.1).powi(2);
                        (len2 > 3.0, len2.sqrt())
                    }
                    Chord::Radius => {
                        let d = rng.f64();
                        (d * d < 0.25, 2.0 * (1.0 - d * d).sqrt())
                    }
                    Chord::Midpoint => {
                        let (x, y) = point_in_disk(rng);
                        let r2 = x * x + y * y;
                        (r2 < 0.25, 2.0 * (1.0 - r2).sqrt())
                    }
                };
                if let Some((s, lang)) = story.as_mut() {
                    let _ = match lang {
                        Lang::En => write!(
                            s,
                            "chord of length {detail:.3} (the triangle's side is 1.732): {}",
                            if longer { "longer" } else { "not longer" }
                        ),
                        Lang::Ja => write!(
                            s,
                            "弦の長さ{detail:.3}(正三角形の辺は1.732): {}",
                            if longer { "長い" } else { "長くない" }
                        ),
                    };
                }
                Some(if longer { 1.0 } else { 0.0 })
            }
            Paradox::StPetersburg { bank } => {
                // Count the flips up to the first head. Past 63 flips the
                // prize is beyond any bank this accepts (at most 2^53).
                let mut flips = 1u32;
                while rng.next_u64() & 1 == 0 {
                    flips += 1;
                }
                let prize = if flips >= 63 {
                    *bank
                } else {
                    (1u64 << flips).min(*bank)
                };
                if let Some((s, lang)) = story.as_mut() {
                    let tails = flips - 1;
                    let capped = flips >= 63 || (1u64 << flips) > *bank;
                    let _ = match lang {
                        Lang::En => write!(
                            s,
                            "{tails} tail{} then heads: {flips} flip{}, the prize is {}{}",
                            if tails == 1 { "" } else { "s" },
                            if flips == 1 { "" } else { "s" },
                            crate::output::grouped(prize),
                            if capped { " (the whole bank)" } else { "" }
                        ),
                        Lang::Ja => write!(
                            s,
                            "裏が{tails}回続いて表: {flips}回目で終わり、賞金{}{}",
                            crate::output::grouped(prize),
                            if capped {
                                "(胴元の資金すべて)"
                            } else {
                                ""
                            }
                        ),
                    };
                }
                Some(prize as f64)
            }
            Paradox::Simpson { counts, adjusted } => {
                // Draw a group by the patients in it, then a recovery by the
                // group's counts, with whole-number draws so the chances are
                // exactly the table's.
                let draw_group = |rng: &mut Rng, n1: u32, n2: u32| -> usize {
                    usize::from(rng.below(u64::from(n1) + u64::from(n2)) >= u64::from(n1))
                };
                let recovers =
                    |rng: &mut Rng, (s, n): (u32, u32)| rng.below(u64::from(n)) < u64::from(s);
                let (ga, gb) = if *adjusted {
                    let g = draw_group(rng, counts[0].1 + counts[2].1, counts[1].1 + counts[3].1);
                    (g, g)
                } else {
                    (
                        draw_group(rng, counts[0].1, counts[1].1),
                        draw_group(rng, counts[2].1, counts[3].1),
                    )
                };
                let a = recovers(rng, counts[ga]);
                let b = recovers(rng, counts[2 + gb]);
                let x = i32::from(a) - i32::from(b);
                if let Some((s, lang)) = story.as_mut() {
                    let lang = *lang;
                    let word = |ok: bool| match (lang, ok) {
                        (Lang::En, true) => "recovers",
                        (Lang::En, false) => "does not recover",
                        (Lang::Ja, true) => "回復",
                        (Lang::Ja, false) => "回復せず",
                    };
                    let _ = match lang {
                        Lang::En => write!(
                            s,
                            "A's patient (group {}) {}, B's patient (group {}) {}: {x:+}",
                            ga + 1,
                            word(a),
                            gb + 1,
                            word(b)
                        ),
                        Lang::Ja => write!(
                            s,
                            "Aの患者(グループ{})は{}、Bの患者(グループ{})は{}: {x:+}",
                            ga + 1,
                            word(a),
                            gb + 1,
                            word(b)
                        ),
                    };
                }
                Some(f64::from(x))
            }
            Paradox::Gambler { start, goal, win } => {
                let mut money = *start;
                let mut bets = 0u64;
                let mut path = String::new();
                while money > 0 && money < *goal {
                    bets += 1;
                    let won = rng.f64() < *win;
                    if won {
                        money += 1;
                    } else {
                        money -= 1;
                    }
                    if story.is_some() && bets <= 40 {
                        path.push(if won { 'W' } else { 'L' });
                    }
                }
                let reached = money == *goal;
                if let Some((s, lang)) = story.as_mut() {
                    let more = if bets > 40 { "..." } else { "" };
                    let _ = match lang {
                        Lang::En => write!(
                            s,
                            "{path}{more}: {} after {} bet{}",
                            if reached {
                                format!("reached {goal}")
                            } else {
                                "ruined".to_string()
                            },
                            crate::output::grouped(bets),
                            if bets == 1 { "" } else { "s" }
                        ),
                        Lang::Ja => write!(
                            s,
                            "{path}{more}: {}回賭けて{}",
                            crate::output::grouped(bets),
                            if reached {
                                format!("{goal}に到達")
                            } else {
                                "破産".to_string()
                            }
                        ),
                    };
                }
                Some(if reached { 1.0 } else { 0.0 })
            }
        }
    }

    pub fn texts(&self, lang: Lang) -> Texts {
        crate::texts::texts(self, lang)
    }
}
