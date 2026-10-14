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

/// Wilson score interval for a proportion: better than mean +- z*se near 0
/// and 1 and for small n.
pub fn wilson(successes: f64, n: u64) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let n = n as f64;
    let p = successes / n;
    let z2 = Z95 * Z95;
    let denom = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denom;
    let half = Z95 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / denom;
    ((center - half).max(0.0), (center + half).min(1.0))
}

/// mean +- z * s / sqrt(n).
pub fn normal_interval(r: &Running) -> (f64, f64) {
    if r.n < 2 {
        return (f64::NEG_INFINITY, f64::INFINITY);
    }
    let half = Z95 * (r.variance() / r.n as f64).sqrt();
    (r.mean() - half, r.mean() + half)
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
