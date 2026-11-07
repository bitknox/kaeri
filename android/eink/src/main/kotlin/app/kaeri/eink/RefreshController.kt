// The one refresh primitive: a full refresh of a view, in each vendor family's own call, reached
// by reflection. Every call is guarded, so a missing method, a changed signature or a vendor
// exception can never crash a phone: the controller then does nothing and the system's own e-ink
// handling does the rest. Kaeri never draws a black flash of its own.
//
// The calls, from the S1 device probe:
//   Onyx      View.refreshScreen(0, 0, w, h, 34), which flashes on all three Boox devices tried
//   NTX       View.postInvalidateDelayed(0L, 0, 0, w, h, 34)
//   Rockchip  View.requestEpdMode(View.EINK_MODE.EPD_FULL, true), or the "eink" system service's
//             sendOneFullFrame()
// setWaveformAndScheme is never called.

package app.kaeri.eink

import android.content.Context
import android.view.View
import app.kaeri.engine.model.Refresh
import app.kaeri.engine.model.VendorFamily
import java.lang.reflect.Method

/** Asks the panel to refresh a view. */
interface RefreshController {
    /** A hook resolved on this device: the Refresh rows and Clean screen show only then. */
    val available: Boolean

    /**
     * Carry out the engine's [refresh] for [view]. Only a full refresh needs the vendor; the
     * partial, fast and region modes are what the panel does by itself when a view redraws.
     */
    fun refresh(view: View, refresh: Refresh)
}

/** A refresh the vendor call is needed for. */
val Refresh.full: Boolean
    get() = this == Refresh.FULL

/** The Onyx 5-integer refreshScreen. */
class OnyxRefresh internal constructor(private val method: Method?) : RefreshController {
    override val available: Boolean get() = method != null

    override fun refresh(view: View, refresh: Refresh) {
        if (!refresh.full) return
        guarded { method?.invoke(view, 0, 0, view.width, view.height, ONYX_FULL) }
    }

    companion object {
        /** The update mode the probe found to flash. */
        const val ONYX_FULL = 34

        fun resolve(): OnyxRefresh = OnyxRefresh(
            guarded { View::class.java.getMethod("refreshScreen", Int::class.java, Int::class.java, Int::class.java, Int::class.java, Int::class.java) },
        )
    }
}

/** The NTX postInvalidateDelayed with a rectangle and mode. */
class NtxRefresh internal constructor(private val method: Method?) : RefreshController {
    override val available: Boolean get() = method != null

    override fun refresh(view: View, refresh: Refresh) {
        if (!refresh.full) return
        guarded { method?.invoke(view, 0L, 0, 0, view.width, view.height, OnyxRefresh.ONYX_FULL) }
    }

    companion object {
        fun resolve(): NtxRefresh = NtxRefresh(
            guarded {
                View::class.java.getMethod(
                    "postInvalidateDelayed",
                    Long::class.java, Int::class.java, Int::class.java, Int::class.java, Int::class.java, Int::class.java,
                )
            },
        )
    }
}

/** Rockchip's EPD mode on the view, or its "eink" system service. */
class RockchipRefresh internal constructor(
    private val request: Method?,
    private val fullMode: Any?,
    private val service: Any?,
    private val oneFrame: Method?,
) : RefreshController {
    override val available: Boolean get() = (request != null && fullMode != null) || (service != null && oneFrame != null)

    override fun refresh(view: View, refresh: Refresh) {
        if (!refresh.full) return
        if (request != null && fullMode != null) {
            guarded { request.invoke(view, fullMode, true) }
        } else if (service != null && oneFrame != null) {
            guarded { oneFrame.invoke(service) }
        }
    }

    companion object {
        fun resolve(context: Context): RockchipRefresh {
            val mode = guarded { Class.forName("android.view.View\$EINK_MODE") }
            val full = mode?.let { m -> guarded { m.enumConstants?.firstOrNull { (it as? Enum<*>)?.name == "EPD_FULL" } } }
            val request = mode?.let { m -> guarded { View::class.java.getMethod("requestEpdMode", m, Boolean::class.java) } }
            val service = guarded { context.getSystemService("eink") }
            val oneFrame = service?.let { s -> guarded { s.javaClass.getMethod("sendOneFullFrame") } }
            return RockchipRefresh(request, full, service, oneFrame)
        }
    }
}

/** No hook: the layout still never animates, and the panel refreshes as it does by itself. */
object NoRefresh : RefreshController {
    override val available: Boolean = false

    override fun refresh(view: View, refresh: Refresh) = Unit
}

/** Logs every request, for the Fixtures app and the tests, and calls nothing. */
class FakeRefresh(override val available: Boolean = true) : RefreshController {
    data class Request(val refresh: Refresh, val width: Int, val height: Int)

    private val log = mutableListOf<Request>()

    /** Every request so far, full or not, in order. */
    val requests: List<Request> get() = synchronized(log) { log.toList() }

    override fun refresh(view: View, refresh: Refresh) {
        synchronized(log) { log += Request(refresh, view.width, view.height) }
    }

    fun clear() = synchronized(log) { log.clear() }
}

object RefreshControllers {
    /**
     * The controller for a vendor family. Each is resolved only for its own family, so a call
     * meant for one device is never made on another that happens to have the method.
     */
    fun of(family: VendorFamily, context: Context): RefreshController {
        val controller = when (family) {
            VendorFamily.ONYX -> OnyxRefresh.resolve()
            VendorFamily.NTX -> NtxRefresh.resolve()
            VendorFamily.ROCKCHIP -> RockchipRefresh.resolve(context)
            VendorFamily.OTHER -> NoRefresh
        }
        return if (controller.available) controller else NoRefresh
    }

    /** The hooks this device has, by the names the engine's detection reads. */
    fun hooks(context: Context): List<String> = buildList {
        if (OnyxRefresh.resolve().available) add("onyx")
        if (NtxRefresh.resolve().available) add("ntx")
        if (RockchipRefresh.resolve(context).available) add("rockchip")
    }
}

/** Runs a vendor call, and gives null for anything it throws, linkage errors included. */
internal inline fun <T> guarded(call: () -> T): T? = try {
    call()
} catch (_: Exception) {
    null
} catch (_: LinkageError) {
    null
}
