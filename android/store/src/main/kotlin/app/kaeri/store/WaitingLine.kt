// The waiting rule every prelaunch answer follows: nothing for 600 ms, so a quick answer is the
// first thing seen; a waiting line that did show stays at least 300 ms, so it never flickers.

package app.kaeri.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

object Waiting {
    const val WAIT_MILLIS = 600L
    const val HOLD_MILLIS = 300L

    /**
     * For a question asked at [start] and answered at [answer] (milliseconds): when the waiting line
     * shows, or null when the answer comes first, and the earliest the answer may show.
     */
    fun times(start: Long, answer: Long): Pair<Long?, Long> =
        if (answer - start <= WAIT_MILLIS) null to answer
        else (start + WAIT_MILLIS) to maxOf(answer, start + WAIT_MILLIS + HOLD_MILLIS)
}

/** One waiting line: [shown] turns on 600 ms after [begin], and an answer given through [end] waits for the hold. */
class WaitingLine(private val scope: CoroutineScope, private val onShow: () -> Unit = {}) {
    private val _shown = MutableStateFlow(false)

    /** The waiting line is showing. */
    val shown: StateFlow<Boolean> = _shown.asStateFlow()

    private var gate: Job? = null

    /** A question went out. */
    fun begin() {
        gate?.cancel()
        _shown.value = false
        gate = scope.launch {
            delay(Waiting.WAIT_MILLIS)
            _shown.value = true
            onShow()
            delay(Waiting.HOLD_MILLIS)
        }
    }

    /** The answer came: [deliver] runs at once, or after the waiting line's hold when it showed. */
    fun end(deliver: () -> Unit) {
        val held = gate
        gate = null
        scope.launch {
            when {
                held == null -> {}
                held.isActive && !_shown.value -> held.cancel()
                else -> held.join()
            }
            deliver()
            _shown.value = false
        }
    }

    /** Nothing is asked any more. */
    fun cancel() {
        gate?.cancel()
        gate = null
        _shown.value = false
    }
}
