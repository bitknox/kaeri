// The starting screen on a roll-on, under the waiting rule: the ground alone for the first 600 ms,
// so a next episode warmed during the ending goes from the ground straight to its picture. Later
// than that the starting screen fades in and stays at least 0.6 s, so it never flashes.

package app.kaeri.player

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import kotlinx.coroutines.delay

/** The starting screen's waits on a roll-on, in milliseconds. */
object RollOnStart {
    /** The ground alone, from the next episode's staging. */
    const val QUIET = 600L

    /** The shortest the starting screen stays once a roll-on has shown it. */
    const val HOLD = 600L

    /** How long from [now] until the starting screen may show: 0 when it may now. A failure shows at once. */
    fun untilShown(quietUntil: Long?, failed: Boolean, now: Long): Long =
        if (failed || quietUntil == null) 0L else (quietUntil - now).coerceAtLeast(0L)

    /** How long from [now] the starting screen shown at [shownAt] still stays. Only a roll-on's holds. */
    fun stillHeld(shownAt: Long, rollOn: Boolean, now: Long): Long =
        if (!rollOn) 0L else (shownAt + HOLD - now).coerceAtLeast(0L)
}

/**
 * Whether the starting screen's art and column show, with [up] saying a start is under way. Any
 * start but a roll-on shows them as soon as it is up; a roll-on keeps to the ground for
 * [RollOnStart.QUIET] and, once shown, holds them [RollOnStart.HOLD]. The ground itself stays up
 * while either is true.
 */
@Composable
fun rememberStartingShown(model: PlayerModel, up: Boolean): Boolean {
    val quietUntil = model.rollOnQuietUntil
    val failed = model.failure != null
    // When it showed, and whether for a roll-on, which holds it.
    var shown by remember { mutableStateOf<Pair<Long, Boolean>?>(null) }
    // Shown at once where nothing holds it back, so the art fades in with the ground.
    val atOnce = up && shown == null && RollOnStart.untilShown(quietUntil, failed, model.elapsed()) == 0L
    LaunchedEffect(up, quietUntil, failed) {
        if (up) {
            if (shown != null) return@LaunchedEffect
            val wait = RollOnStart.untilShown(quietUntil, failed, model.elapsed())
            if (wait > 0) delay(wait)
            shown = model.elapsed() to (quietUntil != null)
        } else {
            val (at, rollOn) = shown ?: return@LaunchedEffect
            val left = RollOnStart.stillHeld(at, rollOn, model.elapsed())
            if (left > 0) delay(left)
            shown = null
        }
    }
    return atOnce || shown?.let { (_, rollOn) -> up || rollOn } == true
}
