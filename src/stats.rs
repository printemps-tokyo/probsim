//! Running statistics, confidence intervals and the chi-square test.

/// Mean and variance in one pass (Welford's method).
#[derive(Clone, Copy, Debug, Default)]
pub struct Running {
    pub n: u64,
    mean: f64,
    m2: f64,
}

impl Running {
    pub fn push(&mut self, x: f64) {
        self.n += 1;
        let d = x - self.mean;
        self.mean += d / self.n as f64;
        self.m2 += d * (x - self.mean);
    }
    pub fn mean(&self) -> f64 {
        self.mean
    }
    /// Sample variance (n - 1 in the denominator); 0 for fewer than two values.
    pub fn variance(&self) -> f64 {
        if self.n < 2 {
            0.0
        } else {
            self.m2 / (self.n - 1) as f64
        }
    }
}

/// z for a two-sided 95% interval.
pub const Z95: f64 = 1.959_963_984_540_054;

/// Wilson score interval for `successes` out of `n` at confidence z: better
/// than p +- z*se near 0 and 1 and for small n. With no successes the lower
/// bound is exactly 0, and with all successes the upper bound is exactly 1,
/// so rounding never pushes a certain outcome out of its own interval.
pub fn wilson(successes: u64, n: u64, z: f64) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let nf = n as f64;
    let p = successes as f64 / nf;
    let z2 = z * z;
    let denom = 1.0 + z2 / nf;
    let center = (p + z2 / (2.0 * nf)) / denom;
    let half = z * (p * (1.0 - p) / nf + z2 / (4.0 * nf * nf)).sqrt() / denom;
    let lo = if successes == 0 {
        0.0
    } else {
        (center - half).max(0.0)
    };
    let hi = if successes == n {
        1.0
    } else {
        (center + half).min(1.0)
    };
    (lo, hi)
}

/// Below this many values, an interval for an average is not trusted: the
/// normal approximation behind it is too rough (and with a handful of equal
/// values the sample variance is 0).
pub const MIN_FOR_MEAN: u64 = 30;

/// mean +- z * s / sqrt(n), or None with fewer than `MIN_FOR_MEAN` values.
pub fn normal_interval(r: &Running, z: f64) -> Option<(f64, f64)> {
    if r.n < MIN_FOR_MEAN {
        return None;
    }
    let half = z * (r.variance() / r.n as f64).sqrt();
    Some((r.mean() - half, r.mean() + half))
}

/// H_n = 1 + 1/2 + ... + 1/n.
pub fn harmonic(n: u64) -> f64 {
    (1..=n).rev().map(|k| 1.0 / k as f64).sum()
}

/// ln Gamma(x) for x > 0 (Lanczos approximation, g = 7, n = 9).
pub fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        // Reflection formula.
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + G + 0.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// Regularized upper incomplete gamma Q(a, x) = 1 - P(a, x): a series for
/// x < a + 1, a continued fraction (modified Lentz) otherwise.
pub fn gamma_q(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    let ln_front = -x + a * x.ln() - ln_gamma(a);
    if x < a + 1.0 {
        let (mut sum, mut term, mut ap) = (1.0 / a, 1.0 / a, a);
        for _ in 0..10_000 {
            ap += 1.0;
            term *= x / ap;
            sum += term;
            if term.abs() < sum.abs() * 1e-15 {
                break;
            }
        }
        1.0 - sum * ln_front.exp()
    } else {
        let tiny = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..10_000 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < tiny {
                d = tiny;
            }
            c = b + an / c;
            if c.abs() < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let delta = d * c;
            h *= delta;
            if (delta - 1.0).abs() < 1e-15 {
                break;
            }
        }
        ln_front.exp() * h
    }
}

/// p-value of a chi-square statistic with `df` degrees of freedom.
pub fn chi_square_p(stat: f64, df: u32) -> f64 {
    gamma_q(df as f64 / 2.0, stat / 2.0).clamp(0.0, 1.0)
}

/// The central range of a Binomial(n, p) count holding at least `level` of
/// its probability (the smallest k with P(X <= k) > (1 - level) / 2 up to the
/// largest with P(X >= k) > (1 - level) / 2), from the probabilities
/// computed in logs.
pub fn binomial_central(n: u64, p: f64, level: f64) -> (u64, u64) {
    // A certain outcome has all its probability on one count.
    if p <= 0.0 {
        return (0, 0);
    }
    if p >= 1.0 {
        return (n, n);
    }
    let tail = (1.0 - level) / 2.0;
    let ln_pmf = |k: u64| {
        ln_gamma(n as f64 + 1.0) - ln_gamma(k as f64 + 1.0) - ln_gamma((n - k) as f64 + 1.0)
            + k as f64 * p.ln()
            + (n - k) as f64 * (1.0 - p).ln()
    };
    let (mut acc, mut lo) = (0.0, 0);
    for k in 0..=n {
        acc += ln_pmf(k).exp();
        if acc > tail {
            lo = k;
            break;
        }
    }
    let (mut acc, mut hi) = (0.0, n);
    for k in (0..=n).rev() {
        acc += ln_pmf(k).exp();
        if acc > tail {
            hi = k;
            break;
        }
    }
    (lo, hi)
}
