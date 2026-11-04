// A swap of content that crossfades, also with Remove animations on. The system's animator scale
// of 0 makes every timed animation land at once, which would turn a swap into a cut; the board
// says the swaps stay crossfades under Reduce Motion, both sides drawn, never an empty frame. So
// with the scale at 0 the fade runs on a clock of its own at full speed, and only for these swaps.

package app.kaeri.design

import android.content.Context
import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.MotionDurationScale
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.layer.drawLayer
import androidx.compose.ui.platform.LocalContext
import kotlinx.coroutines.withContext

private fun animationsRemoved(context: Context): Boolean =
    runCatching { Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) }
        .getOrDefault(1f) == 0f

/** Whether the system's animator duration scale is 0, read as the screen composes and again whenever it changes. */
@Composable
fun rememberAnimationsRemoved(): Boolean {
    val context = LocalContext.current
    var removed by remember { mutableStateOf(animationsRemoved(context)) }
    DisposableEffect(context) {
        val observer = object : ContentObserver(Handler(Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) {
                removed = animationsRemoved(context)
            }
        }
        context.contentResolver.registerContentObserver(Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, observer)
        removed = animationsRemoved(context)
        onDispose { context.contentResolver.unregisterContentObserver(observer) }
    }
    return removed
}

/** The full-speed clock a swap's fade runs on when the system's scale is 0. */
object FullSpeed : MotionDurationScale {
    override val scaleFactor: Float get() = 1f
}

/** Runs [block] at full speed whatever the system's animator scale says: for a crossfade Remove animations keeps. */
suspend fun <R> atFullSpeed(block: suspend () -> R): R = withContext(FullSpeed) { block() }

private class SwapLayer<T>(val state: T, val alpha: Animatable<Float, androidx.compose.animation.core.AnimationVector1D>)

/**
 * Crossfades [content] when [targetState] changes, like [Crossfade] with [animationSpec]. With
 * Remove animations on it still crossfades: the new content fades in over the old, which stays
 * drawn until the new is fully there, on [Motion.fadeIn]'s 200 ms.
 */
@Composable
fun <T> SwapFade(
    targetState: T,
    modifier: Modifier = Modifier,
    animationSpec: FiniteAnimationSpec<Float> = Motion.standard(),
    label: String = "swap", // i18n-skip
    content: @Composable (T) -> Unit,
) {
    if (rememberAnimationsRemoved()) {
        RemovedSwap(targetState, modifier, content)
    } else {
        Crossfade(targetState, modifier, animationSpec, label, content)
    }
}

@Composable
private fun <T> RemovedSwap(targetState: T, modifier: Modifier, content: @Composable (T) -> Unit) {
    val layers: SnapshotStateList<SwapLayer<T>> = remember { mutableStateListOf(SwapLayer(targetState, Animatable(1f))) }
    LaunchedEffect(targetState) {
        if (layers.last().state == targetState) return@LaunchedEffect
        val layer = SwapLayer(targetState, Animatable(0f))
        layers.add(layer)
        atFullSpeed { layer.alpha.animateTo(1f, Motion.fadeIn()) }
        layers.removeAll { it !== layer }
    }
    Box(modifier) {
        for (layer in layers.toList()) {
            key(layer) {
                Box(Modifier.graphicsLayer { alpha = layer.alpha.value }) { content(layer.state) }
            }
        }
    }
}

/**
 * A still of what a container showed, laid over it while a change that cannot crossfade by itself
 * happens underneath (a jump of a list, a navigation host with the animator scale at 0), then
 * faded out on [Motion.fadeIn]: the old and the new are both drawn, and no frame is empty. The
 * container takes [stillFade] and calls [swap] around the change.
 */
@androidx.compose.runtime.Stable
class StillFade {
    internal var layer: androidx.compose.ui.graphics.layer.GraphicsLayer? = null
    internal var still: androidx.compose.ui.graphics.ImageBitmap? by mutableStateOf(null)
    internal val alpha = Animatable(1f)

    /**
     * Runs [change] behind a still of the container as it is now, then fades the still out. With
     * [atFullSpeed] the fade runs at full speed whatever the system's animator scale says, for
     * Remove animations. Without a container to record, [change] just runs.
     */
    suspend fun swap(fullSpeed: Boolean = false, change: suspend () -> Unit) {
        val taken = layer?.let { runCatching { it.toImageBitmap() }.getOrNull() }
        if (taken == null) {
            change()
            return
        }
        alpha.snapTo(1f)
        still = taken
        try {
            change()
            if (fullSpeed) atFullSpeed { alpha.animateTo(0f, Motion.fadeIn()) } else alpha.animateTo(0f, Motion.fadeIn())
        } finally {
            still = null
        }
    }
}

@Composable
fun rememberStillFade(): StillFade = remember { StillFade() }

/** Records the container so [fade] can take a still of it, and draws the still over it while there is one. */
@Composable
fun Modifier.stillFade(fade: StillFade): Modifier {
    val layer = androidx.compose.ui.graphics.rememberGraphicsLayer()
    fade.layer = layer
    return drawWithContent {
        layer.record { this@drawWithContent.drawContent() }
        drawLayer(layer)
        fade.still?.let { drawImage(it, alpha = fade.alpha.value) }
    }
}
