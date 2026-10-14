//! Running a paradox many times and recording how the estimate settles.

use crate::paradox::{Measure, Paradox};
use crate::rng::Rng;
use crate::stats::{normal_interval, wilson, Running};

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
    pub estimate: f64,
    pub standard_error: f64,
    /// 95% confidence interval.
    pub ci95: (f64, f64),
    pub theory: f64,
    pub checkpoints: Vec<Checkpoint>,
}

impl Outcome {
    /// Whether the exact answer is inside the interval at confidence z
    /// (`stats::Z95` for the printed 95% interval).
    pub fn consistent(&self, z: f64) -> bool {
        let (lo, hi) = interval(
            self.paradox.measure(),
            self.estimate,
            self.standard_error,
            self.counted,
            z,
        );
        (lo..=hi).contains(&self.theory)
    }
}

/// An interval at confidence z: Wilson for probabilities, normal for means.
fn interval(m: Measure, estimate: f64, se: f64, n: u64, z: f64) -> (f64, f64) {
    match m {
        Measure::Probability => wilson_z(estimate, n, z),
        Measure::Mean => (estimate - z * se, estimate + z * se),
    }
}

fn wilson_z(p: f64, n: u64, z: f64) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let n = n as f64;
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denom;
    let half = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / denom;
    ((center - half).max(0.0), (center + half).min(1.0))
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

/// Run `trials` trials of `p`. A trial that does not count (see
/// `Paradox::trial`) is still a trial run; checkpoints follow counted trials.
pub fn simulate(p: &Paradox, trials: u64, seed: u64, points: usize) -> Outcome {
    let mut rng = Rng::new(seed);
    let mut scratch = Vec::new();
    let mut running = Running::default();
    let marks = checkpoint_counts(trials, points);
    let mut next_mark = 0;
    let mut checkpoints = Vec::with_capacity(marks.len());
    for _ in 0..trials {
        if let Some(x) = p.trial(&mut rng, &mut scratch) {
            running.push(x);
            while next_mark < marks.len() && running.n >= marks[next_mark] {
                checkpoints.push(Checkpoint {
                    n: running.n,
                    estimate: running.mean(),
                });
                next_mark += 1;
            }
        }
    }
    if checkpoints.last().map(|c| c.n) != Some(running.n) && running.n > 0 {
        checkpoints.push(Checkpoint {
            n: running.n,
            estimate: running.mean(),
        });
    }
    let n = running.n;
    let estimate = running.mean();
    let (standard_error, ci95) = match p.measure() {
        Measure::Probability => {
            let se = if n == 0 {
                0.0
            } else {
                (estimate * (1.0 - estimate) / n as f64).sqrt()
            };
            (se, wilson(estimate * n as f64, n))
        }
        Measure::Mean => {
            let se = if n < 2 {
                f64::INFINITY
            } else {
                (running.variance() / n as f64).sqrt()
            };
            (se, normal_interval(&running))
        }
    };
    Outcome {
        paradox: *p,
        seed,
        attempted: trials,
        counted: n,
        estimate,
        standard_error,
        ci95,
        theory: p.theory(),
        checkpoints,
    }
}
