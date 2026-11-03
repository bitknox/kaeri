---
title: Torrents
description: Torrent playback is off by default and finds no torrents of its own. A proxy for its traffic is optional.
---

:::caution[Nothing to search included]
Kaeri doesn't host any shows. It finds no torrents, ranks none and ships no plugin that does. Torrent
playback plays a torrent a [source plugin](/docs/plugins/authoring/) names, or a magnet link or
torrent file you open yourself. You are responsible for what you play and for having the right to
play it, under the laws that apply to you. Torrent playback is off by default.
:::

Kaeri provides torrent playback, a librqbit session that streams over loopback HTTP. A source
plugin that declares the `torrents` capability names torrents, in its own order, and they play
through it. Without such a plugin, nothing plays through torrents until you open a link:
`kaeri --open-torrent <magnet link, info hash or file.torrent>`.

```toml
[providers.torrent]
enabled = true

# Optional: route torrent traffic through a SOCKS5 proxy.
proxy = "socks5://192.0.2.10:1080"

# Needed with a proxy: DHT is off through a proxy, so without trackers there are no peers.
trackers = ["udp://tracker.example:1337/announce"]
```

Streams play while they download, and you can seek in them.

## In the apps

The apps keep the same settings under **Settings › Sources**. The **Torrents** switch turns torrent
playback on straight away. Torrents connect directly unless you set a proxy.

## Routing torrent traffic through a proxy

Other people in a torrent can see your IP address. To keep it private, turn on **Route torrent
traffic through a proxy** and give a SOCKS5 proxy you run or rent: its host, port, and a user name
and password if it asks for them. This is optional and off by default.

A proxy is checked before it is saved. Kaeri connects to it and completes the SOCKS5 handshake,
and the app says it is checking. Only a passing check saves the proxy. A failed check saves
nothing, says "The proxy check failed", and keeps the proxy you had. Once a proxy is set,
**Edit proxy…** opens its form filled in with the saved host, port and user name. Leave the
password empty to keep the saved one, or type a new one to replace it. The password is kept in
the system's secret store, never in `config.toml`.

While a proxy is set:

- The torrent session goes through it, and DHT is off, since SOCKS5 has no reliable way to carry
  it. Add trackers so peers can be found.
- Torrents start once the proxy check passes. While it fails, they wait and Kaeri tries again
  every so often, so a proxy that comes up after launch brings torrents up without a restart.
  The interface does not wait for it.
- The terminal's header shows `Proxy ● Passed`, `Proxy ● Checking` or `Proxy ✕ Check failed`.
  **Check now** in the apps checks again at once.

Turn the setting off, or remove the `proxy` key, and torrents connect directly.

On Android TV, the proxy comes from a page the TV opens on your phone through a QR code, or you
type it with the TV's keyboard. The TV does not download episodes.

An earlier `[providers.torrent.vpn]` table still loads: its `socks_url` becomes `proxy`, and the
table is dropped the next time Kaeri saves a setting.

## Watched vs. downloaded

Playing while downloading means an episode can *count* before its file is complete.
[Watched versus resumable](/docs/guides/configuration/#watched-versus-resumable) keeps those ideas
apart: `commit_threshold` decides when it is watched, and resume is tracked separately.

Seeding after an episode is watched, and after a download finishes, is off by default to save
upload; turn it on to give back to the swarm. An episode still uploads to its swarm while it plays,
as [Seeding](#seeding) describes. Completed downloads get subtitles muxed in with a stream copy.

## Seeding

An episode uploads to its swarm while it plays. What happens after
it has been watched is up to you, on the Settings screen under Sources or in `config.toml`:

```toml
[providers.torrent.seeding]
ratio = 1.0              # seed each watched episode until it has given back what it took; 0 is off
hours = 24               # the longest one episode keeps seeding, whatever its ratio
upload_limit_kib = 1024  # total upload in KiB/s, 0 for unlimited
```

The ratio counts what this machine has of the torrent, so an episode you only partly streamed is
not asked to upload more than it fetched. Seeding only runs while Kaeri is open, a handful of
episodes at a time. The Downloads screen
lists what is still seeding, how close each episode is to its ratio and how long it has left.

The upload limit covers all torrent traffic, playback included. On a slow uplink it keeps seeding
from crowding out the requests the episode you are watching needs.

Seeding after watching is off by default to save upload; turn it on to give back to the swarm.

What streamed episodes leave on disk, and the limit on it, is covered under
[Disk space](/docs/guides/configuration/#disk-space).
