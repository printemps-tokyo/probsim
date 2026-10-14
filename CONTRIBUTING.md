# Contributing

Thanks for your interest in contributing to probsim.

## Development

1. Fork / clone the repository
2. Install a stable Rust toolchain (https://rustup.rs)
3. Create a branch: `git switch -c feat/your-change`
4. Make your change and verify locally:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all
   cargo build --release
   ./target/release/probsim all --seed 2026 --check
   ```
5. Commit and open a pull request

## Adding a paradox

- Add a variant to `Paradox` in `src/paradox.rs` with its exact answer
  (`theory`), one trial (`trial`) and its texts in English and Japanese.
- The trial must simulate the situation itself; it must not use the formula
  it is compared with.
- Cite a source for the exact answer in the pull request, and add a test in
  `tests/math.rs` that checks it against a published value, and one in
  `tests/simulate.rs` that the simulation agrees with it.
- Keep probsim free of dependencies.
