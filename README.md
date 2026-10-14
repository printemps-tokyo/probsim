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
  0.73211 |^                      .
          |                        ..
          |                          ...
          |                             ....
          |                                 ..........
  0.66667 |--------*------------*-------*----------**********
          |                  *** *    ** **********...
          |                 *    *   *  ....
          |                 *     *  ...
          |               **       **
  0.60122 |   v vvvvvvvvvv        .
          +--------------------------------------------------
           1                      316                    100k
  - exact answer   * this run   . where about 95% of runs stay (normal approximation, drawn where it holds)
```

Each paradox comes with the question, what intuition says, the exact answer
and why, then a simulation with a 95% confidence interval and a chart of the
running estimate settling onto the exact value, inside a funnel that narrows
like 1/sqrt(n) (where its normal approximation holds). `--explain N` tells the first trials step by step.
Explanations are in English or Japanese (`--lang ja`).

## Install

From source, with a Rust toolchain 1.82 or later (https://rustup.rs):

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
probsim monty-hall --explain 5           # the first 5 games, step by step
probsim monty-hall --doors 10            # host opens 8 goat doors
probsim monty-hall --random-host         # the careless host: now it is 1/2
probsim birthday --people 40
probsim coupon --items 50                # gacha: draws to collect 50 items
probsim secretary --candidates 1000
probsim buffon                           # estimate pi
probsim benford --file sales.csv --column 3   # your own numbers
probsim all --check                      # every paradox, exit 1 if one is off
```

```console
$ probsim monty-hall --random-host --explain 3 --trials 10 --seed 3
The first 3 games, step by step:
  Game 1: car behind door 3, you pick door 2; the host, opening doors at
          random, leaves door 1 closed and reveals the car: this game does not
          count
  Game 2: car behind door 2, you pick door 2; the host leaves door 1 closed
          and opens the other door; switching to door 1 loses
  Game 3: car behind door 1, you pick door 3; the host, opening doors at
...
```

```console
$ probsim all --seed 7
paradox            exact   simulated  95% CI
monty-hall       0.66667     0.66540  [0.66247, 0.66832]      ok
birthday         0.50730     0.50798  [0.50488, 0.51108]      ok
coupon             29.29       29.34  [29.27, 29.41]          ok
secretary        0.37104     0.37214  [0.36915, 0.37514]      ok
buffon           0.31831     0.31923  [0.31635, 0.32213]      ok
benford                -     p=1.000  powers2 100,000         ok

100,000 trials each, seed 7 (each paradox gets its own stream from it; see --json)
```

| Option | Description | Default |
| --- | --- | --- |
| `--trials N` | Trials, 1 to 100,000,000; for `benford` sequences, the number of terms | `100000` |
| `--explain N` | Tell the first N trials step by step (1 to 100); they are the first trials of the run itself | |
| `--seed N` | Seed for a repeatable run, 0 to 2^53 - 1 (so it survives JSON); always printed | random |
| `--check` | Exit 1 if an exact answer is outside the 99.9% interval (benford: p < 0.001); exit 2 if there are too few trials to judge | |
| `--json` | JSON instead of text | |
| `--no-chart` | Leave out the convergence chart | |
| `--width N` | Chart width in columns, 10 to 200 | `60` |
| `--lang L` | `auto`, `ja` or `en`; `auto` reads `LC_ALL`, `LC_MESSAGES`, `LANG` | `auto` |

Exit codes: 0 on success, 1 when `--check` finds a result off, 2 on a usage
error or when `--check` cannot judge.

## The paradoxes

| Command | Question | Options |
| --- | --- | --- |
| `monty-hall` | You pick a door; the host opens goat doors. Switch? Switching wins (n-1)/n. With `--random-host` the host opens doors at random, and among the games where no car was revealed, switching wins only 1/2. | `--doors N` (3), `--random-host` |
| `birthday` | How likely is a shared birthday? 50.7% for 23 people. | `--people N` (23), `--days N` (365) |
| `coupon` (`gacha`) | Draws to collect every item: n x H_n on average, 29.3 for 10 items. | `--items N` (10) |
| `secretary` | Hire the best candidate, deciding on the spot. Skip the first ~37%, then take the first who beats them all. | `--candidates N` (100), `--skip K` (the best cutoff) |
| `buffon` | A needle of length l on lines t apart crosses with probability 2l/(pi t), so needles estimate pi. Only l/t matters. | `--length L` (1), `--spacing T` (2), L <= T |
| `benford` | In data that follows Benford's law, digit d leads with probability log10(1 + 1/d): 30.1% for 1. | `--source powers2\|fibonacci`, `--file PATH`, `--column N`, `--delimiter C` |

### Benford's law on your own data

`probsim benford --file data.csv` counts the leading digits of the numbers in
a text file and runs a chi-square test against Benford's law (8 degrees of
freedom).

- `--column N` reads only column N (1-based) of a CSV file, with quoted
  fields handled; `--delimiter` sets the separator (`,` by default, `tab` for
  TSV). Use it for CSV files: without it every number in the file counts,
  and "1,250" is read as one thousand two hundred fifty, where in a CSV it may
  be two columns. probsim says how many numbers it read that way.
- The leading digit is taken from the number as written, so very long numbers
  and exponents are exact. Zeros have no leading digit and are left out (and
  counted in the report).
- Encodings: UTF-8 and other ASCII-compatible encodings (Shift_JIS, EUC-JP,
  Latin-1, ...) work as they are, since only the ASCII digits are read;
  UTF-16 needs a byte order mark. A quoted CSV field may span lines.
- The test needs at least 110 numbers, so that every digit is expected at
  least 5 times; below that probsim shows the counts without a p-value.
- Benford's law fits data spanning several orders of magnitude (populations,
  prices, transaction amounts). Data with a narrow range or assigned numbers
  (heights, IDs, phone numbers) is not expected to follow it, so a mismatch is
  not by itself a sign of anything wrong. For the built-in sequences (powers of
  two, Fibonacci) the p-value treats them as if they were a random sample.

## How it works

- Random numbers: xoshiro256** seeded through splitmix64, written in the
  crate (no `rand`) and tested against the values of the authors' reference C
  code. Integers in a range use Lemire's method, without modulo bias. The same
  seed gives the same random numbers everywhere; results that also go through
  floating-point functions (logarithms, for example) are expected to match but
  not promised bit for bit across platforms.
- Every trial is simulated step by step: the host really picks a door, the
  candidates are really shuffled. Nothing uses the formula it is checked
  against. Buffon's needle does not use pi either: a needle's angle comes
  from a random point in a quarter disk. `--explain` runs the same code with
  the same random numbers, so the story is exactly what was simulated.
- The exact answers are computed directly (the birthday product, n x H_n,
  the secretary sum (k/n) sum 1/j, and so on).
- Intervals: Wilson score intervals for probabilities (with no successes the
  lower bound is exactly 0, with all successes the upper bound exactly 1), and
  mean +- z x standard error for averages, given only from 30 trials up.
  The dots on the chart are exact +- 1.96 x sd / sqrt(n), from the exact
  standard deviation of one trial: by the normal approximation, about 95% of
  runs are inside at each point. They are drawn only where that holds (n p
  and n (1 - p) at least 5 for a probability, 30 trials for an average).
- `--check` uses 99.9% intervals, so with enough trials a correct simulation
  fails one check about once in 1,000 runs; `probsim all` makes six checks
  (about 0.6%). In `all` each paradox gets its own random stream derived from
  the seed; `--json` lists every one.
- Benford: powers of two and Fibonacci numbers are exact while they fit in
  128 bits (2^127, F_186) and are computed from logarithms after that; a
  chi-square p-value comes from the regularized incomplete gamma function.

## License

[MIT](./LICENSE) (c) printemps.tokyo
