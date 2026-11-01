// The app flavour has no fake TVs: its engine is built without the `testing` feature.

package app.kaeri.store

import app.kaeri.engine.ffi.FfiEngine
import app.kaeri.engine.ffi.FfiTvBrowser

object FixtureTvs {
    fun start(@Suppress("UNUSED_PARAMETER") engine: FfiEngine): FfiTvBrowser? = null

    fun openLink(@Suppress("UNUSED_PARAMETER") browser: FfiTvBrowser, @Suppress("UNUSED_PARAMETER") index: Int): String? = null

    suspend fun stage(model: TvRemoteModel, scene: TvScene, fakes: FfiTvBrowser, play: TvPlayRequest) {}
}
