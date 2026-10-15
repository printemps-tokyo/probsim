use std::sync::Arc;

use probsim::benford::{
    self, analyze, count_digits, csv_fields, expected, fibonacci, leading_digit, leading_digit_of,
    leading_two_of, number_tokens, powers_of_two, scan_csv, Conformity, Source,
};
use probsim::exact;
use probsim::paradox::{
    birthday_probability, coins, secretary_best_skip, secretary_success, Chord, Paradox, Weights,
};
use probsim::rng::Rng;
use probsim::stats::{binomial_central, chi_square_p, harmonic, ln_gamma, wilson, Running, Z95};

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn rng_matches_the_reference_implementation() {
    // splitmix64 seeded with 42 fills the state, then xoshiro256**: the
    // values printed by the authors' C code (prng.di.unimi.it).
    let mut r = Rng::new(42);
    let want = [
        0x1578_0b2e_0c2e_c716u64,
        0x6104_d986_6d11_3a7e,
        0xae17_5332_39e4_99a1,
        0xecb8_ad47_03b3_60a1,
        0xfde6_dc7f_e2ec_5e64,
        0xc50d_a531_0179_5238,
    ];
    for w in want {
        assert_eq!(r.next_u64(), w);
    }
}

#[test]
fn rng_stays_in_range_and_is_uniform() {
    let mut r = Rng::new(1);
    for _ in 0..10_000 {
        let x = r.f64();
        assert!((0.0..1.0).contains(&x));
        assert!(r.below(7) < 7);
    }
    assert_eq!(r.below(1), 0);
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
fn below_rejects_correctly_when_rejection_is_common() {
    // n = 2^63 + 1 rejects about half of all draws; the results (0..=2^63)
    // must stay below n and split evenly around the middle, 2^62.
    let n = (1u64 << 63) + 1;
    let mut r = Rng::new(5);
    let mut high = 0;
    for _ in 0..20_000 {
        let v = r.below(n);
        assert!(v < n);
        if v >= 1 << 62 {
            high += 1;
        }
    }
    assert!((9_500..10_500).contains(&high), "{high}");
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
    assert!(close(chi_square_p(112.022, 89), 0.05, 3e-4));
    assert!(close(chi_square_p(0.0, 8), 1.0, 1e-12));
    assert!(chi_square_p(500.0, 8) < 1e-50);
    assert!(close(harmonic(10), 7381.0 / 2520.0, 1e-12));
}

#[test]
fn intervals() {
    let (lo, hi) = wilson(0, 10, Z95);
    assert_eq!(lo, 0.0);
    assert!(close(hi, 3.841_459 / 13.841_459, 1e-6));
    // A certain outcome is never pushed out of its own interval by rounding.
    for n in 1..5_000u64 {
        assert_eq!(wilson(n, n, Z95).1, 1.0, "n={n}");
        assert_eq!(wilson(0, n, Z95).0, 0.0, "n={n}");
        assert_eq!(wilson(n, n, 3.29).1, 1.0, "n={n}");
    }
    let mut r = Running::default();
    for x in [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0] {
        r.push(x);
    }
    assert!(close(r.mean(), 5.0, 1e-12));
    assert!(close(r.variance(), 32.0 / 7.0, 1e-12));
    // Binomial(20, 0.95): P(X <= 16) = 0.0159 < 0.025 < P(X <= 17) = 0.0755.
    assert_eq!(binomial_central(20, 0.95, 0.95), (17, 20));
    let (lo, hi) = binomial_central(1000, 0.5, 0.95);
    assert!(
        (468..=470).contains(&lo) && (530..=532).contains(&hi),
        "{lo} {hi}"
    );
}

#[test]
fn exact_answers() {
    assert!(close(birthday_probability(23, 365), 0.507_297_234_3, 1e-9));
    assert_eq!(birthday_probability(366, 365), 1.0);
    assert_eq!(birthday_probability(1, 365), 0.0);
    assert_eq!(secretary_best_skip(100), 37);
    assert!(close(secretary_success(100, 37), 0.371_043, 1e-6));
    assert!(close(secretary_success(3, 1), 0.5, 1e-12));
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
    let coupon = |items| Paradox::Coupon {
        items,
        weights: None,
        budget: None,
    };
    assert!(close(coupon(10).theory(), 29.289_682_539_682_54, 1e-9));
    // Coupon variance for n = 2: draws = 1 + Geometric(1/2), variance 2.
    assert!(close(coupon(2).theory_variance(), 2.0, 1e-12));
    assert!(close(
        Paradox::Buffon {
            length: 1.0,
            spacing: 2.0
        }
        .theory(),
        1.0 / std::f64::consts::PI,
        1e-12
    ));
    // Base rate: 1% prevalence, 99% sensitivity, 95% specificity gives 1/6.
    let br = Paradox::BaseRate {
        prevalence: 0.01,
        sensitivity: 0.99,
        specificity: 0.95,
    };
    assert!(close(br.theory(), 0.0099 / 0.0594, 1e-12));
    assert!(close(
        Paradox::BoyGirl { tuesday: true }.theory(),
        13.0 / 27.0,
        1e-12
    ));
    assert!(close(
        Paradox::BoyGirl { tuesday: false }.theory(),
        1.0 / 3.0,
        1e-12
    ));
    for (m, v) in [
        (Chord::Endpoints, 1.0 / 3.0),
        (Chord::Radius, 0.5),
        (Chord::Midpoint, 0.25),
    ] {
        assert!(close(Paradox::Bertrand { method: m }.theory(), v, 1e-12));
    }
}

#[test]
fn boy_girl_by_counting_every_family() {
    // 14 equally likely kinds of child (sex x weekday); count both pairs.
    let (mut with_boy, mut both) = (0, 0);
    for a in 0..14 {
        for b in 0..14 {
            let tuesday_boy = |k: usize| k < 7 && k == 1;
            if tuesday_boy(a) || tuesday_boy(b) {
                with_boy += 1;
                if a < 7 && b < 7 {
                    both += 1;
                }
            }
        }
    }
    assert_eq!((both, with_boy), (13, 27));
}

#[test]
fn penney_against_published_odds() {
    let p = |a: &str, b: &str| exact::penney_second_wins(&coins(a).unwrap(), &coins(b).unwrap());
    // Conway's answers for three flips (Gardner; arXiv:1904.09888).
    assert!(close(p("HHH", "THH"), 7.0 / 8.0, 1e-12));
    assert!(close(p("HHT", "THH"), 3.0 / 4.0, 1e-12));
    assert!(close(p("HTH", "HHT"), 2.0 / 3.0, 1e-12));
    assert!(close(p("HTT", "HHT"), 2.0 / 3.0, 1e-12));
    assert!(close(p("HH", "TH"), 3.0 / 4.0, 1e-12));
    // Symmetry: the two answers for a pair add up to 1.
    assert!(close(p("HTH", "THT") + p("THT", "HTH"), 1.0, 1e-12));
    let best =
        |a: &str| probsim::paradox::coins_text(&exact::penney_best_reply(&coins(a).unwrap()));
    assert_eq!(best("HHH"), "THH");
    assert_eq!(best("HTH"), "HHT");
    assert_eq!(best("THT"), "TTH");
    assert!(coins("HXT").is_err());
    assert!(coins("").is_err());
}

#[test]
fn best_secretary_cutoff_matches_a_full_search() {
    for n in 1..300u32 {
        let full =
            (0..n)
                .map(|k| (k, secretary_success(n, k)))
                .fold((0, f64::MIN), |best, (k, p)| {
                    if p > best.1 + 1e-15 {
                        (k, p)
                    } else {
                        best
                    }
                });
        assert_eq!(secretary_best_skip(n), full.0, "n={n}");
    }
    assert_eq!(secretary_best_skip(1000), 368);
}

#[test]
fn unequal_birthdays() {
    // Equal weights give the classic answer.
    let flat = Weights::new(&[1.0; 365]).unwrap();
    assert!(close(
        exact::birthday_probability_weighted(23, &flat.probs),
        birthday_probability(23, 365),
        1e-12
    ));
    // Two days with chances p and 1 - p: two people differ with 2 p (1 - p).
    let w = Weights::new(&[1.0, 3.0]).unwrap();
    assert!(close(
        exact::birthday_probability_weighted(2, &w.probs),
        1.0 - 2.0 * 0.25 * 0.75,
        1e-12
    ));
    // Uneven chances never make a match less likely.
    let uneven: Vec<f64> = (0..365).map(|i| 1.0 + (i % 7 == 0) as u8 as f64).collect();
    let uw = Weights::new(&uneven).unwrap();
    assert!(exact::birthday_probability_weighted(23, &uw.probs) > birthday_probability(23, 365));
    // More people than days with a chance: certain.
    assert_eq!(
        exact::birthday_probability_weighted(3, &Weights::new(&[1.0, 0.0, 1.0]).unwrap().probs),
        1.0
    );
    assert!(Weights::new(&[]).is_err());
    assert!(Weights::new(&[1.0, -1.0]).is_err());
    assert!(Weights::new(&[0.0, 0.0]).is_err());
}

#[test]
fn weighted_sampling_follows_the_weights() {
    let w = Weights::new(&[1.0, 0.0, 3.0]).unwrap();
    let mut r = Rng::new(8);
    let mut c = [0u32; 3];
    for _ in 0..40_000 {
        c[w.sample(&mut r)] += 1;
    }
    assert_eq!(c[1], 0, "an item with no chance is never drawn");
    assert!((9_500..10_500).contains(&c[0]), "{c:?}");
}

#[test]
fn gacha_exact_answers() {
    // The integral gives n H_n for equal rates.
    for n in [1u32, 2, 5, 10, 50] {
        let p = vec![1.0 / n as f64; n as usize];
        assert!(
            close(
                exact::coupon_mean_weighted(&p).unwrap(),
                exact::coupon_mean_equal(n),
                1e-7 * n as f64
            ),
            "n={n}"
        );
    }
    // Two items with p and q: E = 1/p + 1/q - 1/(p + q) (inclusion-exclusion).
    let (p, q) = (0.9, 0.1);
    assert!(close(
        exact::coupon_mean_weighted(&[p, q]).unwrap(),
        1.0 / p + 1.0 / q - 1.0,
        1e-8
    ));
    // Completing within a budget: the Markov chain, inclusion-exclusion and a
    // direct count agree. Two equal items within 3 draws: 1 - 2 (1/2)^3 = 3/4.
    assert!(close(
        exact::coupon_complete_within_equal(2, 3),
        0.75,
        1e-12
    ));
    assert!(close(
        exact::coupon_complete_within_weighted(&[0.5, 0.5], 3),
        0.75,
        1e-12
    ));
    let eq = [0.1; 10];
    for b in [10u64, 20, 29, 60] {
        assert!(close(
            exact::coupon_complete_within_equal(10, b),
            exact::coupon_complete_within_weighted(&eq, b),
            1e-9
        ));
    }
    assert_eq!(exact::coupon_complete_within_equal(10, 9), 0.0);
    // Quantiles for 10 items: the median is 27 draws (P(T <= 26) < 1/2 <= P(T <= 27)).
    let q = exact::coupon_quantiles_equal(10, &[0.5], 10_000_000).unwrap();
    assert!(exact::coupon_complete_within_equal(10, q[0]) >= 0.5);
    assert!(exact::coupon_complete_within_equal(10, q[0] - 1) < 0.5);
    let budget = Paradox::Coupon {
        items: 10,
        weights: None,
        budget: Some(30),
    };
    assert_eq!(budget.measure(), probsim::paradox::Measure::Probability);
    let w = Paradox::Coupon {
        items: 2,
        weights: Some(Arc::new(Weights::new(&[9.0, 1.0]).unwrap())),
        budget: None,
    };
    assert!(close(w.theory(), 1.0 / 0.9 + 1.0 / 0.1 - 1.0, 1e-8));
    assert!(w.theory_variance().is_nan());
}

#[test]
fn benford_digits() {
    let total: f64 = (1..=9).map(expected).sum();
    assert!(close(total, 1.0, 1e-12));
    let total2: f64 = (10..=99).map(expected).sum();
    assert!(close(total2, 1.0, 1e-12));
    assert_eq!(benford::min_for_test(1), 110);
    assert_eq!(benford::min_for_test(2), 1146);
    assert_eq!(leading_digit(0.0123), Some(1));
    assert_eq!(leading_digit(f64::NAN), None);
    assert_eq!(powers_of_two(10), vec![2, 4, 8, 1, 3, 6, 1, 2, 5, 1]);
    assert_eq!(fibonacci(12), vec![1, 1, 2, 3, 5, 8, 1, 2, 3, 5, 8, 1]);
    // Past the exact range the formula takes over; it agrees where both exist.
    let exact = fibonacci(186);
    let phi = (1.0 + 5f64.sqrt()) / 2.0;
    for k in 30..=186u64 {
        let x = k as f64 * phi.log10() - 5f64.sqrt().log10();
        let d = (10f64.powf(x - x.floor()).floor() as usize).clamp(1, 9);
        assert_eq!(d, exact[k as usize - 1], "F_{k}");
    }
    assert_eq!(benford::power_of_two_two(10), 10); // 1024
    assert_eq!(benford::power_of_two_two(3), 80); // 8, read as 8.0
}

#[test]
fn powers_of_two_match_exact_values_across_the_switch() {
    // Exact first two digits of 2^k up to k = 200, by doubling a decimal string.
    let mut digits = vec![1u8];
    for k in 1..=200u64 {
        let mut carry = 0;
        for d in digits.iter_mut() {
            let v = *d * 2 + carry;
            *d = v % 10;
            carry = v / 10;
        }
        if carry > 0 {
            digits.push(carry);
        }
        let n = digits.len();
        let two = digits[n - 1] as usize * 10 + if n > 1 { digits[n - 2] as usize } else { 0 };
        assert_eq!(benford::power_of_two_two(k), two, "2^{k}");
        assert_eq!(powers_of_two(k)[k as usize - 1], two / 10, "2^{k}");
    }
}

fn texts(s: &str) -> Vec<String> {
    number_tokens(s).into_iter().map(|t| t.text).collect()
}

#[test]
fn numbers_in_text() {
    assert_eq!(
        texts("a 1,234,567 b 3.5e2, 7,8 -0.04 x12y .5 1,23 9,876,54"),
        vec!["1234567", "3.5e2", "7", "8", "-0.04", "12", ".5", "1", "23", "9876", "54"]
    );
    assert_eq!(texts("売上 1,200円 と 3,400円"), vec!["1200", "3400"]);
    // A hyphen after a digit is not a minus sign. ("15,350" reads as one
    // number in free text; a CSV column is what --column is for.)
    assert_eq!(texts("2024-01-15,350"), vec!["2024", "01", "15350"]);
    assert_eq!(
        texts("1.e3 2.e4 0.e5 3."),
        vec!["1.e3", "2.e4", "0.e5", "3."]
    );
    assert!(texts("no numbers here").is_empty());
}

#[test]
fn leading_digits_as_written() {
    assert_eq!(leading_digit_of("9999999999999999999"), Some(9));
    assert_eq!(leading_digit_of("1e400"), Some(1));
    assert_eq!(leading_digit_of("-0.00071"), Some(7));
    assert_eq!(leading_digit_of("0.000"), None);
    assert_eq!(leading_two_of("0.00071"), Some(71));
    assert_eq!(leading_two_of("7"), Some(70));
    assert_eq!(leading_two_of("1,234"), Some(12));
    assert_eq!(leading_two_of("10.5"), Some(10));
}

#[test]
fn csv_and_encodings() {
    assert_eq!(
        csv_fields("a,\"1,234\",\"say \"\"hi\"\"\"", ','),
        vec!["a", "1,234", "say \"hi\""]
    );
    let s = scan_csv(
        "note,amount\n\"first\nsecond\",250\n\"x\",\"1,300\"\r\n3,0\n",
        2,
        ',',
    );
    assert_eq!((s.numbers, s.zeros), (3, 1));
    let first = Source::File {
        name: "x".into(),
        column: Some(2),
        scan: s,
    }
    .counts(1);
    assert_eq!((first[1], first[0]), (1, 1));
    let utf16le: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("123\n456\n".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    assert_eq!(
        benford::scan_text(&benford::decode(&utf16le).unwrap()).numbers,
        2
    );
    let no_bom: Vec<u8> = "123".encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert!(benford::decode(&no_bom).is_err());
    assert_eq!(
        benford::scan_text(&benford::decode(b"\x94\x84 12").unwrap()).numbers,
        1
    );
}

#[test]
fn benford_tests_and_mad() {
    let fits = analyze(count_digits(&powers_of_two(10_000)), 1);
    assert!(fits.p_value.unwrap() > 0.5);
    assert_eq!(fits.conformity, Conformity::Close);
    let flat: Vec<usize> = (0..9_000).map(|i| i % 9 + 1).collect();
    let a = analyze(count_digits(&flat), 1);
    assert!(a.p_value.unwrap() < 1e-10);
    assert_eq!(a.conformity, Conformity::Nonconformity);
    // MAD of a flat distribution: mean |1/9 - log10(1 + 1/d)|.
    let want: f64 = (1..=9)
        .map(|d| (1.0 / 9.0 - expected(d)).abs())
        .sum::<f64>()
        / 9.0;
    assert!(close(a.mad, want, 1e-12));
    assert_eq!(analyze(count_digits(&[1, 2, 9]), 1).p_value, None);
    // First two digits: 89 degrees of freedom, and the counts add up to the first digits'.
    let two = analyze(Source::PowersOfTwo(5_000).counts(2), 2);
    assert_eq!((two.df, two.counts.len()), (89, 90));
    let firsts = Source::PowersOfTwo(5_000).counts(1);
    for (d, first) in firsts.iter().enumerate() {
        assert_eq!(two.counts[d * 10..d * 10 + 10].iter().sum::<u64>(), *first);
    }
    assert_eq!(
        Source::Fibonacci(500).counts(1),
        count_digits(&fibonacci(500))
    );
}

#[test]
fn gacha_numerics_hold_up_at_the_extremes() {
    use std::time::Instant;
    // Extreme ratios and many items converge quickly to the exact mean.
    let t = Instant::now();
    for q in [1e-3, 1e-6, 1e-9, 1e-12] {
        let (a, b) = (1.0 / (1.0 + q), q / (1.0 + q));
        let want = 1.0 / a + 1.0 / b - 1.0;
        let got = exact::coupon_mean_weighted(&[a, b]).unwrap();
        assert!(((got - want) / want).abs() < 1e-8, "q={q}: {got} vs {want}");
    }
    let mut r = Rng::new(1);
    let raw: Vec<f64> = (0..3000).map(|_| 0.5 + r.f64()).collect();
    let w = Weights::new(&raw).unwrap();
    assert!(w.coupon_mean().is_some());
    assert!(t.elapsed().as_secs() < 20, "took {:?}", t.elapsed());
    // A budget below the number of items is 0 exactly, and tiny chances keep
    // their precision: two items with chances 1 - q and q within 100 draws.
    let probs: Vec<f64> = std::iter::repeat_n(1.0 / 20.0, 20).collect();
    assert_eq!(exact::coupon_complete_within_weighted(&probs, 19), 0.0);
    let q: f64 = 1e-9 / (1.0 + 1e-9);
    let want = -(100.0 * (-q).ln_1p()).exp_m1() - q.powi(100);
    let got = exact::coupon_complete_within_weighted(&[1.0 - q, q], 100);
    assert!(((got - want) / want).abs() < 1e-6, "{got} vs {want}");
    // Weights too uneven to sample are refused.
    assert!(Weights::new(&[1.0, 1e-13]).is_err());
    assert!(Weights::new(&[1.0, 1e-12]).is_ok());
}

#[test]
fn penney_best_reply_is_the_best_for_every_length() {
    for n in 1..=5usize {
        for bits in 0..(1u32 << n) {
            let a: Vec<bool> = (0..n).map(|i| bits & (1 << i) != 0).collect();
            let reply = exact::penney_best_reply(&a);
            assert_ne!(reply, a);
            let best = exact::penney_second_wins(&a, &reply);
            for other in 0..(1u32 << n) {
                let b: Vec<bool> = (0..n).map(|i| other & (1 << i) != 0).collect();
                if b != a {
                    assert!(
                        exact::penney_second_wins(&a, &b) <= best + 1e-12,
                        "{a:?} vs {b:?}"
                    );
                }
            }
            if n >= 3 {
                assert!(
                    best > 0.5,
                    "from three flips on the second player has an edge: {a:?}"
                );
            }
        }
    }
    // Against HTHH the best reply is THTH (9/14), not Conway's three-flip rule (HHTH, 3/5).
    let a = coins("HTHH").unwrap();
    assert_eq!(
        probsim::paradox::coins_text(&exact::penney_best_reply(&a)),
        "THTH"
    );
    assert!(close(
        exact::penney_second_wins(&a, &coins("THTH").unwrap()),
        9.0 / 14.0,
        1e-12
    ));
}

#[test]
fn degenerate_binomials() {
    assert_eq!(binomial_central(20, 0.0, 0.95), (0, 0));
    assert_eq!(binomial_central(20, 1.0, 0.95), (20, 20));
    assert_eq!(binomial_central(0, 0.5, 0.95), (0, 0));
}
