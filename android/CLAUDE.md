# The Android app

- Kotlin and Jetpack Compose, minSdk 30 for every app and module (`KaeriSdk.MIN`), the phone
  app arm64-v8a only. What newer Androids add is checked at run time with a fallback below. Modules: `:engine` (bindings),
  `:store`, `:design`, `:ui`, `:player` (libmpv through a JNI shim), `:widgets`, `:app`.
  Flavours: `app` (`app.kaeri.app`) and `fixtures` (made-up data, never for real use).
- Scripts: `engine.sh <testing|release>` builds the engine (cached in `KAERI_ENGINE_CACHE`),
  `test.sh` runs unit tests under its own HOME guard, `scenes.sh` re-records the test kit's
  scenes, `shots.sh` takes emulator screenshots, `install.sh` installs on a phone,
  `update-test.sh` rehearses the in-app update with a throwaway key.
- Unit tests and Roborazzi shots run on the JVM. Use `KAERI_PLAYER_NATIVE=off` when the
  native player isn't needed. Re-record shots only after looking at what changed.

## Rules

- After an engine change that alters events or records, re-record the scenes
  (`android/testkit/.../scenes/*.json`) instead of hand-editing them.
- Release builds use R8; check `:app:minifyFixturesReleaseWithR8` when touching JNI or anything
  only native code calls (keep rules in `proguard-rules.pro`).
- The engine library must need only the system's libraries (CI checks `NEEDED`); C++ runtimes
  link statically.
- Emulators: one at a time on this Mac, your own AVD name and port, selected by `-avd`/`-port`
  in scripts. Stop it when done.
- Never print or commit the release keystore or its passwords. `REQUEST_INSTALL_PACKAGES` and
  the update receiver belong to the `app` flavour only.
