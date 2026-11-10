// The paged reader: one leaf at a time, turned by taps in the zones, swipes, keys and jumps. On a
// phone a tap or key turn is a 0.2 s crossfade with the old page held under until the new one is
// drawn, never through empty, and Remove animations keeps that fade;
// a swipe follows the finger and turns past 25% of the width or a 450 dp/s flick with a spring that
// does not bounce. With motion off (e-ink, Paper mode) every turn is at once. Pinch zooms, and
// while zoomed a drag pans and the tap zones are off.

package app.kaeri.reader

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animate
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.util.VelocityTracker
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.kaeri.design.Motion
import app.kaeri.design.atFullSpeed
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.LeafPart
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.TurnHow
import app.kaeri.i18n.Strings
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.math.abs

/** Where the paged reader is, for the screen to turn it from keys, bars and the scrubber. */
@Stable
class PagedReaderState(initial: Int = 0) {
    /** The leaf on screen. */
    var leaf by mutableIntStateOf(initial)
        private set

    /** The leaf being turned away from, held under the new one while it fades in. */
    internal var under by mutableStateOf<Int?>(null)

    /** The chapter whose leaves [leaf] counts in. */
    internal var chapter by mutableStateOf<Any?>(null)

    /** The turn whose crossfade has started; until then the new leaf waits under nothing. */
    internal var fading by mutableIntStateOf(0)

    /** How the leaf on screen was reached, and a count that moves with each turn. */
    var how by mutableStateOf(TurnHow.TURN)
        private set
    var turns by mutableIntStateOf(0)
        private set

    var zoom by mutableFloatStateOf(1f)
        internal set
    var pan by mutableStateOf(Offset.Zero)
        internal set
    val zoomed: Boolean get() = zoom > 1.01f

    internal var count = 0

    /** Show [to] (clamped to the leaves there are), reached [how]. Returns false when it is already shown. */
    fun go(to: Int, how: TurnHow): Boolean {
        val target = to.coerceIn(0, (count - 1).coerceAtLeast(0))
        if (target == leaf && turns > 0) return false
        under = if (how == TurnHow.TURN || how == TurnHow.KEY || how == TurnHow.PLACED) leaf else null
        leaf = target
        this.how = how
        turns++
        zoom = 1f
        pan = Offset.Zero
        return true
    }

    fun forward(how: TurnHow = TurnHow.TURN) = go(leaf + 1, how)

    fun back(how: TurnHow = TurnHow.TURN) = go(leaf - 1, how)
}

@Composable
fun rememberPagedReaderState(initial: Int = 0): PagedReaderState = remember { PagedReaderState(initial) }

/**
 * The paged reader over [state]'s leaves. [motion] is false on e-ink and in Paper mode. [onControls]
 * runs for a tap in the middle zone, or any tap while zoomed. [onShown] runs after the frame that
 * shows a leaf, with how it was reached; the screen passes it to [ReaderSession.shown]. The end page
 * leaf is drawn by [endPage].
 */
@Composable
fun PagedReader(
    state: ReaderState,
    pager: PagedReaderState,
    images: PageImages,
    direction: ReadDirection,
    zones: TapZones,
    onControls: () -> Unit,
    onShown: (leaf: Int, how: TurnHow) -> Unit,
    onMissing: (LeafPart) -> Unit,
    onRetry: (page: Int) -> Unit,
    onJumped: (id: Long) -> Unit,
    endPage: @Composable (Leaf) -> Unit,
    modifier: Modifier = Modifier,
    motion: Boolean = true,
    onReady: (leaf: Int) -> Unit = {},
    waiting: WaitingSlot = { page, long -> PageWaiting(page, long) },
    failed: FailedSlot = { page, failure, retry -> PageFailed(page, failure, retry) },
) {
    val leaves = state.leaves
    pager.count = leaves.size
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val rtl = direction.rightToLeft
    val shown by rememberUpdatedState(onShown)

    // The engine's own moves: the start place, Back to page, a place another device reached.
    // A new chapter's leaves count afresh: the old leaf number names another page there, so it is
    // never shown or faded from. Until the chapter's jump lands, the leaf it lands on shows.
    val chapter = state.view?.chapter?.id
    val fresh = chapter != pager.chapter
    LaunchedEffect(state.jump, leaves.size) {
        val jump = state.jump ?: return@LaunchedEffect
        if (leaves.isEmpty()) return@LaunchedEffect
        // The app's own move, which never offers Back to page.
        pager.go(jump.leaf, TurnHow.PLACED)
        if (fresh) pager.under = null
        pager.chapter = chapter
        onJumped(jump.id)
    }
    if (fresh && state.jump == null && leaves.isNotEmpty()) SideEffect { pager.chapter = chapter }
    // A leaf counts as shown once it is on screen, never while a jump is still to come.
    LaunchedEffect(pager.turns, state.layout) {
        if (leaves.isEmpty() || state.jump != null) return@LaunchedEffect
        withFrameNanos { }
        shown(pager.leaf, pager.how)
    }

    // The crossfade: the new leaf over the old one, then the old one goes once the new one is drawn,
    // so a turn never shows the empty ground. With Remove animations on the fade keeps its own clock.
    val fade = remember { Animatable(1f) }
    val removed = rememberAnimationsRemoved()
    var drawn by remember { mutableIntStateOf(-1) }
    LaunchedEffect(pager.turns) {
        if (pager.under == null) return@LaunchedEffect
        if (!motion) {
            pager.under = null
            return@LaunchedEffect
        }
        val turn: suspend () -> Unit = {
            fade.snapTo(0f)
            pager.fading = pager.turns
            fade.animateTo(1f, Motion.quick())
            withTimeoutOrNull(PageWaits.PLATE_MS) { snapshotFlow { drawn == pager.leaf }.first { it } }
        }
        if (removed) atFullSpeed { turn() } else turn()
        pager.under = null
    }

    var drag by remember { mutableFloatStateOf(0f) }
    var settling by remember { mutableStateOf<Job?>(null) }
    BoxWithConstraints(modifier.fillMaxSize()) {
        val width = constraints.maxWidth.toFloat().coerceAtLeast(1f)
        val flick = with(LocalDensity.current) { 450.dp.toPx() }

        fun neighbour(offset: Float): Int? {
            // A drag to the right brings in the page at the left: the next one when reading right to left.
            val next = (offset > 0) == rtl
            val to = if (next) pager.leaf + 1 else pager.leaf - 1
            return to.takeIf { it in leaves.indices }
        }

        fun settle(velocity: Float) {
            val target = neighbour(drag)
            val far = abs(drag) > width * 0.25f || abs(velocity) > flick && (velocity > 0) == (drag > 0)
            if (target != null && far) {
                if (!motion) {
                    drag = 0f
                    pager.go(target, TurnHow.SWIPE)
                    return
                }
                val end = if (drag > 0) width else -width
                settling = scope.launch {
                    animate(drag, end, velocity, Motion.standard()) { v, _ -> drag = v }
                    pager.go(target, TurnHow.SWIPE)
                    drag = 0f
                }
            } else if (motion) {
                settling = scope.launch { animate(drag, 0f, velocity, Motion.standard()) { v, _ -> drag = v } }
            } else {
                drag = 0f
            }
        }

        val pageWords = leaves.getOrNull(pager.leaf)?.let { leaf -> describe(context, leaf, state.pages) }.orEmpty()
        Box(
            Modifier
                .fillMaxSize()
                .semantics {
                    contentDescription = pageWords
                    customActions = listOf(
                        CustomAccessibilityAction(Strings.readerActionNext(context)) { pager.forward() },
                        CustomAccessibilityAction(Strings.readerActionPrevious(context)) { pager.back() },
                        CustomAccessibilityAction(Strings.readerActionControls(context)) { onControls(); true },
                    )
                }
                .pointerInput(rtl, zones, motion, pager.zoomed) {
                    if (pager.zoomed) {
                        // A double tap returns to fit; a single tap shows the controls.
                        detectTapGestures(
                            onDoubleTap = {
                                pager.zoom = 1f
                                pager.pan = Offset.Zero
                            },
                            onTap = { onControls() },
                        )
                    } else {
                        detectTapGestures { at ->
                            when (zones.at(at.x / size.width)) {
                                ReaderAction.FORWARD -> pager.forward()
                                ReaderAction.BACK -> pager.back()
                                else -> onControls()
                            }
                        }
                    }
                }
                .pointerInput(rtl, motion) {
                    awaitEachGesture {
                        val down = awaitFirstDown(requireUnconsumed = false)
                        // A new touch takes over a settling swipe from where it is. A page caught
                        // between two places is held there until the finger moves or lifts.
                        val caught = settling?.isActive == true && drag != 0f
                        settling?.cancel()
                        settling = null
                        val tracker = VelocityTracker()
                        tracker.addPosition(down.uptimeMillis, down.position)
                        var dragging = false
                        var total = Offset.Zero
                        var event: androidx.compose.ui.input.pointer.PointerEvent
                        do {
                            event = awaitPointerEvent()
                            val pressed = event.changes.filter { it.pressed }
                            if (pressed.size >= 2) {
                                // Pinch: zoom about the centre, pan within the page.
                                val before = pager.zoom
                                val zoom = (before * event.calculateZoom()).coerceIn(1f, 4f)
                                val ratio = zoom / before
                                val middle = Offset(size.width / 2f, size.height / 2f)
                                // The point under the fingers stays under them.
                                val held = pager.pan * ratio + (event.calculateCentroid() - middle) * (1f - ratio)
                                pager.zoom = zoom
                                pager.pan = clampPan(held + event.calculatePan(), zoom, size.width.toFloat(), size.height.toFloat())
                                if (dragging) {
                                    dragging = false
                                    drag = 0f
                                }
                                event.changes.forEach { it.consume() }
                            } else if (pressed.size == 1) {
                                val change = pressed.first()
                                val delta = change.positionChange()
                                if (pager.zoomed) {
                                    pager.pan = clampPan(pager.pan + delta, pager.zoom, size.width.toFloat(), size.height.toFloat())
                                    if (delta != Offset.Zero) change.consume()
                                } else {
                                    total += delta
                                    if (!dragging && abs(total.x) > viewConfiguration.touchSlop && abs(total.x) > abs(total.y)) dragging = true
                                    if (dragging) {
                                        tracker.addPosition(change.uptimeMillis, change.position)
                                        // With motion off the page stays put until the finger lifts.
                                        drag = if (motion) drag + delta.x else total.x
                                        change.consume()
                                    }
                                }
                            }
                        } while (event.changes.any { it.pressed })
                        if (pager.zoom <= 1.01f) {
                            pager.zoom = 1f
                            pager.pan = Offset.Zero
                        }
                        if (dragging) {
                            settle(tracker.calculateVelocity().x)
                        } else if (caught && drag != 0f) {
                            // A tap on a page caught mid-swipe finishes the swipe the way it was going,
                            // and is not also a tap: the zones would turn a second time.
                            event.changes.forEach { it.consume() }
                            settle(0f)
                        }
                    }
                },
        ) {
            @Composable
            fun Show(index: Int, alpha: Float = 1f, shift: Float = 0f, current: Boolean = false) {
                val leaf = leaves.getOrNull(index) ?: return
                Box(
                    Modifier.fillMaxSize().graphicsLayer {
                        this.alpha = alpha
                        translationX = shift + if (current) pager.pan.x else 0f
                        translationY = if (current) pager.pan.y else 0f
                        if (current) {
                            scaleX = pager.zoom
                            scaleY = pager.zoom
                        }
                    },
                ) {
                    if (leaf.endPage) {
                        endPage(leaf)
                    } else {
                        LeafView(
                            leaf = leaf,
                            state = state,
                            images = images,
                            onMissing = onMissing,
                            onRetry = onRetry,
                            instant = !motion,
                            onReady = {
                                if (current) {
                                    drawn = index
                                    onReady(index)
                                }
                            },
                            waiting = waiting,
                            failed = failed,
                        )
                    }
                }
            }
            if (leaves.isNotEmpty()) {
                val under = pager.under
                if (under != null && motion) Show(under)
                if (drag != 0f && motion) neighbour(drag)?.let { Show(it, shift = if (drag > 0) drag - width else drag + width) }
                // The frame between a turn and its fade's start draws the old leaf alone, not the new one
                // whole for a frame before it fades in.
                val alpha = when {
                    under == null || !motion -> 1f
                    pager.fading != pager.turns -> 0f
                    else -> fade.value
                }
                val landing = state.jump?.leaf?.takeIf { fresh }?.coerceIn(0, leaves.lastIndex)
                Show(landing ?: pager.leaf, alpha = if (landing != null) 1f else alpha, shift = if (motion) drag else 0f, current = true)
            }
        }
    }
}

/** What a screen reader says for a leaf: its page, or its two pages. */
private fun describe(context: android.content.Context, leaf: Leaf, pages: Int): String {
    val numbers = leaf.parts.map { it.page.toInt() + 1 }.distinct()
    return when (numbers.size) {
        0 -> ""
        1 -> Strings.readerPageOf(context, numbers[0], pages)
        else -> Strings.readerPagesOf(context, numbers.min(), numbers.max(), pages)
    }
}

/** Keeps a zoomed page's edges on screen. */
internal fun clampPan(pan: Offset, zoom: Float, width: Float, height: Float): Offset {
    val x = (zoom - 1f) * width / 2
    val y = (zoom - 1f) * height / 2
    return Offset(pan.x.coerceIn(-x, x), pan.y.coerceIn(-y, y))
}
