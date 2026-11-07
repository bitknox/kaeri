# Fuzzing the book readers

One target per reader: `zip`, `rar`, `sevenz`, `pdf`, and `epub` (container, package,
navigation, encryption and page XML inside a generated ZIP). Every book file is untrusted input,
and these run before a release that changes a reader.

With cargo-fuzz and a nightly toolchain:

```bash
cd crates/kaeri-books/fuzz
cargo run --release --bin seeds -- corpus
cargo +nightly fuzz run rar corpus/rar -- -max_total_time=300 -rss_limit_mb=2048
```

On stable, without the address sanitizer (the crate forbids `unsafe`, so panics, overflows,
hangs and memory blowups are what is being looked for). The seeds binary is built without the
coverage flags, and the targets with them and an explicit `--target`, so build scripts stay
plain:

```bash
cd crates/kaeri-books/fuzz
CARGO_TARGET_DIR=target/plain cargo run --release --bin seeds -- corpus
export RUSTFLAGS="--cfg fuzzing -Cdebug-assertions -Coverflow-checks \
  -Cpasses=sancov-module -Cllvm-args=-sanitizer-coverage-level=4 \
  -Cllvm-args=-sanitizer-coverage-inline-8bit-counters \
  -Cllvm-args=-sanitizer-coverage-pc-table -Cllvm-args=-sanitizer-coverage-trace-compares \
  -Clink-arg=-Wl,-undefined,dynamic_lookup"
cargo build --release --bins --target aarch64-apple-darwin
target/aarch64-apple-darwin/release/rar corpus/rar -max_total_time=300 -rss_limit_mb=2048 -timeout=10
```

The last link flag is for macOS, where one dependency also builds as a dynamic library that
cannot resolve the coverage hooks on its own. The `fuzzing` feature lets decoder panics through
to the fuzzer instead of turning them into errors, so a crash shows up as one. The corpus and
any crash files stay out of git.
