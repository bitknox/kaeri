// What a title page's banner trailer does during one visit: wait, load, play, give way to the
// banner when the page scrolls away or the trailer is about to end, and let go.

package app.kaeri.store

import app.kaeri.engine.model.TrailerControl
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/**
 * One visit to a title page. The page calls [begin] once it has settled and the engine allows a
 * trailer; from then on the web view exists while [live] and the video is in front of the banner
 * art while [shown]. The art always comes back before anything is paused or released, so the
 * player's own paused and end screens never show.
 *
 * - The banner stays, silently, if nothing plays within [loadTimeout] or the player says an error.
 * - Scrolling away brings the art back, pauses the video once the art covers it ([fade] later),
 *   and releases the view [awayLimit] after the page went away; scrolling back before that plays
 *   it again and takes the art away once a picture shows.
 * - It plays once: the page says it is ending a second before the end, the art starts back
 *   [endLead] later, half a second before the end, and the view goes once the art is back.
 *
 * @param send tells the page to play or pause.
 */
class TrailerVisit(
    private val scope: CoroutineScope,
    private val send: (TrailerControl) -> Unit,
    private val loadTimeout: Long = LOAD_TIMEOUT,
    private val awayLimit: Long = AWAY_LIMIT,
    private val fade: Long = FADE_OUT,
    private val endLead: Long = END_LEAD,
) {
    private enum class Stage { Waiting, Loading, Playing, Held, Done }

    private var stage = Stage.Waiting
    private var away = false
    private var timer: Job? = null
    private var pause: Job? = null
    private var awayTimer: Job? = null

    private val _live = MutableStateFlow(false)

    /** Whether the web view exists. */
    val live: StateFlow<Boolean> = _live.asStateFlow()

    private val _shown = MutableStateFlow(false)

    /** Whether the video is in front of the banner art. */
    val shown: StateFlow<Boolean> = _shown.asStateFlow()

    /** Start loading: the page has settled and a trailer may play. Once per visit. */
    fun begin() {
        if (stage != Stage.Waiting) return
        stage = Stage.Loading
        _live.value = true
        waitForPicture()
    }

    private fun waitForPicture() {
        timer?.cancel()
        timer = scope.launch {
            delay(loadTimeout)
            if (stage == Stage.Loading) finish()
        }
    }

    /** The page scrolled past the point where it would slide over the video, or back to the top. */
    fun scrolledAway(value: Boolean) {
        if (away == value) return
        away = value
        if (stage == Stage.Waiting || stage == Stage.Done) return
        if (value) {
            awayTimer?.cancel()
            awayTimer = scope.launch {
                delay(awayLimit)
                if (away && stage != Stage.Done) finish()
            }
            if (stage == Stage.Playing) coverThenPause()
            return
        }
        awayTimer?.cancel()
        when {
            // Back before the pause landed: the video still plays, so it simply comes forward.
            stage == Stage.Playing && pause?.isActive == true -> {
                pause?.cancel()
                _shown.value = true
            }
            stage == Stage.Held -> {
                // Play, then the art goes once the page says a picture shows again.
                stage = Stage.Loading
                send(TrailerControl.PLAY)
                waitForPicture()
            }
        }
    }

    private fun coverThenPause() {
        _shown.value = false
        pause?.cancel()
        pause = scope.launch {
            delay(fade)
            send(TrailerControl.PAUSE)
            stage = Stage.Held
        }
    }

    /** What the page said. */
    fun signal(signal: TrailerSignal) {
        when (signal) {
            TrailerSignal.Playing -> if (stage == Stage.Loading) {
                timer?.cancel()
                stage = Stage.Playing
                // Scrolled away while it loaded: it stays under the art and waits.
                if (away) coverThenPause() else _shown.value = true
            }
            TrailerSignal.Ending -> if (stage == Stage.Playing || stage == Stage.Loading) {
                stage = Stage.Done
                timer?.cancel()
                timer = scope.launch {
                    delay(endLead)
                    endUnderArt()
                }
            }
            // The end came with no warning, or before the lead ran out: the art back now.
            TrailerSignal.Ended -> if (_live.value) {
                stage = Stage.Done
                if (_shown.value) {
                    timer?.cancel()
                    endUnderArt()
                } else if (timer?.isActive != true) {
                    finish()
                }
            }
            is TrailerSignal.Failed -> if (stage != Stage.Done) finish()
            TrailerSignal.Quiet -> {}
        }
    }

    private fun endUnderArt() {
        pause?.cancel()
        awayTimer?.cancel()
        _shown.value = false
        timer = scope.launch {
            delay(fade)
            finish()
        }
    }

    /**
     * A sheet, the background, another page or a back gesture: once the trailer has started, the
     * view goes at once and the trailer does not come back on this visit.
     */
    fun leave() {
        if (stage != Stage.Waiting) finish()
    }

    /** The page is gone: let go of everything. */
    fun release() {
        finish()
    }

    private fun finish() {
        timer?.cancel()
        pause?.cancel()
        awayTimer?.cancel()
        stage = Stage.Done
        _shown.value = false
        _live.value = false
    }

    companion object {
        /** Nothing playing this long after the view was made: the banner stays. */
        const val LOAD_TIMEOUT = 8_000L

        /** Scrolled away this long: the view goes for the rest of the visit. */
        const val AWAY_LIMIT = 30_000L

        /** How long the art takes to come back over the video (fade-out). */
        const val FADE_OUT = 300L

        /** From the page's "ending", a second before the end, to the art starting back: half a second before it. */
        const val END_LEAD = 500L
    }
}
