// What the starting screen is made of on a phone and on the TV: Kaeri's line, the column that stays
// where it is while a start fills it in, the ground over the art and the reduced-motion switch.

package app.kaeri.design

import android.provider.Settings
import androidx.compose.animation.core.FastOutSlowInEasing
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.draw.alpha
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.layout.layoutId
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.max
import kotlin.math.min

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
 * over 2 s and back (a 4 s cycle). Given a [percent] (only from mpv's buffering) it becomes a fill that follows each
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
    // The breathing runs on the frame clock, which the animator scale does not touch, so it keeps
    // going under Remove animations. 4 s a cycle: 2 s from 100 % to 40 %, 2 s back.
    var breath by remember { mutableFloatStateOf(1f) }
    if (!still) {
        LaunchedEffect(reduced) {
            if (!reduced) {
                breath = 1f
                return@LaunchedEffect
            }
            val begun = withFrameNanos { it }
            while (true) {
                withFrameNanos { now -> breath = breathing((now - begun) / 1_000_000L) }
            }
        }
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
        // What the column may take, so the room it keeps for what has not come yet gives way in a short window.
        val available = max(0, height - bottom.roundToPx() - top.roundToPx())
        val closed = measurables.first { it.layoutId == "closed" }.measure(Constraints(minWidth = w, maxWidth = w, maxHeight = available))
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

/** The loader's alpha [elapsed] milliseconds into the breathing: 1.0 to 0.4 over 2 s, and back over 2 s. */
fun breathing(elapsed: Long): Float {
    val half = (elapsed % BREATH_CYCLE_MS) / BREATH_HALF_MS.toFloat()
    val fraction = if (half <= 1f) half else 2f - half
    return 1f - 0.6f * fraction
}

private const val BREATH_HALF_MS = 2_000L
private const val BREATH_CYCLE_MS = 4_000L

private class FadeLayer<T>(val value: T, alpha: Float) {
    var alpha by mutableFloatStateOf(alpha)
}

/**
 * A crossfade timed from the frame clock, so it runs the same under Remove animations (animator
 * scale 0), which turns Compose's own animation specs into cuts. The newest [target] rises while
 * the ones before it fall; the first is there at once. Nothing fades through empty: the old value
 * stays until the new one is on its way up.
 */
@Composable
fun <T> FrameCrossfade(target: T, modifier: Modifier = Modifier, millis: Int = 300, content: @Composable (T) -> Unit) {
    val layers = remember { mutableStateListOf<FadeLayer<T>>().apply { add(FadeLayer(target, 1f)) } }
    LaunchedEffect(target) {
        if (layers.last().value != target) layers.add(FadeLayer(target, 0f))
        var last = withFrameNanos { it }
        while (layers.size > 1 || layers.last().alpha < 1f) {
            withFrameNanos { now ->
                val step = ((now - last) / 1_000_000f / millis).coerceAtMost(1f)
                last = now
                val newest = layers.last()
                newest.alpha = (newest.alpha + step).coerceAtMost(1f)
                for (layer in layers) if (layer !== newest) layer.alpha = (layer.alpha - step).coerceAtLeast(0f)
                // An older layer goes once the newest is up, not before, so there is always something under.
                layers.removeAll { it !== newest && it.alpha <= 0f && newest.alpha >= 1f }
            }
        }
    }
    Box(modifier) {
        for (layer in layers.toList()) {
            Box(Modifier.alpha(layer.alpha)) { content(layer.value) }
        }
    }
}

/**
 * A row of buttons that takes the height of the tallest row it can come to ([reserve]), from the
 * start, so the column around it never moves when buttons arrive. [reserve] is measured, never
 * placed or drawn.
 */
@Composable
fun ReservedHeight(reserve: @Composable () -> Unit, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Layout(
        content = {
            Box(Modifier.layoutId("reserve").clearAndSetSemantics { }) { reserve() }
            Box(Modifier.layoutId("live")) { content() }
        },
        modifier = modifier,
    ) { measurables, constraints ->
        val wide = Constraints(minWidth = constraints.maxWidth, maxWidth = constraints.maxWidth)
        val reserved = measurables.first { it.layoutId == "reserve" }.measure(wide)
        val live = measurables.first { it.layoutId == "live" }.measure(wide)
        val cap = if (constraints.hasBoundedHeight) constraints.maxHeight else Int.MAX_VALUE
        layout(constraints.maxWidth, max(min(reserved.height, cap), live.height)) {
            live.place(0, 0)
        }
    }
}
