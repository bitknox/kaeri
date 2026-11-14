package app.kaeri.store

import org.junit.Assert.assertEquals
import org.junit.Test
import app.kaeri.engine.ffi.FfiDiscoverySink
import app.kaeri.engine.ffi.FfiTvBrowser
import app.kaeri.engine.ffi.FfiTvException
import app.kaeri.engine.ffi.FfiTvProblem
import app.kaeri.engine.model.SyncPairProblem

/**
 * A connection that failed where the device may have moved has the browse resolve what it found
 * again, which below Android 14 is the only way to learn a new address.
 */
class RelookTest {
    private class Browser : FfiTvBrowser, NsdRelook {
        var relooked = 0
        override fun start(sink: FfiDiscoverySink) {}
        override fun stop() {}
        override fun relook() {
            relooked++
        }
    }

    @Test
    fun a_tv_that_could_not_be_reached_or_was_gone_is_resolved_again() {
        val browser = Browser()
        TvRemoteModel.relookAfter(FfiTvException.Failed(FfiTvProblem.UNREACHABLE, "Can't reach the TV."), browser)
        TvRemoteModel.relookAfter(FfiTvException.Failed(FfiTvProblem.GONE, "Gone."), browser)
        assertEquals(2, browser.relooked)
    }

    @Test
    fun a_tv_that_answered_is_not_resolved_again() {
        val browser = Browser()
        for (problem in listOf(FfiTvProblem.WRONG_CODE, FfiTvProblem.BUSY, FfiTvProblem.VERSION, FfiTvProblem.UNPAIRED)) {
            TvRemoteModel.relookAfter(FfiTvException.Failed(problem, "No."), browser)
        }
        assertEquals(0, browser.relooked)
    }

    private class Network : app.kaeri.engine.ffi.FfiSyncNetwork, NsdRelook {
        var relooked = 0
        override fun advertise(advert: app.kaeri.engine.ffi.FfiSyncAdvert) {}
        override fun withdraw(instance: String) {}
        override fun browse(sink: app.kaeri.engine.ffi.FfiSyncSink) {}
        override fun stopBrowsing() {}
        override fun relook() {
            relooked++
        }
    }

    @Test
    fun a_typed_code_no_device_answered_resolves_the_devices_again() {
        val sync = SyncModel()
        val network = Network()
        sync.network = network
        sync.pairFailed(SyncPairProblem.WRONG_CODE, "Desk")
        assertEquals(0, network.relooked)
        sync.pairFailed(SyncPairProblem.NOT_FOUND, null)
        assertEquals(1, network.relooked)
    }
}
