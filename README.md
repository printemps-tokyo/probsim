# probsim

> Watch probability paradoxes converge, next to their exact answers: Monty Hall, the birthday paradox, gacha completion, the secretary problem, Buffon's needle, the base rate fallacy, Penney's game, the boy or girl paradox, Bertrand's paradox and Benford's law (also on your own data). Zero-dependency Rust CLI.

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
like 1/sqrt(n) (where its normal approximation holds). Explanations are in
English or Japanese (`--lang ja`).

Beyond single runs:

- `--explain N` tells the first trials step by step.
- `sweep` varies one parameter and shows exact and simulated side by side.
- `coverage` repeats a run to show what "95% confidence" means.
- `play monty-hall` lets you play the game yourself.
- `--csv` and `--svg` export for spreadsheets and slides.

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
probsim list                                  # the paradoxes
probsim monty-hall                            # 100,000 games
probsim monty-hall --explain 5                # the first 5 games, step by step
probsim monty-hall --random-host              # the careless host: now it is 1/2
probsim birthday --people 40
probsim gacha --items 50                      # draws to collect 50 items
probsim gacha --weights 1,1,1,1,0.2           # one rare item
probsim gacha --items 10 --budget 30          # chance of completing within 30 draws
probsim base-rate --prevalence 1%             # a positive test for a rare disease
probsim penney --first HTH                    # Penney's game; the second picks the best reply
probsim boy-girl                              # the Tuesday boy
probsim bertrand --method midpoint            # one of three random chords
probsim benford --file sales.csv --column 3   # your own numbers
probsim sweep birthday --vary people --from 1 --to 60
probsim coverage monty-hall --runs 200
probsim play monty-hall
probsim all --check                           # every paradox, exit 1 if one is off
```

```console
$ probsim all --seed 7
paradox            exact   simulated  95% CI
monty-hall       0.66667     0.66540  [0.66247, 0.66832]      ok
birthday         0.50730     0.50798  [0.50488, 0.51108]      ok
coupon             29.29       29.34  [29.27, 29.41]          ok
secretary        0.37104     0.37214  [0.36915, 0.37514]      ok
buffon           0.31831     0.31923  [0.31635, 0.32213]      ok
base-rate        0.16667     0.17405  [0.16467, 0.18385]      ok
penney           0.75000     0.75050  [0.74781, 0.75317]      ok
boy-girl         0.48148     0.47807  [0.46968, 0.48647]      ok
bertrand         0.33333     0.33521  [0.33229, 0.33814]      ok
benford                -     p=1.000  powers2 100,000         ok

100,000 trials each, seed 7 (each paradox gets its own stream from it; see --json)
```

| Option | Description | Default |
| --- | --- | --- |
| `--trials N` | Trials, 1 to 100,000,000; for `benford` sequences, the number of terms | `100000` (`sweep` 10,000 per point, `coverage` 1,000 per run) |
| `--explain N` | Tell the first N trials step by step (1 to 100); they are the first trials of the run itself | |
| `--seed N` | Seed for a repeatable run, 0 to 2^53 - 1 (so it survives JSON); always printed | random |
| `--check` | Exit 1 if an exact answer is outside the 99.9% interval (benford: p < 0.001); exit 2 if there are too few trials to judge | |
| `--json` | JSON instead of text | |
| `--csv` | CSV instead of text: a run's checkpoints, a sweep's points, or a coverage study's runs | |
| `--svg PATH` | Also write the convergence chart as an SVG file | |
| `--no-chart` | Leave out the chart | |
| `--width N` | Chart width in columns, 10 to 200 | `60` |
| `--lang L` | `auto`, `ja` or `en`; `auto` reads `LC_ALL`, `LC_MESSAGES`, `LANG` | `auto` |

Exit codes: 0 on success, 1 when `--check` finds a result off, 2 on a usage
error or when `--check` cannot judge.

## The paradoxes

| Command | Question | Options |
| --- | --- | --- |
| `monty-hall` | You pick a door; the host opens goat doors. Switch? Switching wins (n-1)/n. With `--random-host` the host opens doors at random, and among the games where no car was revealed, switching wins only 1/2. | `--doors N` (3), `--random-host` |
| `birthday` | How likely is a shared birthday? 50.7% for 23 people. With `--day-weights`, days have the chances you give (a file of one weight per day); uneven chances never make a match less likely. | `--people N` (23), `--days N` (365), `--day-weights PATH` |
| `coupon` (`gacha`) | Draws to collect every item: n x H_n on average, 29.3 for 10 items, with the median and the 90th and 99th percentiles. `--weights` gives items unequal rates; `--budget` asks for the chance of completing within a number of draws. | `--items N` (10), `--weights LIST`, `--weights-file PATH`, `--budget N` |
| `secretary` | Hire the best candidate, deciding on the spot. Skip the first ~37%, then take the first who beats them all. | `--candidates N` (100), `--skip K` (the best cutoff) |
| `buffon` | A needle of length l on lines t apart crosses with probability 2l/(pi t), so needles estimate pi. Only l/t matters. | `--length L` (1), `--spacing T` (2), L <= T |
| `base-rate` | A disease in 1% of people, a test that finds 99% of cases and flags 5% of the healthy: a positive test means sickness only 16.7% of the time. | `--prevalence P` (0.01), `--sensitivity P` (0.99), `--specificity P` (0.95); `5%` works too |
| `penney` | Two players pick coin sequences; the one whose sequence appears first wins. Whatever the first picks, the second can pick one that wins more often: against HHT, THH wins 3/4. | `--first SEQ` (HHT), `--second SEQ` (Conway's best reply) |
| `boy-girl` | Two children, at least one a boy born on a Tuesday: both boys with probability 13/27; without the Tuesday, 1/3. | `--any-day` |
| `bertrand` | A random chord is longer than the inscribed triangle's side with probability 1/3, 1/2 or 1/4, depending on how it is drawn. | `--method endpoints\|radius\|midpoint` |
| `benford` | In data that follows Benford's law, digit d leads with probability log10(1 + 1/d): 30.1% for 1. | `--source powers2\|fibonacci`, `--file PATH` (`-` for standard input), `--column N`, `--delimiter C`, `--digits 1\|2` |

The conditional questions (the careless host, a positive test, a family with
such a boy) count only the trials that match the condition, and say how many
were left out.

### Gacha: unequal rates and the long tail

```console
$ probsim gacha --weights 1,1,1,1,0.2 --seed 1 --no-chart
...

  Intuition: About 5 draws, maybe a few more.
  Truth:     23.0 draws on average (computed exactly from the rates).
  Simulated: 22.84  95% CI [22.72, 22.96]  (100,000 collectors, seed 1)
             the exact answer is inside the 95% interval
             Half of the collectors were done within 16 draws, 90% within 47
             draws, 99% within 94 draws: the average hides a long tail of
             unlucky collectors.
  Why:       The rarest item dominates: it alone takes 21.0 draws on average
...
```

With unequal rates the mean comes from the integral of
1 - prod(1 - e^(-p_i t)) (Flajolet, Gardy and Thimonier, 1992). The chance of
completing within a budget is exact for equal rates (a Markov chain on the
number collected) and, by inclusion-exclusion, for up to 20 items with unequal
rates.

### Sweeps

`probsim sweep <paradox> --vary NAME --from A --to B [--steps N]` runs the
paradox at each value and shows the exact and simulated answers together. Each
point has its own random stream derived from the seed. Number parameters can
be swept: `doors`, `people`, `days`, `items`, `budget`, `candidates`, `skip`,
`length`, `spacing`, `prevalence`, `sensitivity`, `specificity`.

```console
$ probsim sweep secretary --vary skip --from 0 --to 90 --steps 10 --trials 5000 --seed 1 --width 50
secretary: the best is hired as skip changes (5,000 trials per point)

        skip       exact   simulated  95% CI
           0     0.01000     0.01020  [0.00777, 0.01339]
          10     0.23484     0.22700  [0.21560, 0.23882]
          20     0.32593     0.32160  [0.30879, 0.33468]
          30     0.36472     0.36200  [0.34879, 0.37542]
          40     0.36953     0.35920  [0.34601, 0.37260]
          50     0.34909     0.34980  [0.33670, 0.36313]
          60     0.30850     0.31560  [0.30286, 0.32862]
          70     0.25118     0.24300  [0.23131, 0.25508]
          80     0.17952     0.18300  [0.17253, 0.19396]
          90     0.09533     0.09360  [0.08584, 0.10199]

  0.36953 |                @     @
          |           @               @
          |                                 @
          |
          |     @                                @
          |
          |                                            @
          |
          |                                                 @
          |
          |
  0.01000 |@
          +--------------------------------------------------
           0                                               90
  o exact   * simulated   @ both
seed 1 (each point gets its own stream from it)
```

### What 95% confidence means

`probsim coverage <paradox> --runs N` repeats a run with independent seeds and
draws each 95% interval against the exact answer (`|`). About 95% of them
contain it; any single one either does or does not.

```console
$ probsim coverage monty-hall --runs 12 --trials 300 --seed 5 --width 44
What a 95% confidence interval means: 12 runs of monty-hall with 300 trials each.

                   [----|-----*---------]      
                        [----------*--------]  <- misses
                       [|--------*---------]   
                 [------|--*---------]         
                      [-|--------*--------]    
                     [--|------*---------]     
               [--------|-*---------]          
             [----------*---------]            
           [----------*-|-------]              
            [----------*|--------]             
                  [-----|----*--------]        
                  [-----|----*--------]        
                     exact 0.66667

11 of 12 intervals (91.7%) contained the exact answer. A 95% interval is a
method that catches the answer in about 95% of runs; any one interval either
contains it or does not. (With this many runs, a correct 95% method catches it
in 83.3% to 100.0% of them, 95% of the time.)
```

### Play

`probsim play monty-hall` asks you for a door, opens the others but one, and
asks whether to switch; after each game it compares your switching and
staying record with the exact answers. Type `q` to stop. Answers are read line
by line, so it also works from a pipe.

### Benford's law on your own data

`probsim benford --file data.csv` counts the leading digits of the numbers in
a text file, runs a chi-square test against Benford's law, and gives Nigrini's
mean absolute deviation (MAD) with his conformity ranges (close, acceptable,
marginally acceptable, nonconformity: 0.006, 0.012 and 0.015 for the first
digit; 0.0012, 0.0018 and 0.0022 for the first two). These are empirical rules
of thumb from Nigrini's work on Benford's law, not statistical tests; unlike
the p-value, the MAD does not call large data "different" just because it is
large.

- `--digits 2` tests the first two digits (10 to 99), the sharper test for
  finding where data departs from the law. A number with one significant digit
  counts as d0 (7 as 70).
- `--column N` reads only column N (1-based) of a CSV file, with quoted
  fields handled (they may span lines); `--delimiter` sets the separator (`,`
  by default, `tab` for TSV). Use it for CSV files: without it every number in
  the file counts, and "1,250" is read as one thousand two hundred fifty,
  where in a CSV it may be two columns. probsim says how many numbers it read
  that way.
- `--file -` reads standard input.
- The leading digits are taken from the number as written, so very long
  numbers and exponents are exact. Zeros have no leading digit and are left
  out (and counted in the report).
- Encodings: UTF-8 and other ASCII-compatible encodings (Shift_JIS, EUC-JP,
  Latin-1, ...) work as they are, since only the ASCII digits are read;
  UTF-16 needs a byte order mark.
- The chi-square test needs every value to be expected at least 5 times: 110
  numbers for the first digit, 1,146 for the first two; below that probsim
  shows the counts and the MAD without a p-value.
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
  candidates are really shuffled, the coins are really flipped. Nothing uses the
  formula it is checked against. Buffon's needle and Bertrand's chords do not use
  pi either: directions come from random points in a disk. `--explain` runs the
  same code with the same random numbers, so the story is exactly what was
  simulated.
- The exact answers are computed directly: the birthday product (and, for
  uneven days, n! times the elementary symmetric polynomial), n x H_n, the
  secretary sum (k/n) sum 1/j, Bayes' rule, Conway's leading numbers for
  Penney's game (the odds for B over A are (AA - AB) : (BB - BA)), and so on.
  The tests check them against published values or brute-force counts.
- Intervals: Wilson score intervals for probabilities (with no successes the
  lower bound is exactly 0, with all successes the upper bound exactly 1), and
  mean +- z x standard error for averages, given only from 30 trials up.
  The dots on the chart are exact +- 1.96 x sd / sqrt(n), from the exact
  standard deviation of one trial: by the normal approximation, about 95% of
  runs are inside at each point. They are drawn only where that holds (n p
  and n (1 - p) at least 5 for a probability, 30 trials for an average), and
  not for gacha with unequal rates, where the exact variance is not computed.
- `--check` uses 99.9% intervals, so with enough trials a correct simulation
  fails one check about once in 1,000 runs; `probsim all` makes ten checks
  (about 1%). In `all` each paradox gets its own random stream derived from
  the seed; `--json` lists every one. `sweep` and `coverage` have no `--check`.
- Benford: powers of two and Fibonacci numbers are exact while they fit in
  128 bits (2^127, F_186) and are computed from logarithms after that; a
  chi-square p-value comes from the regularized incomplete gamma function.

## License

[MIT](./LICENSE) (c) printemps.tokyo
