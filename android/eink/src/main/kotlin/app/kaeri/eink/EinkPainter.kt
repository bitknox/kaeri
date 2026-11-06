// Each e-ink paint goes through the engine's refresh policy (einkPaint, pure and synchronous, so
// a turn never waits on the engine) and then to the vendor's call. The painter keeps the count of
// partial refreshes between fulls.

package app.kaeri.eink

import android.view.View
import app.kaeri.engine.ffi.einkPaint
import app.kaeri.engine.ffi.einkPolicyDefaults
import app.kaeri.engine.model.EinkCounter
import app.kaeri.engine.model.EinkDecision
import app.kaeri.engine.model.EinkPaint
import app.kaeri.engine.model.EinkPolicy
import app.kaeri.engine.model.Panel
import app.kaeri.engine.model.Refresh

class EinkPainter(
    policy: EinkPolicy,
    val controller: RefreshController,
    private val decide: (EinkPolicy, EinkCounter, EinkPaint) -> EinkDecision = ::einkPaint,
) {
    /** The policy, from the profile, with the dark look's own count when it changes. */
    var policy: EinkPolicy = policy
        set(value) {
            if (value != field) counter = EinkCounter(0u)
            field = value
        }

    var counter: EinkCounter = EinkCounter(0u)
        private set

    /** What [paint] on [view] needs; the vendor is asked for it, and it is returned for the log. */
    fun paint(view: View?, paint: EinkPaint): Refresh {
        val decision = decide(policy, counter, paint)
        counter = decision.counter
        if (view != null) controller.refresh(view, decision.refresh)
        return decision.refresh
    }

    companion object {
        /** A painter for [panel] with the engine's default policy. */
        fun defaults(panel: Panel, controller: RefreshController): EinkPainter = EinkPainter(einkPolicyDefaults(panel), controller)
    }
}
