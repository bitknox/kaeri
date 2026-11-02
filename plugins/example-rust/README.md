# example-rust

A reference provider plugin. Build it with:

```sh
cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-rust/Cargo.toml
```

The component lands at `target/wasm32-wasip2/release/kaeri_example_plugin.wasm`. Copy it into
the plugin directory (`kaeri --plugins` prints it) and Kaeri will pick it up.

It exists to be *read*: it exercises every part of the ABI — the manifest, the lent `fetch`, the
lent regex, the error vocabulary — against a made-up host, so the shape of a real
plugin is visible without needing a live server to be up.
