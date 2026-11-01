# The Apple apps

- Mac, iPhone and iPad from one SwiftUI codebase, OS 26 minimum, Xcode 27.1 (Swift 6.4).
  No paid Apple account: Mac builds are ad-hoc signed (`scripts/adhoc-sign.sh`), the iPhone and
  iPad `.ipa` is sideloaded with SideStore or AltStore.
- `Packages/`: KaeriKit (engine bindings), KaeriStore (the app's state on the engine),
  KaeriDesign (tokens, fonts, pointer), KaeriUI (screens), KaeriPlayer and
  KaeriPlayerUI (embedded libmpv), KaeriWidgets, KaeriDev (the dev harness).
  `App/` is the app target, including `MacUpdater.swift` (Sparkle 2). The Xcode project comes
  from `project.yml` through `xcodegen generate`.
- Scripts: `engine.sh <testing|release>` builds the engine slices (cached in
  `KAERI_ENGINE_CACHE`), `mpv.sh` builds libmpv, `test.sh` runs the package tests,
  `shots.sh` takes screenshots (see the `app-shots` skill), `adhoc-sign.sh` signs a Release Mac
  app from the inside out and verifies it.

## Rules

- `swift build -c release` catches concurrency errors that Debug misses; always build Release
  for touched packages.
- Use the Fixtures app (made-up data) for anything that runs. Never launch `app.kaeri.app`,
  and never post real notifications on the Mac.
- The Mac player UI test (`MacPlayerTests/`, scheme "Kaeri Mac Player") needs Automation
  Mode. It runs in CI only; don't run it on the maintainer's Mac.
- Mac screenshots come out white while the display sleeps: run under `caffeinate -u`.
- Shut down only the Simulators you create.
