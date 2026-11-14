// The refresh primitives, in each vendor family's own calls, reached by reflection: a full refresh
// of a view, and on Onyx the update mode a view redraws in. Every call is guarded, so a missing
// method, a changed signature or a vendor exception can never crash a phone: the controller then
// does nothing and the system's own e-ink handling does the rest. Kaeri never draws a black flash
// of its own, and out of the box asks for no full refresh: the engine's policy only asks when the
// person turned a Refresh row on, or held page down for Clean screen.
//
// The calls, from the S1 device probe:
//   Onyx      View.refreshScreen(0, 0, w, h, 34), which flashes on all three Boox devices tried
//   NTX       View.postInvalidateDelayed(0L, 0, 0, w, h, 34)
//   Rockchip  View.requestEpdMode(View.EINK_MODE.EPD_FULL, true), or the "eink" system service's
//             sendOneFullFrame()
// The Onyx mode calls, from the mode probe on a Note Air 4C (Android 13): View.setDefaultUpdateMode
// (int), getDefaultUpdateMode() and resetUpdateMode() resolve for an app that targets the current
// SDK, return without error and read back what was set. The framework hands a view's mode with
// each of its invalidations to the window's EPD update list, the way Onyx's own SDK sets Regal.
// The app-wide android.onyx.ViewUpdateHelper calls are hidden from apps, and are what the E-ink
// Center uses for its per-app mode, so Kaeri never calls them. setWaveformAndScheme is never
// called.

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

    /** The family's own update modes can be set for a view: the Screen mode row shows then. */
    val modes: Boolean get() = false

    /**
     * Redraw [view] in Screen mode's [mode]: "device" gives it back to the device's own setting.
     * Does nothing where [modes] is false.
     */
    fun setMode(view: View, mode: String) = Unit
}

/** A refresh the vendor call is needed for. */
val Refresh.full: Boolean
    get() = this == Refresh.FULL

/** The Onyx 5-integer refreshScreen, and the update mode a view redraws in. */
class OnyxRefresh internal constructor(
    private val method: Method?,
    private val setDefault: Method? = null,
    private val reset: Method? = null,
) : RefreshController {
    override val available: Boolean get() = method != null

    override val modes: Boolean get() = setDefault != null && reset != null

    override fun refresh(view: View, refresh: Refresh) {
        if (!refresh.full) return
        guarded { method?.invoke(view, 0, 0, view.width, view.height, ONYX_FULL) }
    }

    override fun setMode(view: View, mode: String) {
        if (!modes) return
        val value = MODES[mode]
        if (value == null) guarded { reset?.invoke(view) } else guarded { setDefault?.invoke(view, value) }
    }

    companion object {
        /** The update mode the probe found to flash. */
        const val ONYX_FULL = 34

        /**
         * Screen mode's choices as Onyx update modes, the values the E-ink Center gives its own
         * modes: HD is the 16-grey partial update, Regal the reduced-ghosting one, Balanced and
         * Fast the dithered DU and A2 modes. "device" is not here: it resets the view.
         */
        val MODES: Map<String, Int> = mapOf("hd" to 2, "regal" to 6, "balanced" to 2305, "fast" to 2308)

        fun resolve(): OnyxRefresh = OnyxRefresh(
            guarded { View::class.java.getMethod("refreshScreen", Int::class.java, Int::class.java, Int::class.java, Int::class.java, Int::class.java) },
            guarded { View::class.java.getMethod("setDefaultUpdateMode", Int::class.java) }
                ?.takeIf { guarded { View::class.java.getMethod("getDefaultUpdateMode") } != null },
            guarded { View::class.java.getMethod("resetUpdateMode") },
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
class FakeRefresh(override val available: Boolean = true, override val modes: Boolean = false) : RefreshController {
    data class Request(val refresh: Refresh, val width: Int, val height: Int)

    private val log = mutableListOf<Request>()
    private val modeLog = mutableListOf<String>()

    /** Every request so far, full or not, in order. */
    val requests: List<Request> get() = synchronized(log) { log.toList() }

    /** Every Screen mode set so far, in order. */
    val modesSet: List<String> get() = synchronized(log) { modeLog.toList() }

    override fun refresh(view: View, refresh: Refresh) {
        synchronized(log) { log += Request(refresh, view.width, view.height) }
    }

    override fun setMode(view: View, mode: String) {
        if (modes) synchronized(log) { modeLog += mode }
    }

    fun clear() = synchronized(log) {
        log.clear()
        modeLog.clear()
    }
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
        return if (controller.available || controller.modes) controller else NoRefresh
    }

    /** The hooks this device has, by the names the engine's detection reads. */
    fun hooks(context: Context): List<String> = buildList {
        val onyx = OnyxRefresh.resolve()
        if (onyx.available) add("onyx")
        if (onyx.modes) add("onyx-modes")
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
