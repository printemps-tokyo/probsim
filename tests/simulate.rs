use std::sync::Arc;

use probsim::paradox::{coins, Chord, Lang, Paradox, Weights};
use probsim::rng::Rng;
use probsim::sim::{checkpoint_counts, derived_seed, quantile, simulate};
use probsim::stats::Z95;
use probsim::study;

const Z999: f64 = 3.290_526_731_491_926;

fn coupon(items: u32) -> Paradox {
    Paradox::Coupon {
        items,
        weights: None,
        budget: None,
    }
}

fn weights(w: &[f64]) -> Option<Arc<Weights>> {
    Some(Arc::new(Weights::new(w).unwrap()))
}

fn all() -> Vec<Paradox> {
    vec![
        Paradox::MontyHall {
            doors: 3,
            host_knows: true,
        },
        Paradox::MontyHall {
            doors: 5,
            host_knows: true,
        },
        Paradox::MontyHall {
            doors: 3,
            host_knows: false,
        },
        Paradox::MontyHall {
            doors: 6,
            host_knows: false,
        },
        Paradox::Birthday {
            people: 23,
            days: 365,
            weights: None,
        },
        Paradox::Birthday {
            people: 5,
            days: 10,
            weights: None,
        },
        Paradox::Birthday {
            people: 4,
            days: 3,
            weights: weights(&[1.0, 2.0, 3.0]),
        },
        Paradox::Birthday {
            people: 2,
            days: 3,
            weights: weights(&[1.0, 2.0, 3.0]),
        },
        coupon(10),
        coupon(1),
        Paradox::Coupon {
            items: 3,
            weights: weights(&[1.0, 1.0, 0.2]),
            budget: None,
        },
        Paradox::Coupon {
            items: 10,
            weights: None,
            budget: Some(30),
        },
        Paradox::Coupon {
            items: 3,
            weights: weights(&[1.0, 2.0, 5.0]),
            budget: Some(8),
        },
        Paradox::Secretary {
            candidates: 100,
            skip: 37,
        },
        Paradox::Secretary {
            candidates: 10,
            skip: 0,
        },
        Paradox::Buffon {
            length: 1.0,
            spacing: 2.0,
        },
        Paradox::Buffon {
            length: 1.0,
            spacing: 1.0,
        },
        Paradox::BaseRate {
            prevalence: 0.01,
            sensitivity: 0.99,
            specificity: 0.95,
        },
        Paradox::BaseRate {
            prevalence: 0.3,
            sensitivity: 0.8,
            specificity: 0.7,
        },
        Paradox::Penney {
            first: coins("HHT").unwrap(),
            second: coins("THH").unwrap(),
        },
        Paradox::Penney {
            first: coins("HTH").unwrap(),
            second: coins("HHT").unwrap(),
        },
        Paradox::Penney {
            first: coins("HHTH").unwrap(),
            second: coins("THTT").unwrap(),
        },
        Paradox::BoyGirl { tuesday: true },
        Paradox::BoyGirl { tuesday: false },
        Paradox::Bertrand {
            method: Chord::Endpoints,
        },
        Paradox::Bertrand {
            method: Chord::Radius,
        },
        Paradox::Bertrand {
            method: Chord::Midpoint,
        },
    ]
}

#[test]
fn every_simulation_agrees_with_its_exact_answer() {
    // Fixed seeds, so this is deterministic; at 99.9% a correct simulation
    // could still miss once in a thousand seeds, and these seeds do not.
    for (i, p) in all().iter().enumerate() {
        let o = simulate(p, 200_000, 1000 + i as u64, 60);
        assert_eq!(
            o.consistent(Z999),
            Some(true),
            "{p:?}: {} vs {}",
            o.estimate,
            o.theory
        );
        assert_eq!(o.attempted, 200_000);
    }
}

#[test]
fn conditional_questions_count_only_the_matching_trials() {
    // The careless host reveals no car in 2/n of games.
    let o = simulate(
        &Paradox::MontyHall {
            doors: 3,
            host_knows: false,
        },
        300_000,
        5,
        10,
    );
    assert!((o.counted as f64 / o.attempted as f64 - 2.0 / 3.0).abs() < 0.005);
    // 27 of 196 families have a Tuesday boy.
    let b = simulate(&Paradox::BoyGirl { tuesday: true }, 300_000, 5, 10);
    assert!((b.counted as f64 / b.attempted as f64 - 27.0 / 196.0).abs() < 0.003);
    // P(positive) = 0.01 * 0.99 + 0.99 * 0.05.
    let r = simulate(
        &Paradox::BaseRate {
            prevalence: 0.01,
            sensitivity: 0.99,
            specificity: 0.95,
        },
        300_000,
        5,
        10,
    );
    assert!((r.counted as f64 / r.attempted as f64 - 0.0594).abs() < 0.002);
}

#[test]
fn same_seed_same_result() {
    let p = Paradox::Birthday {
        people: 23,
        days: 365,
        weights: None,
    };
    let (a, b) = (simulate(&p, 10_000, 9, 30), simulate(&p, 10_000, 9, 30));
    assert_eq!(a.estimate, b.estimate);
    assert_eq!(a.checkpoints, b.checkpoints);
    assert_ne!(simulate(&p, 10_000, 10, 30).estimate, a.estimate);
    let o = simulate(
        &Paradox::MontyHall {
            doors: 3,
            host_knows: true,
        },
        1_000,
        1,
        10,
    );
    assert_eq!(o.estimate, o.successes as f64 / 1_000.0);
}

#[test]
fn checkpoints_are_log_spaced_and_end_at_the_total() {
    assert_eq!(
        checkpoint_counts(100_000, 6),
        vec![1, 10, 100, 1_000, 10_000, 100_000]
    );
    assert_eq!(checkpoint_counts(1, 60), vec![1]);
    assert_eq!(checkpoint_counts(0, 60), Vec::<u64>::new());
    let o = simulate(&coupon(5), 5_000, 1, 40);
    assert_eq!(o.checkpoints.last().unwrap().n, 5_000);
    assert!(o.checkpoints.windows(2).all(|w| w[0].n < w[1].n));
    let r = simulate(
        &Paradox::MontyHall {
            doors: 1000,
            host_knows: false,
        },
        200_000,
        1,
        60,
    );
    assert!(r.checkpoints.len() > 30, "{}", r.checkpoints.len());
}

#[test]
fn coupon_distribution_gives_quantiles() {
    let o = simulate(&coupon(10), 100_000, 3, 10);
    let d = o.distribution.as_ref().unwrap();
    assert_eq!(d.values().sum::<u64>(), 100_000);
    assert!(
        *d.keys().next().unwrap() >= 10,
        "no one completes 10 items in fewer than 10 draws"
    );
    let median = quantile(d, 0.5).unwrap();
    assert!((26..=28).contains(&median), "{median}");
    assert!(quantile(d, 0.99).unwrap() > quantile(d, 0.9).unwrap());
    assert!(simulate(
        &Paradox::MontyHall {
            doors: 3,
            host_knows: true
        },
        100,
        1,
        10
    )
    .distribution
    .is_none());
}

#[test]
fn certain_outcomes_always_pass() {
    let certain = [
        Paradox::Birthday {
            people: 2,
            days: 1,
            weights: None,
        },
        Paradox::Birthday {
            people: 1,
            days: 365,
            weights: None,
        },
        Paradox::Secretary {
            candidates: 1,
            skip: 0,
        },
        Paradox::Coupon {
            items: 2,
            weights: None,
            budget: Some(1),
        },
    ];
    for p in certain {
        for trials in [1, 2, 3, 7, 100, 1000] {
            let o = simulate(&p, trials, 1, 10);
            assert_eq!(o.consistent(Z999), Some(true), "{p:?} {trials}");
            assert_eq!(o.consistent(Z95), Some(true), "{p:?} {trials}");
        }
    }
}

#[test]
fn averages_need_enough_trials_to_be_judged() {
    let few = simulate(&coupon(2), 2, 2, 10);
    assert_eq!(few.consistent(Z999), None);
    assert_eq!(few.ci95, None);
    assert!(simulate(&coupon(2), 30, 2, 10).ci95.is_some());
}

#[test]
fn buffon_depends_only_on_the_ratio() {
    let a = simulate(
        &Paradox::Buffon {
            length: 1.0,
            spacing: 2.0,
        },
        20_000,
        3,
        10,
    );
    let b = simulate(
        &Paradox::Buffon {
            length: 1e300,
            spacing: 2e300,
        },
        20_000,
        3,
        10,
    );
    assert_eq!(a.estimate, b.estimate);
    assert_eq!(a.theory, b.theory);
}

#[test]
fn explain_tells_the_same_trials_the_simulation_runs() {
    for p in all() {
        for lang in [Lang::En, Lang::Ja] {
            let (mut r1, mut r2) = (Rng::new(77), Rng::new(77));
            let (mut s1, mut s2) = (Vec::new(), Vec::new());
            for _ in 0..30 {
                let (told, story) = p.explain(&mut r1, &mut s1, lang);
                assert_eq!(told, p.trial(&mut r2, &mut s2), "{p:?}");
                assert!(!story.is_empty());
            }
            assert_eq!(
                r1.next_u64(),
                r2.next_u64(),
                "explaining must not use extra random numbers: {p:?}"
            );
        }
    }
}

#[test]
fn derived_seeds_fit_in_json_numbers_and_differ() {
    let mut seeds: Vec<u64> = (0..6).map(|i| derived_seed(7, i)).collect();
    assert!(seeds.iter().all(|&s| s < 1 << 53));
    seeds.sort();
    seeds.dedup();
    assert_eq!(seeds.len(), 6);
}

#[test]
fn sweeps_and_coverage() {
    assert_eq!(
        study::sweep_values(1.0, 5.0, 5, true),
        vec![1.0, 2.0, 3.0, 4.0, 5.0]
    );
    assert_eq!(study::sweep_values(1.0, 3.0, 10, true), vec![1.0, 2.0, 3.0]);
    assert_eq!(study::sweep_values(0.0, 1.0, 3, false), vec![0.0, 0.5, 1.0]);
    let pts = study::sweep(
        &[10.0, 23.0, 40.0],
        |v| {
            Ok(Paradox::Birthday {
                people: v as u32,
                days: 365,
                weights: None,
            })
        },
        20_000,
        4,
    )
    .unwrap();
    assert_eq!(pts.len(), 3);
    assert!(pts
        .windows(2)
        .all(|w| w[0].outcome.theory < w[1].outcome.theory));
    assert!(pts.iter().all(|p| p.outcome.consistent(Z999) == Some(true)));
    assert!(study::sweep(&[1.0], |_| Err("bad".to_string()), 10, 1).is_err());
    // About 95% of 95% intervals contain the exact answer.
    let runs = study::coverage(
        &Paradox::MontyHall {
            doors: 3,
            host_knows: true,
        },
        400,
        2_000,
        9,
    );
    let hits = runs.iter().filter(|r| r.hit).count();
    assert_eq!(runs.len(), 400);
    assert!((365..=395).contains(&hits), "{hits}");
    // Averages from too few trials have no interval and are left out.
    assert!(study::coverage(&coupon(3), 10, 5, 1).is_empty());
}

#[test]
fn nothing_counted_means_no_interval() {
    // A disease nobody has and a perfect test: no one tests positive.
    let p = Paradox::BaseRate {
        prevalence: 1e-100,
        sensitivity: 0.99,
        specificity: 1.0,
    };
    let o = simulate(&p, 10, 1, 5);
    assert_eq!(o.counted, 0);
    assert_eq!(o.ci95, None);
    assert_eq!(o.consistent(Z95), None);
    assert!(
        study::coverage(&p, 5, 3, 1).is_empty(),
        "runs with nothing counted are not hits"
    );
}

#[test]
fn sweep_values_stay_in_range_and_errors_name_the_value() {
    assert_eq!(study::sweep_values(1.5, 3.5, 3, true), vec![2.0, 3.0]);
    assert_eq!(
        study::sweep_values(5.0, 1.0, 5, true),
        vec![5.0, 4.0, 3.0, 2.0, 1.0]
    );
    let err = study::sweep(
        &[1.0, 2.0],
        |v| {
            if v > 1.5 {
                Err("bad".into())
            } else {
                Ok(coupon(2))
            }
        },
        10,
        1,
    )
    .unwrap_err();
    assert_eq!(err, "at 2: bad");
}
