<div align="center">

<a href="https://kaeri.app">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/readme/wordmark-dark.webp">
    <source media="(prefers-color-scheme: light)" srcset="docs/assets/readme/wordmark-light.webp">
    <img src="docs/assets/readme/wordmark-light.webp" width="303" alt="kaeri">
  </picture>
</a>

<h3>Browse, watch, track and sync anime in one client.</h3>

<p>
Native apps for Mac, iPhone, iPad, Android, Android TV, Windows and Linux, and one for the terminal.
</p>

<p>
<a href="https://github.com/bitknox/kaeri/releases/latest">Download</a>
&nbsp;·&nbsp;
<a href="https://kaeri.app/apps/">Apps</a>
&nbsp;·&nbsp;
<a href="https://kaeri.app/features/">Features</a>
&nbsp;·&nbsp;
<a href="https://kaeri.app/docs/">Docs</a>
&nbsp;·&nbsp;
<a href="https://kaeri.app/docs/privacy/">Privacy</a>
</p>

<p>
<a href="https://github.com/bitknox/kaeri/releases/latest"><img src="https://img.shields.io/github/v/release/bitknox/kaeri?style=flat-square&label=release&labelColor=161a2e&color=f2a64b" alt="Latest release"></a>
<a href="LICENSE"><img src="https://img.shields.io/github/license/bitknox/kaeri?style=flat-square&label=licence&labelColor=161a2e&color=343a57" alt="MIT licence"></a>
</p>

<br>

<img src="docs/assets/readme/hero.webp" width="884" alt="The Mac app on Continue, with a show in progress and a button to resume its episode, and the iPhone app in front of it on that show's title page">

<sub>The Mac and iPhone apps. Every title and picture here is made up.</sub>

</div>

<br>

Kaeri browses AniList with no account, plays episodes in mpv, keeps your history on your
device and syncs it to AniList, MyAnimeList, Kitsu, Simkl and Trakt. It is free, open source and
non-commercial.

<table>
  <tr>
    <td width="33%" align="center" valign="top">
      <img src="docs/assets/readme/android-continue.webp" width="200" alt="The Android app on Home, with the show in progress, a button to resume it and the shows under way">
      <br><br>
      <b>Continue watching</b>
      <br>
      The shows you're partway through, with the next episode ready to play.
    </td>
    <td width="33%" align="center" valign="top">
      <img src="docs/assets/readme/iphone-calendar.webp" width="200" alt="The iPhone app's calendar: the week's days, then each day's episodes with their times">
      <br><br>
      <b>Calendar and sync</b>
      <br>
      See what airs each day. Your progress syncs to AniList, MyAnimeList, Kitsu, Simkl and Trakt.
    </td>
    <td width="33%" align="center" valign="top">
      <img src="docs/assets/readme/iphone-tv-remote.webp" width="200" alt="The iPhone app as the TV's remote: the episode playing, play and seek buttons, subtitles, audio and the TV's volume">
      <br><br>
      <b>Play on TV</b>
      <br>
      Send an episode to the Android TV app, then use your phone or computer as the remote.
    </td>
  </tr>
</table>

Also in the apps:

- Skip the opening, and start the next episode from the credits.
- Pick sub or dub once and each show remembers.
- Watch with friends in sync, on Mac, iPhone, iPad, Windows, Linux and in the terminal.
- New-episode alerts, off until you turn them on for a show.
- Home screen widgets, and Live Activities and Handoff on Apple devices.

The [features page](https://kaeri.app/features/) lists every feature and which app has it.

## Apps

Each app is built with its platform's toolkit: SwiftUI on Apple, Jetpack Compose on Android,
WinUI 3 on Windows, GTK 4 on Linux and Ratatui in the terminal. They share one engine, written in
Rust.

<table>
  <tr>
    <td width="50%" align="center" valign="top">
      <img src="docs/assets/readme/windows-title.webp" width="410" alt="The Windows app on a title page, with a button to resume episode 7 and the episode list">
      <br>
      <sub>Windows</sub>
    </td>
    <td width="50%" align="center" valign="top">
      <img src="docs/assets/readme/linux-continue.webp" width="410" alt="The Linux app on Home, with the show in progress and a button to resume its episode">
      <br>
      <sub>Linux</sub>
    </td>
  </tr>
</table>

<table>
  <tr>
    <td width="50%" align="center" valign="top">
      <img src="docs/assets/readme/tv-continue.webp" width="410" alt="The Android TV app on Continue, with a large button to resume episode 7 and a row of shows in progress">
      <br>
      <sub>Android TV</sub>
    </td>
    <td width="50%" align="center" valign="top">
      <img src="docs/assets/readme/ipad-title.webp" width="410" alt="The iPad app on a title page, with the seasons, the episode list and the show's details">
      <br>
      <sub>iPad</sub>
    </td>
  </tr>
</table>

### The player

Episodes play in mpv, upscaled by ArtCNN or Anime4K, with a skip button for the opening and Up
Next at the end. The player menu has the episodes, sources, sub or dub, upscaling and tracks.

<img src="assets/screenshots/player-menu.webp" width="884" alt="The subtitles menu inside the player: tracks, then style choices such as size and outline">

<sub>The player frame is from こうしす！ #3.1 by OPAP-JP contributors, licensed
<a href="https://creativecommons.org/licenses/by/4.0/">CC BY 4.0</a>. See
<a href="assets/screenshots/NOTICE.md">the notice</a>.</sub>

### The terminal app

Kaeri also runs in a terminal on macOS, Linux and Windows. On a Mac it shares its history and
config with the Mac app. Covers show as images in Ghostty, Kitty, iTerm2, WezTerm and other
terminals with image support.

<p align="center">
<img src="docs/assets/readme/terminal.webp" width="642" alt="The terminal app on its home screen: the sections down the side, and the show in progress with its cover and how far you are">
</p>

## Download

Every release on the [releases page](https://github.com/bitknox/kaeri/releases/latest) has the
apps and the terminal app. Each file has a `.sha256` beside it. The apps are not in the App Store
or on Google Play, so each one needs a one-time step the first time you open it. The
[installation guide](https://kaeri.app/docs/getting-started/installation/) has the full steps.

| Platform | File | First open |
| --- | --- | --- |
| Mac, macOS 26 or later, Apple silicon | `kaeri-<version>-macos.dmg` | Drag Kaeri to Applications. The app isn't signed with an Apple developer account and isn't notarized, so macOS stops it once. Choose **Done**, then open **System Settings › Privacy & Security** and choose **Open Anyway**. |
| iPhone and iPad, iOS or iPadOS 26 or later | `kaeri-<version>-ios-unsigned.ipa` | Add the source below in [SideStore](https://sidestore.io) or [AltStore](https://altstore.io), then install Kaeri from it. They sign the app with your own Apple ID. |
| Android 15 or later, 64-bit ARM | `kaeri-<version>-android-arm64.apk` | Open the APK on the device and allow installs from your browser or file manager once. |
| Android TV and Google TV, Android 11 or later, 64-bit ARM or 32-bit ARM | `kaeri-<version>-android-tv-arm64.apk` or `kaeri-<version>-android-tv-armv7.apk` | Install it with a sideloading app or `adb install`. |
| Windows 10 version 2004 or later, x64 or Arm64 | `kaeri-<version>-windows-x64-setup.exe` or `kaeri-<version>-windows-arm64-setup.exe` | The installer is not signed: in SmartScreen, choose **More info**, then **Run anyway**. |
| Linux with glibc 2.41 or newer (Ubuntu 25.04 or later, including 26.04 LTS, Fedora 42 or later, Debian 13), x86-64 or ARM64 | `kaeri-<version>-linux-x86_64.AppImage` or `kaeri-<version>-linux-aarch64.AppImage` | `chmod +x`, then open it. Ubuntu 24.04 is not supported. |

The SideStore and AltStore source address, for **Sources** › **+**:

```
https://github.com/bitknox/kaeri/releases/latest/download/sidestore.json
```

<details>
<summary>The terminal app</summary>

<br>

The terminal app comes as an installer or an archive for each system:

| System | Files |
| --- | --- |
| macOS | `kaeri-<version>-aarch64-apple-darwin.pkg` (Apple silicon), `kaeri-<version>-x86_64-apple-darwin.pkg` (Intel), or the `.tar.gz` of either |
| Windows | `kaeri-<version>-setup.exe`, or `kaeri-<version>-x86_64-pc-windows-msvc.zip` |
| Debian, Ubuntu | `kaeri-<version>-amd64.deb` |
| Other Linux | `kaeri-<version>-x86_64.AppImage`: `chmod +x`, then run it from a terminal. The `x86_64-unknown-linux-gnu.tar.gz` holds the binary alone. |

The Linux builds need glibc 2.28 or newer. The terminal app drives mpv and ffmpeg, which are not
bundled: install them first (`brew install mpv ffmpeg` on macOS). The apps have mpv built in. The
terminal app is not code-signed, so macOS and Windows ask you to allow it once; the installation
guide says how.

</details>

<details>
<summary>Updates</summary>

<br>

The apps check GitHub for a new release once a day and say when one is out, and an update keeps
your history and settings. On the Mac, the app downloads the release, checks its signature and
replaces itself. On Android and Android TV, it downloads the APK, checks it and installs it over
the old one. On Windows, it downloads and checks the new installer, then runs it when you choose
Restart to update. On Linux, the AppImage downloads and checks the new version and puts it in
place of the old one. On iPhone and iPad, the notice opens SideStore or AltStore with Kaeri's
source. The terminal app updates with `kaeri --update`.

</details>

## Where episodes come from

Kaeri doesn't host any shows. Apart from a demo of Creative Commons works, it comes with no
sources, and it doesn't list, recommend or link to any. You add the sources you're entitled to use:
your own files, your own media server, a stream you have the right to
play, a licensed service, or a sandboxed WebAssembly plugin from a plugin store you joined. A plugin can only reach
the hosts it declares. Kaeri never gets round DRM: a licensed stream opens in its own service.

A plugin store is a small server for your own plugins and your own devices. Its owner
chooses its plugins and is responsible for them. Devices that join can install the store's
plugins from inside Kaeri and hear about new versions. Each device gets its own token, and
every plugin list is signed with the store's key. Kaeri ships no store and lists none. You
join one through an invite link. See [Legal and contact](https://kaeri.app/legal/).

<p align="center">
<img src="docs/assets/readme/windows-stores.webp" width="724" alt="The Windows app's plugin stores: three stores you have joined, and one store's plugins with the hosts each one contacts">
</p>

See [plugin stores](https://kaeri.app/docs/plugins/stores/),
[running a store](https://kaeri.app/docs/plugins/running-a-store/) and
[writing a plugin](https://kaeri.app/docs/plugins/authoring/).

## Build from source

The Rust toolchain version is pinned in `rust-toolchain.toml`, and `rustup` installs it. For the
terminal app, install mpv and ffmpeg, and cmake for the torrent engine's crypto library, then:

```sh
cargo run --release                      # the terminal app, from a checkout
cargo test --workspace                   # no network
cargo clippy --workspace --all-targets
```

- Mac, iPhone and iPad: [apple/README.md](apple/README.md). It needs Xcode 26, XcodeGen and an
  Apple silicon Mac.
- Android and Android TV: [android/README.md](android/README.md). It needs Android Studio, the NDK,
  `cargo-ndk` and the `aarch64-linux-android` target.
- Windows: `desktop/windows`, a .NET solution. See
  [docs/desktop-windows-bindings.md](docs/desktop-windows-bindings.md).
- Linux: `desktop/linux`, a Cargo project with GTK 4 and libadwaita. See
  [docs/desktop-linux-app.md](docs/desktop-linux-app.md).
- Plugins: `rustup target add wasm32-wasip2`, then see [docs/plugins.md](docs/plugins.md).
- The website: `cd website && bun run dev`.
- Claude Code setup and efficient workflows: [docs/claude-code.md](docs/claude-code.md).
- Accessibility: what each app supports and how to report a barrier is in
  [ACCESSIBILITY.md](ACCESSIBILITY.md).

<details>
<summary>Project layout</summary>

<br>

| Folder | What it holds |
| --- | --- |
| `crates/` | The Rust workspace: the engine, the terminal app, the trackers, the plugin sandbox, the plugin store and the FFI layer the apps use |
| `apple/` | The SwiftUI app for Mac, iPhone and iPad |
| `android/` | The Kotlin and Compose apps for Android and Android TV |
| `desktop/` | The WinUI 3 app for Windows and the GTK 4 app for Linux |
| `plugins/` | Example plugins in Rust and JavaScript |
| `wit/` | The interface plugins are written against |
| `deploy/` | Ways to run a plugin store: containers and serverless hosts |
| `packaging/` | Release builds, installers and the release scripts |
| `website/` | The website and docs, in Astro and Starlight |
| `docs/` | Notes on plugins, plugin stores and the apps, and the images in this README |
| `assets/` | Icons, shaders and the images used in the docs |
| `tools/` | Checks and helper scripts for the workspace |

</details>

## Licence

Kaeri is open source under the [MIT licence](LICENSE). Notices for what it bundles are in
[assets/THIRD-PARTY.md](assets/THIRD-PARTY.md) and ship with each release. Security reports go
through [SECURITY.md](SECURITY.md).

<br>

<p align="center">
<a href="https://kaeri.app"><img src="website/public/favicon.svg" width="48" alt="kaeri"></a>
<br>
<sub><a href="https://kaeri.app/legal/">Legal and contact</a></sub>
</p>
