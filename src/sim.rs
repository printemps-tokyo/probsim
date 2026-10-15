//! Running a paradox many times and recording how the estimate settles.

use crate::paradox::{Measure, Paradox};
use crate::rng::Rng;
use crate::stats::{normal_interval, wilson, Running, Z95};

/// The estimate after `n` counted trials.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Checkpoint {
    pub n: u64,
    pub estimate: f64,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub paradox: Paradox,
    pub seed: u64,
    /// Trials run, including any that did not count.
    pub attempted: u64,
    /// Trials that counted.
    pub counted: u64,
    /// For a probability: how many counted trials were successes.
    pub successes: u64,
    /// The running mean and variance of the counted trials.
    pub running: Running,
    /// For an average of whole numbers (the coupon collector's draws): how
    /// many trials gave each value, for quantiles.
    pub distribution: Option<std::collections::BTreeMap<u64, u64>>,
    pub estimate: f64,
    pub standard_error: f64,
    /// 95% confidence interval, or None when there are too few trials for
    /// an interval for an average to mean much.
    pub ci95: Option<(f64, f64)>,
    pub theory: f64,
    pub checkpoints: Vec<Checkpoint>,
}

impl Outcome {
    /// The interval at confidence z: Wilson for a probability, normal for an
    /// average (None below `stats::MIN_FOR_MEAN` trials).
    pub fn interval(&self, z: f64) -> Option<(f64, f64)> {
        if self.counted == 0 {
            // Nothing to estimate from (no trial matched the condition).
            return None;
        }
        match self.paradox.measure() {
            Measure::Probability => Some(wilson(self.successes, self.counted, z)),
            Measure::Mean => normal_interval(&self.running, z),
        }
    }

    /// Whether the exact answer is inside the interval at confidence z, or
    /// None when there is no interval to judge by.
    pub fn consistent(&self, z: f64) -> Option<bool> {
        self.interval(z)
            .map(|(lo, hi)| (lo..=hi).contains(&self.theory))
    }
}

/// About `points` trial counts from 1 to `total`, evenly spaced on a log
/// scale, always ending at `total`.
pub fn checkpoint_counts(total: u64, points: usize) -> Vec<u64> {
    if total == 0 || points == 0 {
        return Vec::new();
    }
    let mut out: Vec<u64> = (0..points)
        .map(|i| {
            let f = if points == 1 {
                1.0
            } else {
                i as f64 / (points - 1) as f64
            };
            ((total as f64).powf(f).round() as u64).clamp(1, total)
        })
        .collect();
    out.push(total);
    out.dedup();
    out
}

/// A seed for the i-th of several runs started from one seed (splitmix64 of
/// seed + i), so `probsim all` gives each paradox its own random stream.
pub fn derived_seed(seed: u64, i: u64) -> u64 {
    let mut z = seed.wrapping_add(i.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    // 53 bits, so the seed survives a trip through JSON numbers.
    (z ^ (z >> 31)) & ((1 << 53) - 1)
}

/// Run `trials` trials of `p`. Checkpoints are log spaced over the counted
/// trials. When some trials may not count, a first pass with the same seed
/// finds how many will, so the checkpoints still spread over the whole run.
pub fn simulate(p: &Paradox, trials: u64, seed: u64, points: usize) -> Outcome {
    simulate_known(p, p.theory(), trials, seed, points)
}

/// `simulate` with the exact answer already computed, for studies that run
/// the same paradox many times.
pub fn simulate_known(p: &Paradox, theory: f64, trials: u64, seed: u64, points: usize) -> Outcome {
    let mut scratch = Vec::new();
    let counted_total = if p.may_reject() {
        let mut rng = Rng::new(seed);
        (0..trials)
            .filter(|_| p.trial(&mut rng, &mut scratch).is_some())
            .count() as u64
    } else {
        trials
    };
    let mut rng = Rng::new(seed);
    let mut running = Running::default();
    let mut successes = 0u64;
    let mut distribution =
        (p.measure() == Measure::Mean).then(std::collections::BTreeMap::<u64, u64>::new);
    let probability = p.measure() == Measure::Probability;
    let estimate = |running: &Running, successes: u64| {
        if probability && running.n > 0 {
            successes as f64 / running.n as f64
        } else {
            running.mean()
        }
    };
    let marks = checkpoint_counts(counted_total, points);
    let mut next_mark = 0;
    let mut checkpoints: Vec<Checkpoint> = Vec::with_capacity(marks.len());
    for _ in 0..trials {
        if let Some(x) = p.trial(&mut rng, &mut scratch) {
            running.push(x);
            if probability && x == 1.0 {
                successes += 1;
            }
            if let Some(d) = distribution.as_mut() {
                *d.entry(x as u64).or_insert(0) += 1;
            }
            if next_mark < marks.len() && running.n >= marks[next_mark] {
                next_mark += 1;
                checkpoints.push(Checkpoint {
                    n: running.n,
                    estimate: estimate(&running, successes),
                });
            }
        }
    }
    let n = running.n;
    let est = estimate(&running, successes);
    let standard_error = match p.measure() {
        Measure::Probability if n > 0 => (est * (1.0 - est) / n as f64).sqrt(),
        Measure::Mean if n >= 2 => (running.variance() / n as f64).sqrt(),
        _ => f64::NAN,
    };
    let mut o = Outcome {
        paradox: p.clone(),
        seed,
        attempted: trials,
        counted: n,
        successes,
        running,
        distribution,
        estimate: est,
        standard_error,
        ci95: None,
        theory,
        checkpoints,
    };
    o.ci95 = o.interval(Z95);
    o
}

/// The smallest value v with at least a share `q` of trials at or below it,
/// from a distribution of whole numbers.
pub fn quantile(d: &std::collections::BTreeMap<u64, u64>, q: f64) -> Option<u64> {
    let total: u64 = d.values().sum();
    if total == 0 {
        return None;
    }
    let need = (q * total as f64).ceil().max(1.0) as u64;
    let mut acc = 0;
    for (&v, &c) in d {
        acc += c;
        if acc >= need {
            return Some(v);
        }
    }
    d.keys().next_back().copied()
}
