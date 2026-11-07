# Writing a plugin

## Before you write a plugin

A plugin connects Kaeri to a source: your own server, an API whose terms allow a third-party
client, or a catalogue licensed for it. You are responsible for your plugin, for the sources it
reaches and for the rights it relies on. Write plugins only for sources you, and the people who
install them, are entitled to use. A plugin must not get round DRM, paywalls, sign-ins or other
access controls. Kaeri does not review, sign or distribute plugins; stores are run by the
people who own them.

A plugin is a WebAssembly component: search terms in, episodes and stream URLs out. A book
source plugin is the same with chapters and pages in place of episodes and streams; see
[Book sources](#book-sources).
Kaeri loads `*.wasm` from the plugin directory (`kaeri --plugins` prints it) and treats it
like a native source, with the same ranking, health tracking and failover. Any WASI 0.2 language
works, and reference implementations in Rust and JavaScript live in `plugins/`.

**The sandbox has no sockets. The host lends `fetch`.** The host makes each request, enforces
`allowed-hosts`, rate-limits and keeps the cookie jar, so your plugin turns responses into results. It cannot reach
anything outside its declared `allowed-hosts`.

## The interface

`wit/kaeri-provider.wit` is the contract, package `kaeri:provider@3.0.0`:

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
    search:         func(query: string, translation: string, hints: title-hints)
                      -> result<list<search-hit>, provider-error>;
    list-episodes:  func(id: string, translation: string, hints: title-hints)
                      -> result<list<episode>, provider-error>;
    resolve:        func(id: string, episode: string, translation: string, hints: title-hints)
                      -> result<list<media-stream>, provider-error>;
    sources:        func(id: string, episode: string, translation: string)
                      -> result<list<source-candidate>, provider-error>;
    resolve-source: func(id: string, episode: string, translation: string, source-id: string)
                      -> result<list<media-stream>, provider-error>;
}
```

- `fetch-many`: several requests concurrently, results in request order. Same allowlist and
  fetch budget, per request. Return *all* your streams from `resolve`: playback falls over to
  the next stream when one produces no frames, so ordering is a hint, not a verdict.
- `hints`: what the host knows about the title, passed to `search`, `list-episodes` and `resolve`:
  an optional AniList id (`anilist-id`), the romaji, English and native titles (each optional) and
  a list of synonyms. `query` stays the host's first choice of words. Every field is optional and
  you can ignore all of it. A source with its own index can look the title up by AniList id, and
  one that searches by text can retry with another name when the first finds nothing. A title
  Kaeri made up for a file it could not match has no AniList id, so the field is absent.
- `sources`: the selectable releases for the Sources overlay, best-first. An empty list means
  "nothing to choose between", which is an answer and not a failure.
- `resolve-source`: one candidate's id back into streams. Never fall back to the automatic pick.
- `config-get`: reads `[providers.plugins.settings.<your-id>]` from the user's config.toml.
  Every key is optional, so work with all of them unset.

## Torrents

Kaeri finds no torrents and ranks none. A plugin that finds them declares the `torrents`
capability in its manifest and returns `torrent` streams: `url` is a magnet link or a bare info
hash (40 hex or 32 base32 characters), and `file-index` names the file to play, counting from 0,
when the torrent holds more than one. Without it the host plays the file whose name carries the
episode number, else the largest video.

The host turns each into a magnet link, adds the user's trackers and plays it through torrent
playback, through the user's proxy when one is set. It starts the first torrent in your order and
tries the next only when that one fails. While torrent playback is off or a proxy check has not
passed, your source is passed over as unavailable, which is not counted against it. A plugin
without the capability has its torrent streams dropped. `torrent-http` is the host's own
loopback stream and is not accepted from a plugin.

Ranking is yours: `resolve` returns your pick, `sources` your candidates best first, and `label`
on a candidate is one short line of your own, shown as written (a peer count, say).

Record fields are documented in the WIT. Easy to miss: `synonyms`/`format` on a search hit (title
matching), `description`/`thumbnail`/`air-date`/`filler` on an episode, `format` on a subtitle
whose URL has no extension, `download-source` on a stream.

Errors map one-for-one onto the registry's failover rules:

| Return         | Meaning                                         | Failover?                   |
| -------------- | ----------------------------------------------- | --------------------------- |
| `not-found`    | Works, no such title or episode                 | Yes, not counted as a fault |
| `blocked(msg)` | The host refused it or asked to slow down       | Yes                         |
| `parse(msg)`   | Response arrived, did not look like expected    | Yes                         |
| `other(msg)`   | Anything else                                   | Yes                         |

Do not flatten `not-found` into `other`. A missing title is an answer, and reported as one: the
Sources screen and the failure message would otherwise show a fault where the source only lacks
the title.

## Book sources

A plugin that finds manga, comics or novels is built for a second contract,
`wit/kaeri-books.wit`, package `kaeri:books@1.0.0`. It is its own package, so a book source needs
that one file, and the two contracts change on their own majors. The host is the same: the same
five lent functions with the same allowlist, the same sandbox and the same limits.

```wit
interface source {                        // what a book source exports
    describe:        func() -> manifest;
    search:          func(query: string, hints: title-hints)
                       -> result<list<series-hit>, source-error>;
    series:          func(id: string, hints: title-hints)
                       -> result<series-detail, source-error>;
    list-chapters:   func(id: string, hints: title-hints)
                       -> result<list<chapter>, source-error>;
    chapter-content: func(series: string, chapter: string)
                       -> result<content, source-error>;
    page-bytes:      func(series: string, chapter: string, page: string)
                       -> result<list<u8>, source-error>;
}

record page { image: page-image, width: option<u32>, height: option<u32> }
variant page-image {
    url(page-url),                        // an address the host fetches, with its headers
    plugin(string),                       // the plugin's own id; the host asks page-bytes
}
```

- The manifest adds `media` (`manga`, `novel`) and `languages` (BCP 47 tags). Media the host
  does not know are ignored.
- `hints` carries the AniList id, a MyAnimeList manga id, the titles and synonyms, and the medium
  the host wants, every field optional.
- `list-chapters` returns every chapter the source lists, every version and language. `number` is
  the source's own label, kept as written (`10.5`, `12-13`, `Extra`); the host reads a number from
  it. `version-label` tells two versions of one chapter apart.
- `chapter-content` returns `images`, the pages in reading order, or `text`, an XHTML body
  fragment that the host sanitises before showing it.
- A page is usually a `url`: the plugin names the address and the headers it needs, and the host
  fetches it through the plugin's own `allowed-hosts`, so list the page image hosts there too. A
  download fetches the pages again later the same way, and an address that answers 403 or 410
  makes the host ask `chapter-content` once more for fresh ones.
- A page the host cannot fetch as one plain request is a `plugin` page: the plugin names it with
  an id of its own, and the host calls `page-bytes` with the series, the chapter and that id when
  the page is wanted. A server that hands chapters over only as CBZ or ZIP archives is one case,
  with the plugin reading the page out of the archive; a page that needs a signed request is
  another. The host asks for one page at a time, when it needs that page, so a chapter's pages
  are never all in memory at once. Every call starts
  fresh, so a page read from an archive fetches the archive again; a server that answers range
  requests lets the plugin fetch only that page's part.
- `page-bytes` runs under the same limits as every other call, and the host checks what comes
  back before it keeps it. A page over 16 MiB, one whose first bytes are not a JPEG, PNG, WebP,
  GIF or AVIF file, and one whose header claims more than 40 million pixels are refused without
  being decoded. A refused or stopped page is the plugin's `parse` error, with the same failover
  and health rules. A plugin page has no address to expire, so the 403 and 410 rule does not
  apply; the plugin answers with its own errors.
- The errors are the show sources' errors, with the same failover rules.

The host tells the two kinds apart by what a component imports and exports, so the kind comes
from the plugin itself. Book sources go in the same plugin directory. Kaeri loads them only while
reading is available, and a version without reading leaves them alone. A book source cannot take
the id `folders` or `servers`, the names of Kaeri's own book sources.

Start from `plugins/example-books/src/lib.rs`. It uses every part of the contract against
made-up hosts, and answers everything but `search` from data it keeps. Its "Extra" chapter is one
archive, and its pages are `plugin` pages read out of it.

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

- Imports carry the package version: `kaeri:provider/host@3.0.0`.
- Names are lowerCamelCase: `list-episodes` → `listEpisodes`.
- Errors are thrown, not returned: `throw { tag: 'not-found' }`. `fetch` needs `try`/`catch`.
- Exports run without a receiver: `this.resolve(…)` traps, so share a plain function.
- `--disable all` strips WASI.

## Versions

A plugin is built against one major version of the WIT file. The host refuses a plugin built for
another major with a message that says it needs rebuilding against the current WIT file.

Major 3 added the `hints` argument to `search`, `list-episodes` and `resolve`. A plugin built for
major 2 is refused with a message that says to rebuild it; rebuild against the current WIT file and
add the argument to those three functions.

`kaeri:books` is at major 1. A plugin built for a later major of either contract is refused with a
message that says it needs a newer Kaeri.

Adding a field to a record changes the contract. The component model checks an export's types
when a plugin is instantiated, and it matches a record by its exact list of fields and a variant
by its exact list of cases. A plugin built before a field was added, even an `option` field, fails
to load, and so does one built before a case or a function was added. The conformance suite proves
this with a grown copy of the book contract (`a_record_that_grows_a_field_is_a_different_world` and
`a_variant_that_grows_a_case_is_a_different_world` in `crates/kaeri-plugin/tests/book_conformance.rs`).
So every change to the records, variants or functions a plugin exports is a new major. The
contracts grow without one in two ways: a plugin declares new behaviour as a `capabilities`
string, which an older host ignores, and a value it needs from the person comes through
`config-get`.

## Cost

Measured with `cargo run -p kaeri-plugin --example plugin_bench --release`:

|            | size    | compile (once) | per call |
| ---------- | ------- | -------------- | -------- |
| Rust       | 0.1 MB  | 19 ms          | 38 µs    |
| JavaScript | 12.2 MB | 1.06 s         | 923 µs   |

Both run under the same default limits; both are negligible beside one HTTP request. Every call
gets a fresh store, so no state survives between calls.

## The sandbox

All limits are enforced host-side. `memory_mb` and `deadline_secs` are configurable under
`[providers.plugins]`; the fetch budget and body ceiling are fixed. Raising a ceiling cannot
grant a new capability.

| Limit           | Default                   |
| --------------- | ------------------------- |
| Reachable hosts | manifest `allowed-hosts`  |
| Memory          | 64 MiB                    |
| Deadline        | 20 s per call             |
| Fetches         | 12 per call               |
| Response body   | 8 MiB                     |
| Methods         | GET, POST                 |

The allowlist: http/https only; exact host or subdomain; no credentials in the URL; no loopback or
private addresses even if declared; redirects are not followed, and the next hop is allowlisted
again. Reserved headers (`host`, `content-length`, `connection`, `transfer-encoding`,
`accept-encoding`) are refused; `referer`, `cookie` and `user-agent` are yours. `config-get` sees
only your own settings table, and the `describe` that reads your manifest at load time runs with
no capabilities at all: no settings, no allowlist, no client.

WASI is linked, and the context is what withholds: no preopened directories, so nothing on disk is
openable; every socket address is refused; no environment, no stdio. A guest can read the clock and
take random bytes, and that is the whole extent of it.

## Testing and publishing

```sh
cargo test -p kaeri-plugin       # skips itself if components are not built
```

The conformance suite runs the same assertions against both references, plus `test-hostile`,
which spins, allocates without bound, fetches an undeclared host and probes undeclared settings.
Copy the `assert_reference_behaviour` shape for your own plugin. `book_conformance.rs` does the
same for book sources against `example-books` and `test-hostile-books`, which also names page
images on a host it never declared and makes pages that break each of the page limits.

To share a plugin, publish it to a plugin store ([plugin-stores.md](plugin-stores.md)), or hand
over the `.wasm` for someone to put in their plugin directory. Either way, declare the narrowest
`allowed-hosts` that works, since Kaeri shows it to users, name yourself clearly in
`display-name`, and version `describe()`.

A store reads the id, version and hosts from the plugin itself, and checks them:

- The id is 1 to 64 characters of `a`–`z`, `0`–`9`, `-` and `_`, and does not start with `-` or
  `_`. It cannot be `torrent`, `remote`, `plugins`, `folders` or `servers`, the names of
  Kaeri's own sources. The id
  becomes the file name on every member's machine, so use one no other plugin uses.
- The version is 1 to 64 letters, digits and `.+-_`, and does not start with `.`.
- A published version never changes. Publish a new version for any change, however small.

A store that cannot run WebAssembly, such as one on Cloudflare Workers, Vercel or Netlify, cannot
ask the plugin what it is. It needs the plugin to carry its manifest in an `kaeri:manifest`
custom section, and refuses one without it. Kaeri adds the section when you publish from the
app. For a plugin you publish some other way, stamp it first:

```sh
kaeri plugin stamp plugin.wasm            # in place, or --out stamped.wasm
```

Stamping loads the plugin with no capabilities, asks it to describe itself and writes the answer
into the file. For a book source the answer also says it is one, and which media it serves. A store that can run the plugin checks the section against the plugin's own answer.
Every member's app loads the plugin before installing it and refuses one whose answer differs from
the store's list, so nothing trusts the section further than that.
