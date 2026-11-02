---
title: Writing a plugin
description: 'Plugins are WebAssembly components in any WASI 0.2 language: the interface, the error rules, building and publishing.'
---

A plugin is a WebAssembly component: search terms in, episodes and stream URLs out.
Kaeri loads `*.wasm` from the plugin directory (see
[where things live](/docs/getting-started/installation/#where-things-live), or run
`kaeri --plugins`) and treats it like a native source, with the same ranking, health tracking
and failover. Any WASI 0.2 language works; reference implementations in Rust and JavaScript live in
`plugins/`.

## Before you write a plugin

A plugin connects Kaeri to a source: your own server, an API whose terms allow a third-party
client, or a catalogue licensed for it. You are responsible for your plugin, for the sources it
reaches and for the rights it relies on. Write plugins only for sources you, and the people who
install them, are entitled to use. A plugin must not get round DRM, paywalls, sign-ins or other
access controls. Kaeri does not review, sign or distribute plugins; stores are run by the
people who own them.

:::note
The sandbox has no sockets. The host makes requests for the plugin through `fetch`, so a plugin
turns responses into results. [The plugin sandbox](/docs/plugins/sandbox/) covers what is enforced.
:::

## The interface

`wit/kaeri-provider.wit` is the contract, package `kaeri:provider@2.0.0`:

```wit
interface host {                          // what you may call
    fetch:          func(req: http-request) -> result<http-response, host-error>;
    log:            func(level: string, msg: string);
    regex-captures: func(pattern: string, haystack: string) -> list<list<string>>;
    fetch-many:     func(reqs: list<http-request>) -> list<result<http-response, host-error>>;
    config-get:     func(key: string) -> option<string>;
}

interface provider {                      // what you must export
    describe:       func() -> manifest;
    search:         func(query: string, translation: string) -> result<list<search-hit>, provider-error>;
    list-episodes:  func(id: string, translation: string) -> result<list<episode>, provider-error>;
    resolve:        func(id: string, episode: string, translation: string)
                      -> result<list<media-stream>, provider-error>;
    sources:        func(id: string, episode: string, translation: string)
                      -> result<list<source-candidate>, provider-error>;
    resolve-source: func(id: string, episode: string, translation: string, source-id: string)
                      -> result<list<media-stream>, provider-error>;
}
```

- `fetch-many`: several requests concurrently, results in request order. Same allowlist and
  fetch budget, per request. Return *all* your streams from `resolve`: playback falls over to
  the next stream when one produces no frames, so the order only sets which stream is tried first.
- `sources`: the selectable releases for the Sources overlay, best-first. An empty list means there is
  nothing to choose between, and it is not an error.
- `resolve-source`: one candidate's id back into streams. Never fall back to the automatic pick.
- `config-get`: reads `[providers.plugins.settings.<your-id>]` from the user's config.toml.
  Every key is optional, so work with all of them unset.

Record fields are documented in the WIT. These are easy to miss: `synonyms`/`format` on a search hit (title
matching), `description`/`thumbnail`/`air-date`/`filler` on an episode, `format` on a subtitle
whose URL has no extension, `download-source` on a stream.

Errors map one-for-one onto the registry's failover rules:

| Return | Meaning | Fails over |
|---|---|---|
| `not-found` | Works, no such title or episode | Yes, not counted as a fault |
| `blocked(msg)` | The host refused the request or asked the client to slow down | Yes |
| `parse(msg)` | Response arrived, did not look like expected | Yes |
| `other(msg)` | Anything else | Yes |

Do not flatten `not-found` into `other`. A missing title is an answer, and reported as one: the
Sources screen and the failure message would otherwise show a fault where the source only lacks
the title.

## Rust

```sh
cargo build --release --target wasm32-wasip2 \
  --manifest-path plugins/example-rust/Cargo.toml
# then copy the .wasm into the plugin directory; `kaeri --plugins` prints it
```

Start from `plugins/example-rust/src/lib.rs`. It exercises the whole ABI against a stable
endpoint.

## JavaScript

```sh
cd plugins/example-ts && npm install && npm run build
# then copy kaeri-example-plugin-ts.wasm into the plugin directory
```

Five differences from Rust:

- Imports carry the package version: `kaeri:provider/host@2.0.0`.
- Names are lowerCamelCase: `list-episodes` → `listEpisodes`.
- Errors are thrown, not returned: `throw { tag: 'not-found' }`. `fetch` needs `try`/`catch`.
- Exports run without a receiver: `this.resolve(…)` traps, so share a plain function.
- `--disable all` strips WASI.

## Versions

A plugin is built against one major version of the WIT file. The host refuses a plugin built for
another major with a message that says it needs rebuilding against the current WIT file.

## Cost

Measured with `cargo run -p kaeri-plugin --example plugin_bench --release`:

| | size | compile (once) | per call |
|---|---|---|---|
| Rust | 0.1 MB | 19 ms | 38 µs |
| JavaScript | 12.2 MB | 1.06 s | 923 µs |

Both run under the same default limits; both are negligible beside one HTTP request. Every call
gets a fresh store, so no state survives between calls.

## Publishing

To share a plugin, publish it to a [plugin store](/docs/plugins/stores/), or hand over the `.wasm`
for someone to put in their plugin directory. Either way, declare the narrowest `allowed-hosts`
that works, since Kaeri shows it to users, name yourself clearly in `display-name`, and
version `describe()`.

A store reads the id, version and hosts from the plugin itself, and checks them:

- The id is 1 to 64 characters of `a`–`z`, `0`–`9`, `-` and `_`, and does not start with `-` or
  `_`. It cannot be `torrent`, `remote` or `plugins`, the names of Kaeri's own sources. The id
  becomes the file name on every member's machine, so use one no other plugin uses.
- The version is 1 to 64 letters, digits and `.+-_`, and does not start with `.`.
- A published version never changes. Publish a new version for any change, however small.

A store that cannot run WebAssembly, such as one on Cloudflare Workers, Vercel or Netlify, cannot
ask the plugin what it is. It needs the plugin to carry its manifest in the file, and refuses one
without it. Kaeri adds the manifest when you publish from the app, and
`kaeri plugin stamp plugin.wasm` adds it to a file you publish some other way.
