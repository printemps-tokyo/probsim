use probsim::paradox::Paradox;
use probsim::sim::{checkpoint_counts, simulate};

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
        assert!(
            o.consistent(Z999),
            "{p:?}: estimate {} vs theory {}",
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
}

#[test]
fn checkpoints_are_log_spaced_and_end_at_the_total() {
    let c = checkpoint_counts(100_000, 6);
    assert_eq!(c, vec![1, 10, 100, 1_000, 10_000, 100_000]);
    assert_eq!(checkpoint_counts(1, 60), vec![1]);
    assert_eq!(checkpoint_counts(0, 60), Vec::<u64>::new());
    let o = simulate(&Paradox::Coupon { items: 5 }, 5_000, 1, 40);
    assert_eq!(o.checkpoints.last().unwrap().n, 5_000);
    assert!(o.checkpoints.windows(2).all(|w| w[0].n < w[1].n));
}

#[test]
fn coupon_with_one_item_takes_exactly_one_draw() {
    let o = simulate(&Paradox::Coupon { items: 1 }, 1_000, 1, 10);
    assert_eq!(o.estimate, 1.0);
}
