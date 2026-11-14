#!/usr/bin/env bash
# Records the testing engine's scenes for the screenshot tests: the events the engine sends for
# its fresh and watching scenes while the store asks for every screen, written as JSON to
# android/testkit/src/main/resources/app/kaeri/testkit/scenes/. Run it again when the engine's
# events or its fixtures change; the screenshot tests replay what it wrote, with no engine.
#
#   android/scripts/scenes.sh
#   KAERI_FIXTURES_HOME_WORLD=1 android/scripts/scenes.sh
#
# The second run records Home's scenes (home-watching, home-suggested, home-oneleft and so on)
# with the invented Home world on; the first records fresh and watching without it.
#
# It runs the store's SceneRecorder on this Mac's testing engine, under test.sh's home of its own.
# The engine's clock is stopped at one moment for the recording (KAERI_SCENES_AT, set by the
# store's build), and the recorder asks one question at a time, each answered before the next,
# so two runs write the same files. To compare runs without touching the test kit, record into a
# directory of your own:
#
#   android/scripts/test.sh -Pkaeri.scenes=/some/dir :store:testFixturesDebugUnitTest --tests app.kaeri.store.SceneRecorder
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

android="$(cd "$(dirname "$0")/.." && pwd)"
"$android/scripts/test.sh" -Pkaeri.scenes :store:testFixturesDebugUnitTest --tests app.kaeri.store.SceneRecorder
ls -l "$android/testkit/src/main/resources/app/kaeri/testkit/scenes/"
