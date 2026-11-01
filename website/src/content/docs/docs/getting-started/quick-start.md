---
title: Quick start
description: Browsing works at once, with no account and no config; playback, sync and plugins are set up when you want them.
---

In the apps, browsing starts on Continue, with what airs today and this season. Plugins, plugin
stores and tracker accounts are added in the app. The rest of this page is about the terminal app.

Start it from an installer or archive:

```sh
kaeri
```

Or from a source checkout:

```sh
cargo run --release
```

Search, seasonal charts and the airing calendar come from AniList's public API. Your watch history
is saved on your device, so none of it needs an account.

To play more than the built-in demo, add a source: your own files, a server you host, or a plugin
store you join with an invite link from its owner.

Sync to AniList, MyAnimeList, Kitsu, Simkl or Trakt is opt-in. In the terminal app, Kitsu is on by
default and needs one sign-in on the Accounts screen, and the others need an app you register. The
native apps sign in to all five from Accounts with nothing to register. See
[Trackers and sync](/docs/guides/trackers-sync/).

Plugins add sources. If someone you know runs a [plugin store](/docs/plugins/stores/), open
Sources (`7`), press Enter, choose **Join a store** and paste their invite link. To make your own, see
[Writing a plugin](/docs/plugins/authoring/).

## First keys to know

Press `?` for every key, including any [overrides](/docs/guides/keybindings-cli/) you set.

| | |
|---|---|
| `/` | Search |
| `Enter` | Open · `Space` play next unwatched |
| `e` | Episodes · `s` sources · `?` all keys |
| `:` or `Ctrl-K` | Command palette |
| `Q` | Quit |

If images are missing or nothing plays, run `kaeri --doctor`.
It reports what your terminal supports and whether mpv is reachable.
