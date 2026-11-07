# test-hostile-books

A book source plugin that misbehaves on purpose, for `crates/kaeri-plugin/tests/book_conformance.rs`.
It is test infrastructure, not an example. Each call tries something the sandbox must stop:
running forever, allocating without bound, fetching a host it never declared, naming page images
on such a host and reading a setting it was never granted. The pages it makes itself break each
page limit in turn: too many bytes, bytes that are no image, a header that claims too many
pixels, and a page that spins or hoards memory.

```sh
cargo build --release --target wasm32-wasip2 --manifest-path plugins/test-hostile-books/Cargo.toml
```
