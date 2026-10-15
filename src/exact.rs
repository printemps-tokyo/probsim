//! Exact answers that take more than a line. No randomness here.

use crate::stats::harmonic;

/// Success probability of the secretary rule "reject the first `skip`, then
/// take the first who beats them all" with `n` candidates:
/// (k/n) * sum_{j=k}^{n-1} 1/j for k >= 1, and 1/n for k = 0.
pub fn secretary_success(n: u32, skip: u32) -> f64 {
    let n_f = n as f64;
    if skip == 0 {
        return 1.0 / n_f;
    }
    if skip >= n {
        return 0.0;
    }
    let tail: f64 = (skip..n).rev().map(|j| 1.0 / j as f64).sum();
    skip as f64 / n_f * tail
}

/// The cutoff that maximizes `secretary_success`, in one pass: the tail sum
/// is built from the top down. On a tie (within rounding) the smaller cutoff
/// wins, so for two candidates it is 0.
pub fn secretary_best_skip(n: u32) -> u32 {
    const TIE: f64 = 1e-15;
    let mut best: Option<(u32, f64)> = None;
    let mut tail = 0.0;
    for k in (1..n).rev() {
        tail += 1.0 / k as f64;
        let p = k as f64 / n as f64 * tail;
        // Going down, a later k is smaller, so it takes a tie.
        if best.is_none_or(|(_, b)| p >= b - TIE) {
            best = Some((k, p));
        }
    }
    match best {
        Some((k, p)) if p > 1.0 / n as f64 + TIE => k,
        _ => 0,
    }
}

/// P(at least two of `people` share a birthday) with `days` equally likely days.
pub fn birthday_probability(people: u32, days: u32) -> f64 {
    if people > days {
        return 1.0;
    }
    let mut all_different = 1.0;
    for k in 0..people {
        all_different *= (days - k) as f64 / days as f64;
    }
    1.0 - all_different
}

/// P(at least two of `people` share a birthday) when day d has probability
/// `probs[d]`. All different has probability n! * e_n(p), e_n the elementary
/// symmetric polynomial of degree n; f_k = k! * e_k stays within [0, 1] and
/// is built day by day with f_k += k * p_d * f_{k-1}.
pub fn birthday_probability_weighted(people: u32, probs: &[f64]) -> f64 {
    let n = people as usize;
    if n > probs.iter().filter(|&&p| p > 0.0).count() {
        return 1.0;
    }
    let mut f = vec![0.0; n + 1];
    f[0] = 1.0;
    for &p in probs {
        for k in (1..=n).rev() {
            f[k] += k as f64 * p * f[k - 1];
        }
    }
    (1.0 - f[n]).clamp(0.0, 1.0)
}

/// Expected draws to collect every item when item i comes with probability
/// `probs[i]`: E[T] = integral_0^inf (1 - prod_i (1 - e^{-p_i t})) dt
/// (Flajolet, Gardy and Thimonier, "Birthday paradox, coupon collectors,
/// caching algorithms and self-organizing search", 1992), by adaptive Simpson.
pub fn coupon_mean_weighted(probs: &[f64]) -> f64 {
    let f = |t: f64| 1.0 - probs.iter().map(|&p| 1.0 - (-p * t).exp()).product::<f64>();
    let p_min = probs.iter().copied().fold(f64::INFINITY, f64::min);
    // Past this point the integrand is below n * e^-45.
    let end = ((probs.len() as f64).ln() + 45.0) / p_min;
    // Split at the scale of each rate so no feature is stepped over.
    let mut cuts: Vec<f64> = vec![0.0];
    let mut t = 1.0 / probs.iter().copied().fold(0.0, f64::max);
    while t < end {
        cuts.push(t);
        t *= 2.0;
    }
    cuts.push(end);
    cuts.windows(2)
        .map(|w| simpson(&f, w[0], w[1], 1e-12, 40))
        .sum()
}

fn simpson(f: &impl Fn(f64) -> f64, a: f64, b: f64, tol: f64, depth: u32) -> f64 {
    // The recursion carries the interval, its three function values and the
    // running estimate; a struct would only rename them.
    #[allow(clippy::too_many_arguments)]
    fn step(
        f: &impl Fn(f64) -> f64,
        a: f64,
        b: f64,
        fa: f64,
        fm: f64,
        fb: f64,
        whole: f64,
        tol: f64,
        depth: u32,
    ) -> f64 {
        let m = (a + b) / 2.0;
        let (lm, rm) = ((a + m) / 2.0, (m + b) / 2.0);
        let (flm, frm) = (f(lm), f(rm));
        let left = (m - a) / 6.0 * (fa + 4.0 * flm + fm);
        let right = (b - m) / 6.0 * (fm + 4.0 * frm + fb);
        let delta = left + right - whole;
        if depth == 0 || delta.abs() <= 15.0 * tol {
            left + right + delta / 15.0
        } else {
            step(f, a, m, fa, flm, fm, left, tol / 2.0, depth - 1)
                + step(f, m, b, fm, frm, fb, right, tol / 2.0, depth - 1)
        }
    }
    let (fa, fb, fm) = (f(a), f(b), f((a + b) / 2.0));
    let whole = (b - a) / 6.0 * (fa + 4.0 * fm + fb);
    step(f, a, b, fa, fm, fb, whole, tol, depth)
}

/// P(every one of `items` equally likely items is collected within `draws`
/// draws), by a Markov chain on the number collected: O(items * draws).
pub fn coupon_complete_within_equal(items: u32, draws: u64) -> f64 {
    let n = items as usize;
    let mut p = vec![0.0; n + 1];
    p[0] = 1.0;
    for _ in 0..draws {
        // Going down, so p[k - 1] is still the old value when p[k] is updated.
        for k in (1..=n).rev() {
            let new_item = (n - (k - 1)) as f64 / n as f64;
            p[k] = p[k] * (k as f64 / n as f64) + p[k - 1] * new_item;
        }
        p[0] = 0.0;
        if p[n] > 1.0 - 1e-17 {
            break;
        }
    }
    p[n].clamp(0.0, 1.0)
}

/// The same with item probabilities `probs`, by inclusion-exclusion over the
/// subsets of missing items: sum_S (-1)^|S| (1 - p(S))^draws. Exponential, so
/// for at most `MAX_WEIGHTED_BUDGET_ITEMS` items.
pub fn coupon_complete_within_weighted(probs: &[f64], draws: u64) -> f64 {
    let n = probs.len();
    let mut total = 0.0;
    for mask in 0u32..(1 << n) {
        let p_s: f64 = (0..n)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| probs[i])
            .sum();
        let term = (1.0 - p_s).max(0.0).powf(draws as f64);
        if mask.count_ones() % 2 == 0 {
            total += term;
        } else {
            total -= term;
        }
    }
    total.clamp(0.0, 1.0)
}

/// Items for which `coupon_complete_within_weighted` stays fast.
pub const MAX_WEIGHTED_BUDGET_ITEMS: usize = 20;

/// Equal-rate quantiles of the draws to complete: the smallest t with
/// P(T <= t) >= q, for each q, from the same Markov chain. None when the
/// chain would take too long (more than `limit` item-steps).
pub fn coupon_quantiles_equal(items: u32, qs: &[f64], limit: u64) -> Option<Vec<u64>> {
    let n = items as usize;
    let mut p = vec![0.0; n + 1];
    p[0] = 1.0;
    let mut out = vec![None; qs.len()];
    let mut t = 0u64;
    while out.iter().any(Option::is_none) {
        if t * n as u64 > limit {
            return None;
        }
        t += 1;
        for k in (1..=n).rev() {
            let new_item = (n - (k - 1)) as f64 / n as f64;
            p[k] = p[k] * (k as f64 / n as f64) + p[k - 1] * new_item;
        }
        p[0] = 0.0;
        for (i, &q) in qs.iter().enumerate() {
            if out[i].is_none() && p[n] >= q {
                out[i] = Some(t);
            }
        }
    }
    Some(out.into_iter().map(Option::unwrap).collect())
}

/// n * H_n.
pub fn coupon_mean_equal(items: u32) -> f64 {
    items as f64 * harmonic(items as u64)
}

/// Conway's leading number of x over y: sum of 2^(n-1-k) over the shifts k
/// where the last n - k flips of x equal the first n - k flips of y.
pub fn leading_number(x: &[bool], y: &[bool]) -> u64 {
    let n = x.len();
    (0..n)
        .filter(|&k| x[k..] == y[..n - k])
        .map(|k| 1u64 << (n - 1 - k))
        .sum()
}

/// P(the second sequence appears before the first) in fair coin flips, by
/// Conway's algorithm: the odds for B over A are (AA - AB) : (BB - BA).
pub fn penney_second_wins(a: &[bool], b: &[bool]) -> f64 {
    let (aa, ab, bb, ba) = (
        leading_number(a, a) as f64,
        leading_number(a, b) as f64,
        leading_number(b, b) as f64,
        leading_number(b, a) as f64,
    );
    let (for_b, for_a) = (aa - ab, bb - ba);
    for_b / (for_b + for_a)
}

/// Conway's best reply to `a`: the opposite of a's second flip, then a
/// without its last flip.
pub fn penney_best_reply(a: &[bool]) -> Vec<bool> {
    let mut b = vec![!a.get(1).copied().unwrap_or(a[0])];
    b.extend_from_slice(&a[..a.len() - 1]);
    b
}
