# example-ts

The same reference provider as `example-rust`, in JavaScript — proof the ABI is
language-agnostic. It shares nothing with the host but `wit/kaeri-provider.wit`, and
`tests/conformance.rs` runs the same assertions against both.

```sh
npm install
npm run build      # → kaeri-example-plugin-ts.wasm
```

Copy the `.wasm` into the plugin directory (`kaeri --plugins` prints it).

The component is 12 MB and ~1 s to compile because it embeds a JS engine; per call it is still
negligible beside one HTTP request, and it runs under the same default limits as the Rust one.
The JavaScript-specific pitfalls — versioned imports (`kaeri:provider/host@2.0.0`),
lowerCamelCase names, thrown errors, no `this` in exports, `--disable all` — are listed in
[docs/plugins.md](../../docs/plugins.md).
