# test-hostile-books

A book source plugin that misbehaves on purpose, for `crates/kaeri-plugin/tests/book_conformance.rs`.
It is test infrastructure, not an example. Each call tries something the sandbox must stop:
running forever, allocating without bound, fetching a host it never declared, naming page images
on such a host and reading a setting it was never granted.

```sh
cargo build --release --target wasm32-wasip2 --manifest-path plugins/test-hostile-books/Cargo.toml
```
