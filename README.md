# probsim

> Watch probability paradoxes converge, next to their exact answers: Monty Hall, the birthday paradox, gacha completion, the secretary problem, Buffon's needle and Benford's law (also on your own data). Zero-dependency Rust CLI.

[![CI](https://github.com/printemps-tokyo/probsim/actions/workflows/ci.yml/badge.svg)](https://github.com/printemps-tokyo/probsim/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

```console
$ probsim monty-hall --seed 42 --width 50
Monty Hall problem (3 doors, the host knows)

There are 3 doors: a car behind one, goats behind the rest. You pick a door.
The host, who knows where the car is, opens another door, a goat, and offers
you the one door left. Should you switch?

  Intuition: Two doors are left, so it is 50/50 and switching makes no
             difference.
  Truth:     Switching wins 2/3 of the time (66.7%).
  Simulated: 0.66553  95% CI [0.66260, 0.66845]  (100,000 games, seed 42)
             the exact answer is inside the 95% interval
  Why:       Your first pick is right only 1/3 of the time. The host never
             opens the car, so in every other case the car is behind the door
             he leaves closed.

Convergence: switching wins, by number of games (log scale)
  0.73211 |^
          |
          |
          |
          |
  0.66667 |--------*------------*-------*----------**********
          |                  *** *    ** **********
          |                 *    *   *
          |                 *     *
          |               **       **
  0.60122 |   v vvvvvvvvvv
          +--------------------------------------------------
           1                      316                    100k
```

Each paradox comes with the question, what intuition says, the exact answer
and why, then a simulation with a 95% confidence interval and a chart of the
running estimate settling onto the exact value. Explanations are in English
or Japanese (`--lang ja`).

## Install

From source, with a Rust toolchain (https://rustup.rs):

```bash
cargo install --git https://github.com/printemps-tokyo/probsim
```

Tagged [releases](https://github.com/printemps-tokyo/probsim/releases) carry
prebuilt binaries for Linux (x86_64) and macOS (x86_64, Apple silicon), with
SHA-256 checksums. No dependencies beyond the Rust standard library.

## Usage

```bash
probsim list                             # the paradoxes
probsim monty-hall                       # 100,000 games
probsim monty-hall --doors 10            # host opens 8 goat doors
probsim monty-hall --random-host         # the careless host: now it is 1/2
probsim birthday --people 40
probsim coupon --items 50                # gacha: draws to collect 50 items
probsim secretary --candidates 1000
probsim buffon                           # estimate pi
probsim benford --file sales.csv         # your own numbers
probsim all --check                      # every paradox, exit 1 if one is off
```

```console
$ probsim all --seed 7
paradox            exact   simulated  95% CI
monty-hall       0.66667     0.66566  [0.66273, 0.66858]      ok
birthday         0.50730     0.50878  [0.50568, 0.51188]      ok
coupon             29.29       29.27  [29.20, 29.34]          ok
secretary        0.37104     0.36831  [0.36533, 0.37130]      ok
buffon           0.31831     0.31768  [0.31480, 0.32057]      ok
benford                -     p=1.000  powers2 100,000         ok

100,000 trials each, seed 7
```

| Option | Description | Default |
| --- | --- | --- |
| `--trials N` | Trials, 1 to 100,000,000; for `benford` sequences, the number of terms | `100000` |
| `--seed N` | Seed for a repeatable run; always printed | random |
| `--check` | Exit 1 if an exact answer is outside the 99.9% interval (benford: p < 0.001) | |
| `--json` | JSON instead of text | |
| `--no-chart` | Leave out the convergence chart | |
| `--width N` | Chart width in columns, 10 to 200 | `60` |
| `--lang L` | `auto`, `ja` or `en`; `auto` reads `LC_ALL`, `LC_MESSAGES`, `LANG` | `auto` |

Exit codes: 0 on success, 1 when `--check` finds a result off, 2 on a usage
error.

## The paradoxes

| Command | Question | Options |
| --- | --- | --- |
| `monty-hall` | You pick a door; the host opens goat doors. Switch? Switching wins (n-1)/n. With `--random-host` the host opens doors at random, and among the games where no car was revealed, switching wins only 1/2. | `--doors N` (3), `--random-host` |
| `birthday` | How likely is a shared birthday? 50.7% for 23 people. | `--people N` (23), `--days N` (365) |
| `coupon` (`gacha`) | Draws to collect every item: n x H_n on average, 29.3 for 10 items. | `--items N` (10) |
| `secretary` | Hire the best candidate, deciding on the spot. Skip the first ~37%, then take the first who beats them all. | `--candidates N` (100), `--skip K` (the best cutoff) |
| `buffon` | A needle of length l on lines t apart crosses with probability 2l/(pi t), so needles estimate pi. | `--length L` (1), `--spacing T` (2), L <= T |
| `benford` | Digit d leads with probability log10(1 + 1/d): 30.1% for 1. | `--source powers2\|fibonacci`, `--file PATH` |

### Benford's law on your own data

`probsim benford --file data.csv` takes every number in a text file (CSV,
TSV, logs, anything), counts the leading digits of the nonzero ones, and runs
a chi-square test against Benford's law (8 degrees of freedom). Commas that
group thousands (`1,234,567`) are read as part of the number; other commas
separate numbers, so a CSV field like `1,234` is read as one thousand two
hundred thirty-four. Benford's law fits data spanning several orders of
magnitude (populations, prices, transaction amounts); data with a narrow range
or assigned numbers (heights, IDs, phone numbers) is not expected to follow
it, so a mismatch is not by itself a sign of anything wrong.

## How it works

- Random numbers: xoshiro256** seeded through splitmix64, written in the
  crate (no `rand`), so a seed gives the same result on every machine and
  version. Integers in a range use Lemire's method, without modulo bias.
- Every trial is simulated step by step: the host really picks a door, the
  candidates are really shuffled. Nothing uses the formula it is checked
  against. Buffon's needle does not use pi either: a needle's angle comes
  from a random point in a quarter disk.
- The exact answers are computed directly (the birthday product, n x H_n,
  the secretary sum (k/n) sum 1/j, and so on).
- Intervals: Wilson score intervals for probabilities, mean +- z x standard
  error for averages. A 95% interval misses the exact answer in about 1 run
  in 20, which the text says when it happens; `--check` uses 99.9% so that a
  correct simulation fails it about once in 1,000 runs.
- Benford: powers of two and Fibonacci numbers are exact while they fit in
  128 bits (2^127, F_186) and are computed from logarithms after that; a
  chi-square p-value comes from the regularized incomplete gamma function.

## License

[MIT](./LICENSE) (c) printemps.tokyo
