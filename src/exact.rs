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
/// caching algorithms and self-organizing search", 1992), by adaptive
/// Simpson. The integrand is computed as -expm1(sum ln1p(-e^{-p_i t})), which
/// keeps its precision in the tail and over thousands of items, and the
/// tolerance is relative to the integral. None if it does not converge
/// within a budget of evaluations.
pub fn coupon_mean_weighted(probs: &[f64]) -> Option<f64> {
    let f = |t: f64| {
        -probs
            .iter()
            .map(|&p| (-(-p * t).exp()).ln_1p())
            .sum::<f64>()
            .exp_m1()
    };
    let p_min = probs.iter().copied().fold(f64::INFINITY, f64::min);
    let p_max = probs.iter().copied().fold(0.0, f64::max);
    if p_min.is_nan() || p_min <= 0.0 {
        return None;
    }
    // Past this point the integrand is below n * e^-45.
    let end = ((probs.len() as f64).ln() + 45.0) / p_min;
    if !end.is_finite() {
        return None;
    }
    // Split at doubling scales from the fastest rate up, so no feature is
    // stepped over; the integral is at least 1/p_min (the rarest item alone).
    let mut cuts: Vec<f64> = vec![0.0];
    let mut t = 1.0 / p_max;
    while t < end {
        cuts.push(t);
        t *= 2.0;
    }
    cuts.push(end);
    let scale = 1.0 / p_min;
    let mut evaluations = 0u64;
    let mut total = 0.0;
    for w in cuts.windows(2) {
        // Each piece gets its share of a relative tolerance by width.
        let tol = 1e-11 * scale * (w[1] - w[0]) / end;
        total += simpson(&f, w[0], w[1], tol, &mut evaluations)?;
    }
    Some(total)
}

/// Adaptive Simpson on [a, b] to an absolute tolerance, or None past a
/// budget of function evaluations (shared across calls through `evaluations`).
fn simpson(
    f: &impl Fn(f64) -> f64,
    a: f64,
    b: f64,
    tol: f64,
    evaluations: &mut u64,
) -> Option<f64> {
    const BUDGET: u64 = 5_000_000;
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
        evaluations: &mut u64,
    ) -> Option<f64> {
        let m = (a + b) / 2.0;
        let (lm, rm) = ((a + m) / 2.0, (m + b) / 2.0);
        let (flm, frm) = (f(lm), f(rm));
        *evaluations += 2;
        if *evaluations > BUDGET {
            return None;
        }
        let left = (m - a) / 6.0 * (fa + 4.0 * flm + fm);
        let right = (b - m) / 6.0 * (fm + 4.0 * frm + fb);
        let delta = left + right - whole;
        if delta.abs() <= 15.0 * tol || depth == 0 {
            Some(left + right + delta / 15.0)
        } else {
            Some(
                step(
                    f,
                    a,
                    m,
                    fa,
                    flm,
                    fm,
                    left,
                    tol / 2.0,
                    depth - 1,
                    evaluations,
                )? + step(
                    f,
                    m,
                    b,
                    fm,
                    frm,
                    fb,
                    right,
                    tol / 2.0,
                    depth - 1,
                    evaluations,
                )?,
            )
        }
    }
    let (fa, fb, fm) = (f(a), f(b), f((a + b) / 2.0));
    *evaluations += 3;
    let whole = (b - a) / 6.0 * (fa + 4.0 * fm + fb);
    step(f, a, b, fa, fm, fb, whole, tol, 50, evaluations)
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
        if p[n] >= 1.0 - 1e-15 {
            break;
        }
    }
    p[n].clamp(0.0, 1.0)
}

/// The same with item probabilities `probs`, by inclusion-exclusion over the
/// subsets of missing items: sum_S (-1)^|S| (1 - p(S))^draws, each term as
/// exp(draws * ln1p(-p(S))) and the sum compensated (Neumaier). Exponential,
/// so for at most `MAX_WEIGHTED_BUDGET_ITEMS` items; the absolute error stays
/// around 2^n * 1e-16, far below what a simulation can see.
pub fn coupon_complete_within_weighted(probs: &[f64], draws: u64) -> f64 {
    let n = probs.len();
    if (draws as usize) < n {
        return 0.0;
    }
    let (mut sum, mut comp) = (0.0f64, 0.0f64);
    for mask in 0u32..(1 << n) {
        let p_s: f64 = (0..n)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| probs[i])
            .sum();
        let term = if p_s >= 1.0 {
            0.0
        } else {
            (draws as f64 * (-p_s).ln_1p()).exp()
        };
        let term = if mask.count_ones() % 2 == 0 {
            term
        } else {
            -term
        };
        let t = sum + term;
        comp += if sum.abs() >= term.abs() {
            (sum - t) + term
        } else {
            (term - t) + sum
        };
        sum = t;
    }
    (sum + comp).clamp(0.0, 1.0)
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

/// The best reply to `a`: of all sequences of the same length, the one that
/// most often appears before it (exact odds; on a tie, Conway's reply if it
/// is among the best, else the first in H-before-T order). For three flips
/// this is Conway's rule (the opposite of a's second flip, then a without its
/// last); for four or more it can differ, so every candidate is checked.
pub fn penney_best_reply(a: &[bool]) -> Vec<bool> {
    let n = a.len();
    let conway = {
        let mut b = vec![!a.get(1).copied().unwrap_or(a[0])];
        b.extend_from_slice(&a[..n - 1]);
        b
    };
    let candidate =
        |bits: u32| -> Vec<bool> { (0..n).map(|i| bits & (1 << (n - 1 - i)) == 0).collect() };
    let mut best: (f64, Vec<bool>) = (-1.0, conway.clone());
    for bits in 0..(1u32 << n) {
        let b = candidate(bits);
        if b == a {
            continue;
        }
        let p = penney_second_wins(a, &b);
        if p > best.0 + 1e-12 {
            best = (p, b);
        }
    }
    if conway != a && (penney_second_wins(a, &conway) - best.0).abs() <= 1e-12 {
        conway
    } else {
        best.1
    }
}
