// The cast glyph and the cast button in the top bars: an outline screen while TVs are there, its
// waves pulsing while a TV connects, the screen filled inside its frame once one is held.

package app.kaeri.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.StartOffset
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.Canvas
import androidx.compose.ui.draw.drawBehind
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.i18n.R
import app.kaeri.store.CastButtonState

/** How the glyph draws: an outline screen, the same with its waves pulsing, or a filled screen. */
enum class CastGlyphState { Outline, Connecting, Filled }

private val OUTLINE_SCREEN = PathParser().parsePathString("M3 7.4 V3.6 H21 V19.2 H13.6").toPath()
// Inside the outline screen, 1.8 clear of its frame all round, its corner cut on a circle about the
// waves' centre (2.6, 19.4) that keeps the same 1.8 clear of the outer wave.
private val FILLED_SCREEN = PathParser().parsePathString("M5.7 6.3 H18.3 V16.5 H13.73 A11.5 11.5 0 0 0 5.7 8.33 Z").toPath()
private val OUTER_WAVE = PathParser().parsePathString("M3 10.4 A9 9 0 0 1 11.6 19.2").toPath()
private val INNER_WAVE = PathParser().parsePathString("M3 14 A5.4 5.4 0 0 1 8 19.2").toPath()

/** One wave's opacity over the 1.2 s pulse: 0.3, up to 1 at 30 %, back to 0.3 at 60 %. */
private fun pulse(ms: Int): Float {
    val t = ((ms % 1200) + 1200) % 1200 / 1200f
    return when {
        t < 0.3f -> 0.3f + 0.7f * (t / 0.3f)
        t < 0.6f -> 1f - 0.7f * ((t - 0.3f) / 0.3f)
        else -> 0.3f
    }
}

/**
 * The cast glyph on the 24 grid, stroked at 1.8. While connecting, the dot and both waves pulse
 * 0.2 s apart; with animations removed they hold still on the pulse's middle frame.
 */
@Composable
fun CastGlyph(state: CastGlyphState, modifier: Modifier = Modifier, size: Dp = 24.dp, tint: Color = LocalContentColor.current) {
    val still = app.kaeri.design.rememberAnimationsRemoved()
    var dot = 1f
    var inner = 1f
    var outer = 1f
    if (state == CastGlyphState.Connecting) {
        if (still) {
            dot = pulse(600)
            inner = pulse(400)
            outer = pulse(200)
        } else {
            val waves = rememberInfiniteTransition(label = "cast waves") // i18n-skip
            val spec = { delay: Int ->
                infiniteRepeatable<Float>(
                    keyframes {
                        durationMillis = 1200
                        0.3f at 0 using LinearEasing
                        1f at 360 using LinearEasing
                        0.3f at 720 using LinearEasing
                        0.3f at 1200
                    },
                    RepeatMode.Restart,
                    StartOffset(delay),
                )
            }
            dot = waves.animateFloat(0.3f, 0.3f, spec(0), label = "dot").value // i18n-skip
            inner = waves.animateFloat(0.3f, 0.3f, spec(200), label = "inner").value // i18n-skip
            outer = waves.animateFloat(0.3f, 0.3f, spec(400), label = "outer").value // i18n-skip
        }
    }
    Canvas(modifier.size(size)) {
        scale(this.size.width / 24f, pivot = Offset.Zero) {
            val stroke = Stroke(width = 1.8f, cap = StrokeCap.Round, join = StrokeJoin.Round)
            // Held, the frame stays and the screen inside it fills, as the system's cast icon does.
            drawPath(OUTLINE_SCREEN, tint, style = stroke)
            if (state == CastGlyphState.Filled) drawPath(FILLED_SCREEN, tint)
            drawPath(OUTER_WAVE, tint, alpha = outer, style = stroke)
            drawPath(INNER_WAVE, tint, alpha = inner, style = stroke)
            drawCircle(tint, radius = 1.6f, center = Offset(3.9f, 18.3f), alpha = dot)
        }
    }
}

/** The glyph the button's state draws. */
fun CastButtonState.glyph(): CastGlyphState = when (this) {
    CastButtonState.Connecting -> CastGlyphState.Connecting
    CastButtonState.Connected -> CastGlyphState.Filled
    else -> CastGlyphState.Outline
}

/**
 * The cast button, before the account or More in a top bar: hidden while no TV is found. It
 * opens the chooser, the held TV's dialog, or the card of a cast started from Play on TV.
 * Connecting and connected are said once each.
 *
 * @param circle on the title page's banner, the 40 circle its neighbours sit in.
 * @param circleAlpha how strong that circle is drawn, read as it draws: the title page's bar fades it.
 */
@Composable
fun CastButton(circle: Boolean = false, circleAlpha: () -> Float = { 1f }) {
    val cast = LocalAppStore.current.cast
    val state by cast.button.collectAsStateWithLifecycle()
    val session by cast.session.collectAsStateWithLifecycle()
    val joining by cast.joining.collectAsStateWithLifecycle()
    val tvName by LocalAppStore.current.tv.connected.collectAsStateWithLifecycle()
    val shown by cast.shown.collectAsStateWithLifecycle()
    val name = session.deviceName ?: joining ?: shown?.tvName ?: tvName?.let { LocalAppStore.current.tv.card.value?.card?.tvName }.orEmpty()
    // While the TV comes back, the name says Reconnecting, as the mini's line does.
    val reconnecting = castReconnecting(cast)
    val label = when (state) {
        CastButtonState.Connecting -> stringResource(R.string.cast_button_connecting, name)
        CastButtonState.Connected -> stringResource(if (reconnecting) R.string.tv_status_reconnecting else R.string.cast_button_connected, name)
        else -> stringResource(R.string.tv_play_on)
    }
    // Said once as it changes, never as a screen with the button opens.
    val view = LocalView.current
    var said by remember { mutableStateOf(state) }
    LaunchedEffect(state, label) {
        if (state != said && (state == CastButtonState.Connecting || state == CastButtonState.Connected)) view.announceForAccessibility(label)
        said = state
    }
    AnimatedVisibility(state != CastButtonState.Hidden, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
        IconButton(onClick = cast::pressButton, modifier = Modifier.semantics { contentDescription = label }) {
            if (circle) {
                val container = MaterialTheme.colorScheme.surfaceContainer
                Box(Modifier.size(40.dp).drawBehind { drawCircle(container.copy(alpha = container.alpha * circleAlpha())) }, contentAlignment = Alignment.Center) {
                    CastGlyph(state.glyph())
                }
            } else {
                CastGlyph(state.glyph())
            }
        }
    }
}

// A Play button while a TV is held: the title page's, Home's, and Play in Continue's menus

/** The cast glyph in a Play button, for a test to find. */
internal const val CAST_PLAY_GLYPH = "cast-play-glyph" // i18n-skip

/** A Play button's leading glyph: the cast glyph while a TV is held, as the play goes there; Play otherwise. */
@Composable
internal fun CastPlayIcon() {
    val session by LocalAppStore.current.cast.session.collectAsStateWithLifecycle()
    if (session.held) CastGlyph(CastGlyphState.Filled, Modifier.testTag(CAST_PLAY_GLYPH), size = 24.dp) else androidx.compose.material3.Icon(Icons.Play, contentDescription = null, modifier = Modifier.size(24.dp))
}

/** Under the main button while a TV is held: the water dot and "Plays on Living room". */
@Composable
internal fun CastPlaysOn(modifier: Modifier = Modifier) {
    val session by LocalAppStore.current.cast.session.collectAsStateWithLifecycle()
    val name = session.deviceName
    AnimatedVisibility(session.held && name != null, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(Motion.fadeOut())) {
        androidx.compose.foundation.layout.Row(
            modifier.heightIn(min = 24.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp),
        ) {
            StateDot(app.kaeri.design.ObiSilk.Water)
            androidx.compose.material3.Text(
                stringResource(R.string.cast_plays_on, name.orEmpty()),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

/** A Play button's place on screen, kept for the controller to grow from when the play goes to a TV. */
@Stable
internal class CastOriginMark(private val origin: CastOrigin?) {
    private var bounds: androidx.compose.ui.geometry.Rect? = null

    val modifier: Modifier = Modifier.onGloballyPositioned { bounds = it.boundsInRoot() }

    /** The button was pressed: the controller, if it comes up now, grows from it. */
    fun mark() {
        origin?.rect = bounds
    }
}

@Composable
internal fun rememberCastOrigin(): CastOriginMark {
    val origin = LocalCastOrigin.current
    return remember(origin) { CastOriginMark(origin) }
}
