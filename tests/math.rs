use probsim::benford::{
    self, analyze, expected, fibonacci, leading_digit, numbers_in, powers_of_two,
};
use probsim::paradox::{birthday_probability, secretary_best_skip, secretary_success, Paradox};
use probsim::rng::Rng;
use probsim::stats::{chi_square_p, harmonic, ln_gamma, wilson, Running};

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn rng_is_repeatable_and_in_range() {
    let (mut a, mut b) = (Rng::new(7), Rng::new(7));
    for _ in 0..1000 {
        assert_eq!(a.next_u64(), b.next_u64());
    }
    let mut r = Rng::new(1);
    for _ in 0..10_000 {
        let x = r.f64();
        assert!((0.0..1.0).contains(&x));
        assert!(r.below(7) < 7);
    }
    assert_eq!(r.below(1), 0);
}

#[test]
fn below_is_uniform() {
    // 6 faces, 600k rolls: chi-square with 5 df should not be extreme.
    let mut r = Rng::new(99);
    let mut counts = [0f64; 6];
    for _ in 0..600_000 {
        counts[r.below(6) as usize] += 1.0;
    }
    let chi: f64 = counts
        .iter()
        .map(|c| (c - 100_000.0).powi(2) / 100_000.0)
        .sum();
    assert!(chi_square_p(chi, 5) > 0.001, "chi {chi}");
}

#[test]
fn shuffle_is_a_permutation() {
    let mut r = Rng::new(3);
    let mut v: Vec<u32> = (0..100).collect();
    r.shuffle(&mut v);
    let mut sorted = v.clone();
    sorted.sort();
    assert_eq!(sorted, (0..100).collect::<Vec<_>>());
    assert_ne!(v, sorted);
}

#[test]
fn special_functions() {
    assert!(close(ln_gamma(5.0), 24f64.ln(), 1e-12));
    assert!(close(
        ln_gamma(0.5),
        std::f64::consts::PI.sqrt().ln(),
        1e-12
    ));
    // Chi-square critical values from standard tables.
    assert!(close(chi_square_p(15.507, 8), 0.05, 2e-4));
    assert!(close(chi_square_p(20.090, 8), 0.01, 1e-4));
    assert!(close(chi_square_p(3.841, 1), 0.05, 2e-4));
    assert!(close(chi_square_p(0.0, 8), 1.0, 1e-12));
    assert!(chi_square_p(500.0, 8) < 1e-50);
    assert!(close(harmonic(10), 7381.0 / 2520.0, 1e-12));
}

#[test]
fn intervals() {
    // 0 of 10: the upper Wilson bound is z^2 / (n + z^2).
    let (lo, hi) = wilson(0.0, 10);
    assert_eq!(lo, 0.0);
    assert!(close(hi, 3.841_459 / 13.841_459, 1e-6));
    let mut r = Running::default();
    for x in [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0] {
        r.push(x);
    }
    assert!(close(r.mean(), 5.0, 1e-12));
    assert!(close(r.variance(), 32.0 / 7.0, 1e-12));
}

#[test]
fn exact_answers() {
    assert!(close(birthday_probability(23, 365), 0.507_297_234_3, 1e-9));
    assert_eq!(birthday_probability(366, 365), 1.0);
    assert_eq!(birthday_probability(1, 365), 0.0);
    assert_eq!(secretary_best_skip(100), 37);
    assert!(close(secretary_success(100, 37), 0.371_043, 1e-6));
    assert!(close(secretary_success(3, 1), 0.5, 1e-12));
    assert!(close(secretary_success(5, 0), 0.2, 1e-12));
    assert!(close(
        Paradox::MontyHall {
            doors: 3,
            host_knows: true
        }
        .theory(),
        2.0 / 3.0,
        1e-12
    ));
    assert!(close(
        Paradox::MontyHall {
            doors: 10,
            host_knows: false
        }
        .theory(),
        0.5,
        1e-12
    ));
    assert!(close(
        Paradox::Coupon { items: 10 }.theory(),
        29.289_682_539_682_54,
        1e-9
    ));
    assert!(close(
        Paradox::Buffon {
            length: 1.0,
            spacing: 2.0
        }
        .theory(),
        1.0 / std::f64::consts::PI,
        1e-12
    ));
}

#[test]
fn benford_basics() {
    let total: f64 = (1..=9).map(expected).sum();
    assert!(close(total, 1.0, 1e-12));
    assert_eq!(leading_digit(0.0123), Some(1));
    assert_eq!(leading_digit(987.0), Some(9));
    assert_eq!(leading_digit(-5e-7), Some(5));
    assert_eq!(leading_digit(0.0), None);
    assert_eq!(leading_digit(f64::NAN), None);
    assert_eq!(powers_of_two(10), vec![2, 4, 8, 1, 3, 6, 1, 2, 5, 1]);
    assert_eq!(fibonacci(12), vec![1, 1, 2, 3, 5, 8, 1, 2, 3, 5, 8, 1]);
    // Past the exact range the formula takes over; it must agree with the
    // exact digits where both are available.
    let exact = fibonacci(186);
    let phi = (1.0 + 5f64.sqrt()) / 2.0;
    for k in 30..=186u64 {
        let x = k as f64 * phi.log10() - 5f64.sqrt().log10();
        let d = (10f64.powf(x - x.floor()).floor() as usize).clamp(1, 9);
        assert_eq!(d, exact[k as usize - 1], "F_{k}");
    }
    assert_eq!(fibonacci(300).len(), 300);
}

#[test]
fn numbers_in_text() {
    assert_eq!(
        numbers_in("a 1,234,567 b 3.5e2, 7,8 -0.04 x12y .5 1,23 9,876,54"),
        vec![
            1_234_567.0,
            350.0,
            7.0,
            8.0,
            -0.04,
            12.0,
            0.5,
            1.0,
            23.0,
            9876.0,
            54.0
        ]
    );
    assert_eq!(numbers_in("売上 1,200円 と 3,400円"), vec![1200.0, 3400.0]);
    assert!(numbers_in("no numbers here").is_empty());
}

#[test]
fn benford_test_tells_uniform_from_benford() {
    let fits = analyze(&powers_of_two(10_000));
    assert!(fits.p_value > 0.5);
    let flat: Vec<usize> = (0..9_000).map(|i| i % 9 + 1).collect();
    let a = analyze(&flat);
    assert_eq!(a.total, 9_000);
    assert!(a.p_value < 1e-10);
    assert_eq!(benford::analyze(&[]).p_value, 1.0);
}

#[test]
fn powers_of_two_match_exact_values_across_the_switch() {
    // Exact leading digits of 2^k for k up to 200, by repeated doubling of a
    // decimal string, against the u128 and logarithm paths.
    let mut digits = vec![1u8];
    let got = powers_of_two(200);
    for k in 1..=200 {
        let mut carry = 0;
        for d in digits.iter_mut() {
            let v = *d * 2 + carry;
            *d = v % 10;
            carry = v / 10;
        }
        if carry > 0 {
            digits.push(carry);
        }
        assert_eq!(got[k - 1], *digits.last().unwrap() as usize, "2^{k}");
    }
}
