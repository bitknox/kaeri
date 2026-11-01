// The app flavour has no testing engine: its engine is built without the `testing` feature.

package app.kaeri.store

import app.kaeri.engine.ffi.FfiDirs
import app.kaeri.engine.ffi.FfiEngine
import app.kaeri.engine.ffi.FfiException
import app.kaeri.engine.ffi.FfiHost
import app.kaeri.engine.ffi.FfiStart
import app.kaeri.engine.model.Msg

internal fun startFixtures(scene: FixtureScene, dirs: FfiDirs, host: FfiHost, start: FfiStart): FfiEngine =
    throw FfiException.Engine(reason = "this build has no testing engine", msg = Msg.NoticeEngineFault)

/** Whether this build has the testing engine in it. */
const val HAS_TESTING_ENGINE = false
