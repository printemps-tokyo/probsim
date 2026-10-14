use probsim::paradox::{Lang, Paradox};
use probsim::rng::Rng;
use probsim::sim::{checkpoint_counts, derived_seed, simulate};
use probsim::stats::Z95;

const Z999: f64 = 3.290_526_731_491_926;

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
        },
        Paradox::Birthday {
            people: 5,
            days: 10,
        },
        Paradox::Coupon { items: 10 },
        Paradox::Coupon { items: 1 },
        Paradox::Secretary {
            candidates: 100,
            skip: 37,
        },
        Paradox::Secretary {
            candidates: 10,
            skip: 0,
        },
        Paradox::Secretary {
            candidates: 10,
            skip: 9,
        },
        Paradox::Buffon {
            length: 1.0,
            spacing: 2.0,
        },
        Paradox::Buffon {
            length: 1.0,
            spacing: 1.0,
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
fn the_careless_host_reveals_the_car_in_a_third_of_games() {
    let o = simulate(
        &Paradox::MontyHall {
            doors: 3,
            host_knows: false,
        },
        300_000,
        5,
        10,
    );
    let kept = o.counted as f64 / o.attempted as f64;
    // P(no car revealed) = 2/n.
    assert!((kept - 2.0 / 3.0).abs() < 0.005, "{kept}");
}

#[test]
fn same_seed_same_result() {
    let p = Paradox::Birthday {
        people: 23,
        days: 365,
    };
    let (a, b) = (simulate(&p, 10_000, 9, 30), simulate(&p, 10_000, 9, 30));
    assert_eq!(a.estimate, b.estimate);
    assert_eq!(a.checkpoints, b.checkpoints);
    assert_ne!(simulate(&p, 10_000, 10, 30).estimate, a.estimate);
    // A probability estimate is successes / n exactly, with no running-sum noise.
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
    let o = simulate(&Paradox::Coupon { items: 5 }, 5_000, 1, 40);
    assert_eq!(o.checkpoints.last().unwrap().n, 5_000);
    assert!(o.checkpoints.windows(2).all(|w| w[0].n < w[1].n));
    // With a random host most games do not count, but the chart still gets
    // points all the way along.
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
fn coupon_with_one_item_takes_exactly_one_draw() {
    let o = simulate(&Paradox::Coupon { items: 1 }, 1_000, 1, 10);
    assert_eq!(o.estimate, 1.0);
}

#[test]
fn certain_outcomes_always_pass() {
    let certain = [
        Paradox::Birthday { people: 2, days: 1 },
        Paradox::Birthday {
            people: 1,
            days: 365,
        },
        Paradox::Secretary {
            candidates: 1,
            skip: 0,
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
    let few = simulate(&Paradox::Coupon { items: 2 }, 2, 2, 10);
    assert_eq!(few.consistent(Z999), None);
    assert_eq!(few.ci95, None);
    assert!(simulate(&Paradox::Coupon { items: 2 }, 30, 2, 10)
        .ci95
        .is_some());
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
    let c = simulate(
        &Paradox::Buffon {
            length: 5e-324,
            spacing: 1e-323,
        },
        20_000,
        3,
        10,
    );
    assert_eq!(a.estimate, b.estimate);
    assert_eq!(a.theory, b.theory);
    assert!(c.theory.is_finite() && c.estimate.is_finite());
}

#[test]
fn explain_tells_the_same_trials_the_simulation_runs() {
    for p in [
        Paradox::MontyHall {
            doors: 3,
            host_knows: false,
        },
        Paradox::Birthday {
            people: 23,
            days: 365,
        },
        Paradox::Coupon { items: 6 },
        Paradox::Secretary {
            candidates: 10,
            skip: 3,
        },
        Paradox::Buffon {
            length: 1.0,
            spacing: 2.0,
        },
    ] {
        for lang in [Lang::En, Lang::Ja] {
            let (mut r1, mut r2) = (Rng::new(77), Rng::new(77));
            let (mut s1, mut s2) = (Vec::new(), Vec::new());
            for _ in 0..50 {
                let (told, story) = p.explain(&mut r1, &mut s1, lang);
                assert_eq!(told, p.trial(&mut r2, &mut s2), "{p:?}");
                assert!(!story.is_empty());
            }
            assert_eq!(
                r1.next_u64(),
                r2.next_u64(),
                "explaining must not use extra random numbers"
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
