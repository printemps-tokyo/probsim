use probsim::benford::{
    self, analyze, count_digits, csv_fields, expected, fibonacci, leading_digit, leading_digit_of,
    number_tokens, powers_of_two, scan_csv,
};
use probsim::paradox::{birthday_probability, secretary_best_skip, secretary_success, Paradox};
use probsim::rng::Rng;
use probsim::stats::{chi_square_p, harmonic, ln_gamma, wilson, Running, Z95};

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
fn rng_stays_in_range() {
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
    // Coupon variance for n = 2: draws = 1 + Geometric(1/2), variance 2.
    assert!(close(
        Paradox::Coupon { items: 2 }.theory_variance(),
        2.0,
        1e-12
    ));
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
fn benford_digits() {
    let total: f64 = (1..=9).map(expected).sum();
    assert!(close(total, 1.0, 1e-12));
    assert_eq!(leading_digit(0.0123), Some(1));
    assert_eq!(leading_digit(-5e-7), Some(5));
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
fn powers_of_two_match_exact_values_across_the_switch() {
    // Exact leading digits of 2^k up to k = 200, by doubling a decimal string.
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
    assert!(texts("no numbers here").is_empty());
    assert!(number_tokens("1,234 and 5")
        .iter()
        .map(|t| t.grouped)
        .eq([true, false]));
}

#[test]
fn leading_digits_as_written() {
    assert_eq!(leading_digit_of("9999999999999999999"), Some(9));
    assert_eq!(leading_digit_of("1e400"), Some(1));
    assert_eq!(leading_digit_of("2e-400"), Some(2));
    assert_eq!(leading_digit_of("-0.00071"), Some(7));
    assert_eq!(leading_digit_of("0.000"), None);
    assert_eq!(leading_digit_of("0e5"), None);
}

#[test]
fn csv_columns() {
    assert_eq!(
        csv_fields("a,\"1,234\",\"say \"\"hi\"\"\"", ','),
        vec!["a", "1,234", "say \"hi\""]
    );
    let csv = "id,amount\n1,250\n2,\"1,300\"\n3,0\n4,\n";
    let s = scan_csv(csv, 2, ',');
    assert_eq!(s.numbers, 3);
    assert_eq!(s.zeros, 1);
    assert_eq!(s.counts[1], 1); // 250
    assert_eq!(s.counts[0], 1); // 1,300
    let t = scan_csv("x\ty\n7\t8\n", 1, '\t');
    assert_eq!(t.counts[6], 1);
}

#[test]
fn benford_test_tells_uniform_from_benford() {
    let fits = analyze(count_digits(&powers_of_two(10_000)));
    assert!(fits.p_value.unwrap() > 0.5);
    let flat: Vec<usize> = (0..9_000).map(|i| i % 9 + 1).collect();
    let a = analyze(count_digits(&flat));
    assert_eq!(a.total, 9_000);
    assert!(a.p_value.unwrap() < 1e-10);
    // Too few numbers for the chi-square approximation: no p-value.
    assert_eq!(analyze(count_digits(&[1, 2, 9])).p_value, None);
    assert_eq!(analyze([0; 9]).p_value, None);
    assert!(analyze(count_digits(&powers_of_two(110))).p_value.is_some());
    // Counting straight from the source gives the same counts.
    assert_eq!(
        benford::Source::Fibonacci(500).counts(),
        count_digits(&fibonacci(500))
    );
    assert_eq!(
        benford::Source::PowersOfTwo(500).counts(),
        count_digits(&powers_of_two(500))
    );
}

#[test]
fn files_more_like_real_life() {
    // A quoted field may span lines (RFC 4180).
    let s = scan_csv(
        "note,amount\n\"first\nsecond\",250\n\"x\",\"1,300\"\r\n",
        2,
        ',',
    );
    assert_eq!((s.numbers, s.counts[1], s.counts[0]), (2, 1, 1));
    // A point with no digits after it is still part of the number.
    assert_eq!(
        texts("1.e3 2.e4 0.e5 3."),
        vec!["1.e3", "2.e4", "0.e5", "3."]
    );
    // Encodings.
    let utf16le: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("123\n456\n".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    assert_eq!(
        benford::scan_text(&benford::decode(&utf16le).unwrap()).numbers,
        2
    );
    let utf16be: Vec<u8> = [0xFE, 0xFF]
        .into_iter()
        .chain("78".encode_utf16().flat_map(u16::to_be_bytes))
        .collect();
    assert_eq!(benford::decode(&utf16be).unwrap(), "78");
    let no_bom: Vec<u8> = "123".encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert!(benford::decode(&no_bom).is_err());
    assert_eq!(
        benford::scan_text(&benford::decode(b"\x94\x84 12").unwrap()).numbers,
        1
    );
}
