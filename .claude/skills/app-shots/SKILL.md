---
name: app-shots
description: Take screenshots of the Kaeri apps and run UI checks safely - Apple and Android shots scripts, scenes and fixture ids, emulator and Simulator rules, and click tests. Use when capturing screens, comparing with design boards or measuring frames.
---

# App screenshots and UI checks

## Apple

```bash
caffeinate -u apple/scripts/shots.sh <mac|iphone|ipad> [--landscape] [shot…]
```

- A shot is a scene and optionally a screen: `watching` (default), `fresh`, `sourceless`, then
  `:calendar`, `:seasonal`, `:search`, `:library`, `:downloads`, `:providers`, `:pluginStores`,
  `:settings`, or `title=<id>`, `query=<words>`, joined with `+` (`pane=`, `sheet=`,
  `showcase=`, `watched=<id>:<episodes>`). Player states: `player-playing`, `player-skip`,
  `player-up-next`, `player-picture-in-picture` and so on. Read the script's header for all.
- Output goes to `apple/.shots/` (git ignores it). Mac shots come out white while the display
  sleeps, hence `caffeinate -u`. `--landscape` and a clean 9:41 status bar are built in for iPad.
- `KAERI_SIMULATOR` picks the Simulator. Create your own and delete it after; never shut
  down others' (no `simctl shutdown all`).

## Android

```bash
android/scripts/shots.sh <phone|phone-37|tablet|fold> [shot…]
```

Same shot syntax: `watching`, `fresh`, or `app` (real AniList), then `:calendar`, `:seasonal`,
`title=<id>` and so on. Roborazzi JVM shots (`:ui` tests) cover most boards without an emulator.

## Windows

The Fixtures build (`-p:EngineFlavour=testing`, the `windows` kind on slot) takes the same scenes
as launch arguments: `Kaeri.exe --scene=watching --page=calendar`, `--show=<state>` and the rest
in `desktop/windows/Kaeri.Windows/Options.cs`. Shots run in the PC's test VM (`vm.ps1 shot`
under `slot with gui --wsl`, see the slot README) or with `desktop/windows/scripts/spike-shot.ps1`
on a signed-in desktop; both capture the window at set seconds after launch, and spike-shot.ps1
exits 1 when a mark was lost.

- Time-dependent screens are still shots: `--still=<seconds>` holds the engine's still clock
  there. With `--show=starting-<state>` (or the older `--show=starting-<state>-<seconds>`) the
  made-up start is stepped to that moment at once and its seconds read the still clock, so one
  mark about 5 s after the window shows exactly that moment; the app logs "settled" 1.5 s after
  the hold. With any other scene `--still` holds the scene's made-up waits, as `KAERI_STILL_AT`
  does. Without it everything runs on the wall clock, which recordings need.

## Fixture ids

Made-up titles only: 900001–900006 (the base fixtures), 920001–920014 (a 12-season franchise
with a film and an OVA). Never real shows.

## Rules

- Only the Fixtures app or a re-identified copy runs. Never launch `app.kaeri.app`; never post
  real notifications on the Mac.
- One emulator at a time, your own AVD name and port, selected by `-avd`/`-port` in any script
  (including `taskpolicy -b` throttling for phone-speed timing). Stop it when done.
- Anything that moves the pointer on the maintainer's Mac needs their go-ahead, must act only
  when our window is in front, and must restore their windows. The Mac player XCUITest runs in
  CI only.
- Measure frames on release-like builds (R8, not debuggable); debug builds mislead.
