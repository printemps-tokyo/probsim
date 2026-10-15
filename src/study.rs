//! Runs of many simulations: a sweep over one parameter, and repeated runs
//! that show what "95% confidence" means.

use crate::paradox::Paradox;
use crate::sim::{derived_seed, simulate, simulate_known, Outcome};
use crate::stats::Z95;

/// The values a sweep takes: `steps` evenly spaced values from `from` to
/// `to`, rounded and deduplicated for an integer parameter.
pub fn sweep_values(from: f64, to: f64, steps: usize, integer: bool) -> Vec<f64> {
    let steps = steps.max(2);
    let (lo, hi) = (from.min(to), from.max(to));
    let mut v: Vec<f64> = (0..steps)
        .map(|i| from + (to - from) * i as f64 / (steps - 1) as f64)
        .map(|x| if integer { x.round() } else { x })
        // Rounding must not step outside the range asked for.
        .filter(|x| (lo..=hi).contains(x))
        .collect();
    v.dedup_by(|a, b| a == b);
    v
}

/// One point of a sweep.
#[derive(Clone, Debug)]
pub struct SweepPoint {
    pub value: f64,
    pub outcome: Outcome,
}

/// Simulate the paradox `make` builds for each value, each with its own
/// random stream derived from `seed`.
pub fn sweep(
    values: &[f64],
    make: impl Fn(f64) -> Result<Paradox, String>,
    trials: u64,
    seed: u64,
) -> Result<Vec<SweepPoint>, String> {
    values
        .iter()
        .enumerate()
        .map(|(i, &value)| {
            let p = make(value).map_err(|e| format!("at {}: {e}", crate::paradox::num(value)))?;
            Ok(SweepPoint {
                value,
                outcome: simulate(&p, trials, derived_seed(seed, i as u64), 2),
            })
        })
        .collect()
}

/// One run of a coverage study: its 95% interval and estimate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub seed: u64,
    pub estimate: f64,
    pub interval: (f64, f64),
    pub hit: bool,
}

/// Repeat a simulation `runs` times with independent seeds and record which
/// 95% intervals contain the exact answer. Runs whose interval cannot be
/// given (an average from fewer than 30 trials) are left out.
pub fn coverage(p: &Paradox, runs: u64, trials: u64, seed: u64) -> Vec<Run> {
    // The exact answer once, not once per run.
    let theory = p.theory();
    (0..runs)
        .filter_map(|i| {
            let s = derived_seed(seed, i);
            let o = simulate_known(p, theory, trials, s, 2);
            let interval = o.interval(Z95)?;
            Some(Run {
                seed: s,
                estimate: o.estimate,
                interval,
                hit: (interval.0..=interval.1).contains(&o.theory),
            })
        })
        .collect()
}
