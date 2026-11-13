// Vendor calls are reached by reflection and guarded: on a phone none resolves, and a call that
// throws, or a method that is missing, never reaches the app.

package app.kaeri.eink

import android.view.View
import app.kaeri.engine.model.Refresh
import app.kaeri.engine.model.VendorFamily
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/** Stand-ins for a vendor's refreshScreen: one that records, one that throws. */
object Vendor {
    val calls = mutableListOf<List<Int>>()

    @JvmStatic
    fun refreshScreen(left: Int, top: Int, right: Int, bottom: Int, mode: Int) {
        calls += listOf(left, top, right, bottom, mode)
    }

    @JvmStatic
    fun broken(left: Int, top: Int, right: Int, bottom: Int, mode: Int): Unit = throw IllegalStateException("vendor")
}

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class RefreshControllerTest {
    private val context = RuntimeEnvironment.getApplication()
    private val view = View(context)
    private val ints = Array(5) { Int::class.java }

    @Test
    fun a_phone_has_no_hook_and_gets_no_refresh() {
        assertFalse(OnyxRefresh.resolve().available)
        assertFalse(NtxRefresh.resolve().available)
        assertFalse(RockchipRefresh.resolve(context).available)
        assertTrue(RefreshControllers.hooks(context).isEmpty())
        VendorFamily.entries.forEach { assertSame(NoRefresh, RefreshControllers.of(it, context)) }
        // Asking anyway does nothing at all.
        RefreshControllers.of(VendorFamily.ONYX, context).refresh(view, Refresh.FULL)
    }

    @Test
    fun only_a_full_refresh_calls_the_vendor_with_the_whole_view() {
        Vendor.calls.clear()
        val onyx = OnyxRefresh(Vendor::class.java.getMethod("refreshScreen", *ints))
        view.layout(0, 0, 600, 800)
        onyx.refresh(view, Refresh.PARTIAL)
        onyx.refresh(view, Refresh.FAST)
        onyx.refresh(view, Refresh.FULL)
        assertEquals(listOf(listOf(0, 0, 600, 800, 34)), Vendor.calls)
    }

    @Test
    fun a_vendor_that_throws_never_crashes() {
        val onyx = OnyxRefresh(Vendor::class.java.getMethod("broken", *ints))
        assertTrue(onyx.available)
        onyx.refresh(view, Refresh.FULL)
        val wrong = OnyxRefresh(Vendor::class.java.getMethod("refreshScreen", *ints))
        // A view of no size still asks without a crash.
        wrong.refresh(View(context), Refresh.FULL)
    }

    @Test
    fun a_blocked_ntx_call_leaves_the_onyx_refresh_working() {
        // An Onyx framework denies NTX's hidden postInvalidateDelayed when hooks() probes it: that
        // family's controller is then simply absent, and Onyx's own refreshScreen still runs.
        val blocked = NtxRefresh(null)
        assertFalse(blocked.available)
        blocked.refresh(view, Refresh.FULL)
        Vendor.calls.clear()
        val onyx = OnyxRefresh(Vendor::class.java.getMethod("refreshScreen", *ints))
        view.layout(0, 0, 600, 800)
        onyx.refresh(view, Refresh.FULL)
        assertEquals(listOf(listOf(0, 0, 600, 800, 34)), Vendor.calls)
    }

    @Test
    fun the_fake_logs_every_request() {
        val fake = FakeRefresh()
        view.layout(0, 0, 300, 400)
        fake.refresh(view, Refresh.FAST)
        fake.refresh(view, Refresh.FULL)
        assertEquals(listOf(FakeRefresh.Request(Refresh.FAST, 300, 400), FakeRefresh.Request(Refresh.FULL, 300, 400)), fake.requests)
    }
}
