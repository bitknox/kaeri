// Surprise me and Another: which pool a pick is asked of, whether one is on its way, and the
// title the engine picked. The engine words every notice itself; this only opens the page.

package app.kaeri.store

import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.SurpriseOutcome
import app.kaeri.engine.model.SurprisePool
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow

/** A title the engine picked, and whether the page opens in place of the one on screen. */
data class SurprisePick(val id: AnilistId, val pool: SurprisePool, val more: Boolean, val inPlace: Boolean)

class SurpriseModel {
    private val _asking = MutableStateFlow<SurprisePool?>(null)

    /** The pool a pick is on its way for; the button shows a spinner past 600 ms of it. */
    val asking: StateFlow<SurprisePool?> = _asking.asStateFlow()

    private val _visit = MutableStateFlow<SurprisePick?>(null)

    /** The pick the title page on screen was opened by, for its Another; `null` on any other page. */
    val visit: StateFlow<SurprisePick?> = _visit.asStateFlow()

    private val _picks = MutableSharedFlow<SurprisePick>(extraBufferCapacity = 4)

    /** Each pick, for the screen that opens it. */
    val picks: SharedFlow<SurprisePick> = _picks.asSharedFlow()

    private var inPlace = false

    /** A pick is asked for; false when one is on its way already. */
    internal fun ask(pool: SurprisePool, inPlace: Boolean): Boolean {
        if (_asking.value != null) return false
        this.inPlace = inPlace
        _asking.value = pool
        return true
    }

    internal fun answer(pool: SurprisePool, outcome: SurpriseOutcome) {
        _asking.value = null
        if (outcome is SurpriseOutcome.Picked) {
            val pick = SurprisePick(outcome.id, pool, outcome.more, inPlace)
            // A book opens as a series, with no Another on its page.
            _visit.value = pick.takeIf { pool !is SurprisePool.Reading }
            _picks.tryEmit(pick)
        }
    }

    /** The page for [id] closed; its Another goes with it. */
    fun left(id: AnilistId) {
        if (_visit.value?.id == id) _visit.value = null
    }
}
