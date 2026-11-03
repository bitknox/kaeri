// The chat column's motion, shared by the phone's player and the TV's: lines arrive on a fade with
// a small rise, hold, fade out and the column closes up; past three, the rest fold into one "n more"
// line, and when the oldest leaves the folded line takes the fold's place in a crossfade.

package app.kaeri.design

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay

/** One line standing in a chat column, held for [holdMillis] once it is in view. */
class ColumnLine<T>(val item: T, val holdMillis: Long) {
    internal val state = MutableTransitionState(false)
    internal var leaving by mutableStateOf(false)
}

/** The fade-out, then the column closing up under it. */
private const val CLOSE_DELAY_MS = 300
private const val CLOSE_MS = 450

/**
 * The lines of a chat column in arrival order, [keep] in view and the rest folded into [fold]'s
 * count. A line's hold starts when it comes into view; then it fades out, the lines under it move
 * up, and it leaves [lines]. Each line stands [gap] above the next, and the gap closes with it.
 */
@Composable
fun <T> FoldingColumn(
    lines: SnapshotStateList<ColumnLine<T>>,
    keep: Int = 3,
    gap: Dp = 6.dp,
    line: @Composable (T) -> Unit,
    fold: @Composable (Int) -> Unit,
) {
    val rise = with(LocalDensity.current) { 6.dp.roundToPx() }
    val staying = lines.filterNot { it.leaving }
    for (entry in lines.toList()) {
        val rank = staying.indexOf(entry)
        if (!entry.leaving && rank > keep) continue
        key(entry) {
            val inView = entry.leaving || rank < keep
            // The fold keeps its last count while it fades into the line it held.
            var folded by remember { mutableIntStateOf(1) }
            if (staying.size > keep) folded = staying.size - keep
            remember(entry) { entry.state.apply { if (!entry.leaving) targetState = true } }
            LaunchedEffect(inView) {
                if (!inView || entry.leaving) return@LaunchedEffect
                delay(entry.holdMillis)
                entry.leaving = true
                entry.state.targetState = false
                delay((CLOSE_DELAY_MS + CLOSE_MS).toLong())
                lines.remove(entry)
            }
            AnimatedVisibility(
                visibleState = entry.state,
                enter = fadeIn(Motion.fadeIn()) + slideInVertically(Motion.standard()) { rise },
                exit = fadeOut(Motion.fadeOut()) +
                    shrinkVertically(tween(CLOSE_MS, delayMillis = CLOSE_DELAY_MS, easing = FastOutSlowInEasing), shrinkTowards = Alignment.Top),
            ) {
                Box(Modifier.padding(bottom = gap)) {
                    Crossfade(inView, animationSpec = Motion.fadeIn(), label = "fold") { shown ->
                        if (shown) line(entry.item) else fold(folded)
                    }
                }
            }
        }
    }
}
