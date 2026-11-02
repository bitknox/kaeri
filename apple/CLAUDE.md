# The Apple apps

- Mac, iPhone and iPad from one SwiftUI codebase, OS 26 minimum, Xcode 27.1 (Swift 6.4).
  No paid Apple account: the iPhone and iPad `.ipa` is signed ad hoc and sideloaded with
  SideStore or AltStore. Mac releases are signed with a self-signed certificate (CN "Kaeri")
  when the `MAC_CODESIGN_P12` secrets reach the run, and ad hoc otherwise. Local review builds
  are signed with it too, through `signing-keychain.sh sign` (`scripts/adhoc-sign.sh`,
  `scripts/signing-keychain.sh`). The certificate keeps the app's designated requirement the
  same across builds (`identifier "app.kaeri.app" and certificate leaf`), so the login keychain's "Always Allow" lasts across updates. A cdhash, which is what
  an ad hoc signature gives, changes every build. The terminal binary is signed with it too.
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

- The signing identity lives in `~/.kaeri-signing/mac` on the maintainer's Mac (the `.p12`,
  its password, the certificate). Never print, copy or commit them, and never import them into
  a lasting keychain on the Mac: sign locally only through `signing-keychain.sh sign`, which
  makes a throwaway keychain outside the search list and deletes it. Nothing may show a keychain
  dialog.
- Every secret the Mac app keeps is in one login keychain item (service `kaeri`, account
  `kaeri-secrets`), read once per launch (`crates/kaeri-track/src/secret.rs`). Don't add
  another item or a later read: each one is a prompt after an update.

- `swift build -c release` catches concurrency errors that Debug misses; always build Release
  for touched packages.
- Use the Fixtures app (made-up data) for anything that runs. Never launch `app.kaeri.app`,
  and never post real notifications on the Mac.
- The Mac player UI test (`MacPlayerTests/`, scheme "Kaeri Mac Player") needs Automation
  Mode. It runs in CI only; don't run it on the maintainer's Mac.
- Mac screenshots come out white while the display sleeps: run under `caffeinate -u`.
- Shut down only the Simulators you create.
