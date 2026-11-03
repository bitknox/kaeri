// What the starting screen is made of on a phone and on the TV: Kaeri's line, the column that stays
// where it is while a start fills it in, the ground over the art and the reduced-motion switch.

package app.kaeri.design

import android.provider.Settings
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.layoutId
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.max

/** The ground over normal art on the starting screen. */
const val StartGroundAlpha = 0.55f

/** The deeper ground over bright art: the haze text holds 4.5:1 on it. */
const val StartGroundBrightAlpha = 0.80f

/** Whether the system asks for less motion: its animator scale is 0, as Remove animations sets it. */
@Composable
fun rememberSystemReducedMotion(): Boolean {
    val context = LocalContext.current
    fun read() = runCatching { Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) }
        .getOrDefault(1f) == 0f
    var reduced by remember { mutableStateOf(read()) }
    DisposableEffect(context) {
        val observer = object : android.database.ContentObserver(android.os.Handler(android.os.Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) {
                reduced = read()
            }
        }
        context.contentResolver.registerContentObserver(Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, observer)
        reduced = read()
        onDispose { context.contentResolver.unregisterContentObserver(observer) }
    }
    return reduced
}

/**
 * Kaeri's line: a silk track at 14 % with a 32 % amber stretch that moves across and back on a
 * 1.4 s ease. With [reduced] the stretch rests at the start and breathes between 100 % and 40 %
 * over 2 s. Given a [percent] (only from mpv's buffering) it becomes a fill that follows each
 * value; the two crossfade over 0.2 s. In a preview or a screenshot, where infinite animations do
 * not run, a still of the stretch partway across.
 */
@Composable
fun KaeriLoaderLine(percent: Int?, reduced: Boolean, modifier: Modifier = Modifier, thickness: Dp = 2.dp) {
    val still = LocalInspectionMode.current
    val stretch = if (still) 0.35f else {
        val motion = rememberInfiniteTransition(label = "loader") // i18n-skip
        if (reduced) 0f else motion.animateFloat(0f, 1f, infiniteRepeatable(tween(1400, easing = FastOutSlowInEasing), RepeatMode.Reverse), label = "stretch").value // i18n-skip
    }
    val breath = if (still || !reduced) 1f else {
        val motion = rememberInfiniteTransition(label = "breath") // i18n-skip
        motion.animateFloat(1f, 0.4f, infiniteRepeatable(tween(1000, easing = LinearEasing), RepeatMode.Reverse), label = "breathing").value // i18n-skip
    }
    val determinate by animateFloatAsState(if (percent != null) 1f else 0f, if (reduced) snap() else Motion.fadeIn(), label = "determinate") // i18n-skip
    val filled by animateFloatAsState((percent ?: 0) / 100f, if (reduced) snap() else Motion.standard(), label = "filled") // i18n-skip
    Box(
        modifier
            .fillMaxWidth()
            .height(thickness)
            .drawBehind {
                val round = CornerRadius(size.height / 2)
                drawRoundRect(ObiSilk.Silk.copy(alpha = 0.14f), size = size, cornerRadius = round)
                val moving = 1f - determinate
                if (moving > 0f) {
                    val width = size.width * 0.32f
                    drawRoundRect(
                        ObiSilk.Amber.copy(alpha = moving * breath),
                        topLeft = Offset((size.width - width) * stretch, 0f),
                        size = Size(width, size.height),
                        cornerRadius = round,
                    )
                }
                if (determinate > 0f) {
                    drawRoundRect(
                        ObiSilk.Amber.copy(alpha = determinate),
                        size = Size(size.width * filled, size.height),
                        cornerRadius = round,
                    )
                }
            },
    )
}

/**
 * The starting screen's column: [width] wide, centred across and down on its closed height, so
 * what fills in below the title (the detail line, the buttons' room) moves nothing. The
 * [details] list hangs under it; as it opens ([open] from 0 to 1) the column slides up by
 * whatever would run past [bottom], and no further than [top] from the edge. A list taller than
 * the room left is measured to fit it, and scrolls itself.
 */
@Composable
fun StartColumn(
    width: Dp,
    open: Float,
    top: Dp,
    bottom: Dp,
    details: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Layout(
        content = {
            Box(Modifier.layoutId("closed")) { content() }
            Box(Modifier.layoutId("details")) { if (open > 0f) details() }
        },
        modifier = modifier,
    ) { measurables, constraints ->
        val w = minOf(width.roundToPx(), constraints.maxWidth)
        val height = constraints.maxHeight
        val closed = measurables.first { it.layoutId == "closed" }.measure(Constraints(minWidth = w, maxWidth = w))
        val room = max(0, height - bottom.roundToPx() - top.roundToPx() - closed.height)
        val hanging = measurables.first { it.layoutId == "details" }.measure(Constraints(minWidth = w, maxWidth = w, maxHeight = room))
        val shown = (hanging.height * open).toInt()
        val base = (height - closed.height) / 2
        val over = max(0, base + closed.height + shown - (height - bottom.roundToPx()))
        val y = max(top.roundToPx(), base - over)
        val x = (constraints.maxWidth - w) / 2
        layout(constraints.maxWidth, height) {
            closed.place(x, y)
            hanging.placeWithLayer(x, y + closed.height) { alpha = open }
        }
    }
}

/** Details' chevron, turned over when the list is open; its label stays. */
@Composable
fun StartChevron(open: Boolean, reduced: Boolean, color: Color, modifier: Modifier = Modifier) {
    val turn by animateFloatAsState(if (open) 180f else 0f, if (reduced) snap() else Motion.standard(), label = "chevron") // i18n-skip
    Canvas(modifier.size(12.dp).rotate(turn)) {
        val path = Path().apply {
            moveTo(size.width * 0.2f, size.height * 0.38f)
            lineTo(size.width * 0.5f, size.height * 0.68f)
            lineTo(size.width * 0.8f, size.height * 0.38f)
        }
        drawPath(path, color, style = Stroke(1.6.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round))
    }
}
