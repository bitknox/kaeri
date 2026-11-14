---
paths:
  - "crates/kaeri-plugin/**"
  - "plugins/**"
  - "wit/**"
---

# Provider plugin validation

`crates/kaeri-plugin/tests/conformance.rs` and `book_conformance.rs` skip components
that are not built, so a green test result alone does not prove the sandbox or ABI
works. For host, sandbox or WIT changes, build every fixture before running them, and
set `KAERI_REQUIRE_PLUGINS=1` so a missing one fails:

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-rust/Cargo.toml
cargo build --release --target wasm32-wasip2 --manifest-path plugins/test-hostile/Cargo.toml
cargo build --release --target wasm32-wasip2 --manifest-path plugins/test-torrent-source/Cargo.toml
cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-books/Cargo.toml
cargo build --release --target wasm32-wasip2 --manifest-path plugins/test-hostile-books/Cargo.toml
(cd plugins/example-ts && npm ci && npm run build)
KAERI_REQUIRE_PLUGINS=1 cargo test -p kaeri-plugin -- --nocapture
```

Check the output for `skipping` messages and disclose any missing fixture coverage.
Use `docs/plugins.md` for both ABIs (`kaeri:provider` and `kaeri:books`) and the JS
component pitfalls. Built plugin `.wasm`
files are ignored artifacts; the deploy store's committed WASM is a separate workflow.
