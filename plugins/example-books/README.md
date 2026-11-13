# example-books

A reference book source plugin, built for `wit/kaeri-books.wit`. Build it with:

```sh
cargo build --release --target wasm32-wasip2 --manifest-path plugins/example-books/Cargo.toml
```

The component lands at `target/wasm32-wasip2/release/kaeri_example_books_plugin.wasm`. Copy it
into the plugin directory (`kaeri --plugins` prints it). Kaeri loads book sources only while
reading is on.

It exists to be read. It uses every part of the book ABI: the manifest with its media and
languages, the lent `fetch`, regex and `config-get`, title hints, image and text chapters (one
with a picture the host fetches from `pages.example`) and the error vocabulary. Its two series
are made up, and its hosts, `books.example` and `pages.example`, stand for your own server.
