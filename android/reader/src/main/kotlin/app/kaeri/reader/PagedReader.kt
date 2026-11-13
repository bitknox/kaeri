// The paged reader: one leaf at a time, turned by taps in the engine's zones, swipes, keys and jumps. On a
// phone a tap or key turn is a 0.2 s crossfade with the old page held under until the new one is
// drawn, never through empty, and Remove animations keeps that fade;
// a swipe follows the finger and turns past 25% of the width or a 450 dp/s flick with a spring that
// does not bounce. With motion off (e-ink, Paper mode) every turn is at once. Taps act at once:
// there is no double tap until the page is zoomed. A strip cut into screens (e-ink) also moves on
// with an upward swipe and back with a downward one. Pinch zooms, and while zoomed a drag pans and
// the tap zones are off. With swipes off (an e-ink switch) a drag neither turns nor counts as a tap.

package app.kaeri.reader

import androidx.compose.runtime.key
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import app.kaeri.engine.model.PageKind
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.TapAction
import app.kaeri.engine.model.TapReading
import app.kaeri.engine.model.TapZoneSettings
import kotlinx.coroutines.delay
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
import androidx.compose.runtime.produceState
import androidx.compose.foundation.background
import androidx.compose.ui.graphics.Color
import app.kaeri.engine.model.CreditPage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
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

    /** The leaf on screen is where the engine put it (its start place, a new layout), not a turn. */
    internal var placed = false
        private set

    var zoom by mutableFloatStateOf(1f)
        internal set
    var pan by mutableStateOf(Offset.Zero)
        internal set
    val zoomed: Boolean get() = zoom > 1.01f

    internal var count = 0

    /** Where a leaf starts panned: a page fitted wider or taller than the screen shows its reading edge. */
    internal var startPan: (Int) -> Offset = { Offset.Zero }

    /** Show [to] (clamped to the leaves there are), reached [how]. Returns false when it is already shown. */
    fun go(to: Int, how: TurnHow): Boolean = move(to, how, placed = false)

    /** The engine's own move: reported as placed, and nobody turned. */
    internal fun place(to: Int): Boolean = move(to, TurnHow.PLACED, placed = true)

    private fun move(to: Int, how: TurnHow, placed: Boolean): Boolean {
        val target = to.coerceIn(0, (count - 1).coerceAtLeast(0))
        if (target == leaf && turns > 0) return false
        this.placed = placed
        under = if (how == TurnHow.TURN || how == TurnHow.KEY || how == TurnHow.PLACED) leaf else null
        leaf = target
        this.how = how
        turns++
        zoom = 1f
        pan = startPan(target)
        return true
    }

    /** The last leaf is the end page, which a forward turn crosses from. */
    internal var endLast = false

    /**
     * Credit pages brought back lie between leaves: a turn steps through them first. True when
     * this turn was one of those steps.
     */
    internal var step: (forward: Boolean, how: TurnHow) -> Boolean = { _, _ -> false }

    /** The credit page drawn over the leaf being turned away from, while it fades out under the new one. */
    internal var underCredit by mutableStateOf<CreditPage?>(null)

    /** A credit step by swipe has already slid into place: the credit page shows without its fade. */
    internal var creditSlid = false

    /**
     * Show [step]: the credit page over its leaf, turning to that leaf first when it is another,
     * or the leaf alone. [onCredit] tells the session which credit page is on screen.
     */
    fun show(step: CreditStep, how: TurnHow, onCredit: (UInt?) -> Unit, shownCredit: CreditPage?) {
        when (step) {
            is CreditStep.Credit -> {
                if (step.leaf != leaf) {
                    underCredit = shownCredit
                    go(step.leaf, how)
                }
                onCredit(step.page)
            }
            is CreditStep.Leaf -> {
                onCredit(null)
                if (step.leaf != leaf) {
                    underCredit = shownCredit
                    go(step.leaf, how)
                }
            }
        }
    }

    /** A forward turn on the end page: on to the next chapter. False when there is none. */
    internal var pastEnd: () -> Boolean = { false }

    /** The next leaf; on the end page, the next chapter, as its hint says. */
    fun forward(how: TurnHow = TurnHow.TURN): Boolean =
        step(true, how) || if (endLast && leaf >= count - 1) pastEnd() else go(leaf + 1, how)

    fun back(how: TurnHow = TurnHow.TURN) = step(false, how) || go(leaf - 1, how)
}

@Composable
fun rememberPagedReaderState(initial: Int = 0): PagedReaderState = remember { PagedReaderState(initial) }

/**
 * The paged reader over [state]'s leaves. [motion] is false on e-ink and in Paper mode. A tap asks
 * the engine's tap zones in [taps] for [reading]; [onControls] runs for a tap in a Controls zone,
 * or any tap while zoomed. [onShown] runs after the frame that
 * shows a leaf, with how it was reached and whether the person turned to it (not the engine's start
 * place, and not the same leaf laid out again after a rotation); the screen passes it to
 * [ReaderSession.shown]. The end page leaf is drawn by [endPage].
 */
@Composable
fun PagedReader(
    state: ReaderState,
    pager: PagedReaderState,
    images: PageImages,
    direction: ReadDirection,
    taps: TapZoneSettings,
    reading: TapReading,
    onControls: () -> Unit,
    onShown: (leaf: Int, how: TurnHow, turned: Boolean) -> Unit,
    onMissing: (LeafPart) -> Unit,
    onRetry: (page: Int) -> Unit,
    onJumped: (id: Long) -> Unit,
    endPage: @Composable (Leaf) -> Unit,
    modifier: Modifier = Modifier,
    /** A forward turn on the end page, by swipe, tap or key: true when it crossed to the next chapter. */
    onPastEnd: () -> Boolean = { false },
    motion: Boolean = true,
    onReady: (leaf: Int) -> Unit = {},
    waiting: WaitingSlot = { page, long -> PageWaiting(page, long) },
    failed: FailedSlot = { page, failure, retry -> PageFailed(page, failure, retry) },
    /** The Fit setting: "screen", or "width" or "height" to fill that side and pan along the other. */
    fit: String? = null,
    /** The credit page on screen changed, by a turn through those brought back; null for none. */
    onCredit: (UInt?) -> Unit = {},
    /** What a credit page is drawn on, so the leaf under it does not show through. */
    creditGround: Color = Color.Black,
    /** A swipe turns the page; off, only taps, keys and page buttons do. */
    swipes: Boolean = true,
) {
    val leaves = state.leaves
    // On e-ink a strip is cut into screens shown one at a time.
    val cuts = leaves.firstOrNull { !it.endPage }?.parts?.firstOrNull()?.kind == PageKind.STRIP_CUT
    val zonesNow by rememberUpdatedState(taps)
    val readingNow by rememberUpdatedState(reading)
    pager.count = leaves.size
    pager.endLast = leaves.lastOrNull()?.endPage == true
    val pastEnd by rememberUpdatedState(onPastEnd)
    pager.pastEnd = { pastEnd() }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val rtl = direction.rightToLeft
    val shown by rememberUpdatedState(onShown)
    val latest by rememberUpdatedState(state)
    val creditNow by rememberUpdatedState(onCredit)
    pager.step = { forward, how ->
        val now = latest
        val step = now.creditStep(forward, pager.leaf)
        if (step != null) pager.show(step, how, creditNow, now.shownCredit)
        step != null
    }

    // The engine's own moves: the start place, Back to page, a place another device reached.
    // A new chapter's leaves count afresh: the old leaf number names another page there, so it is
    // never shown or faded from. Until the chapter's jump lands, the leaf it lands on shows.
    val chapter = state.view?.chapter?.id
    val fresh = chapter != pager.chapter
    LaunchedEffect(state.jump, leaves.size) {
        val jump = state.jump ?: return@LaunchedEffect
        if (leaves.isEmpty()) return@LaunchedEffect
        // The app's own move, which never offers Back to page.
        pager.place(jump.leaf)
        if (fresh) pager.under = null
        pager.chapter = chapter
        onJumped(jump.id)
    }
    if (fresh && state.jump == null && leaves.isNotEmpty()) SideEffect { pager.chapter = chapter }
    // A leaf counts as shown once it is on screen, never while a jump is still to come. A new layout
    // reports the leaf again without a turn, also when its jump lands on the leaf number already
    // shown: the engine makes the pages around the leaf it hears of.
    var reported by remember { mutableIntStateOf(pager.turns) }
    LaunchedEffect(pager.turns, state.layout, state.jump == null) {
        if (leaves.isEmpty() || state.jump != null) return@LaunchedEffect
        withFrameNanos { }
        val turned = pager.turns != reported && !pager.placed
        reported = pager.turns
        shown(pager.leaf, pager.how, turned)
    }

    // The crossfade: the new leaf over the old one, then the old one goes once the new one is drawn,
    // so a turn never shows the empty ground. With Remove animations on the fade keeps its own clock.
    val fade = remember { Animatable(1f) }
    val removed = rememberAnimationsRemoved()
    var drawn by remember { mutableIntStateOf(-1) }
    // With motion off each picture is drawn once. The leaf last drawn whole stays on screen until
    // the one after it (a turn, or the same place laid out anew when the device turns) is drawn
    // whole, for at most the plate's wait: the panel never shows the empty ground between them.
    var drawnLeaf by remember { mutableStateOf<Leaf?>(null) }
    val heldFrame = remember { arrayOfNulls<HeldFrame>(1) }

    // The pages beside the one shown are decoded ahead, so a turn draws its page in one frame.
    val near = remember(leaves, pager.leaf, state.files, state.held) {
        listOf(pager.leaf + 1, pager.leaf - 1).flatMap { leaves.getOrNull(it)?.parts.orEmpty() }.mapNotNull(state::file)
    }
    LaunchedEffect(near) {
        if (near.isNotEmpty()) withContext(Dispatchers.IO) { near.forEach { if (images.cached(it) == null) images.load(it) } }
    }
    LaunchedEffect(pager.turns) {
        if (pager.under == null) return@LaunchedEffect
        if (!motion) {
            pager.under = null
            pager.underCredit = null
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
        pager.underCredit = null
    }

    // The credit page over the leaf comes and goes by the same crossfade as a turn, or at once
    // with motion off, and at once when a turn of the leaf already fades it in.
    val credit = state.shownCredit
    var heldCredit by remember { mutableStateOf<CreditPage?>(null) }
    var leavingCredit by remember { mutableStateOf<CreditPage?>(null) }
    val creditFade = remember { Animatable(0f) }
    LaunchedEffect(credit?.page, state.chapter) {
        val before = heldCredit
        val atOnce = !motion || pager.under != null || pager.creditSlid
        pager.creditSlid = false
        if (credit == null) {
            if (before == null) return@LaunchedEffect
            if (atOnce) creditFade.snapTo(0f) else creditFade.animateTo(0f, Motion.quick())
            heldCredit = null
            return@LaunchedEffect
        }
        heldCredit = credit
        if (atOnce) return@LaunchedEffect creditFade.snapTo(1f)
        // From one credit page to the next over the same leaf: the old one under the new one.
        leavingCredit = before?.takeIf { it.page != credit.page }
        creditFade.snapTo(if (leavingCredit != null) 0f else creditFade.value)
        creditFade.animateTo(1f, Motion.quick())
        leavingCredit = null
    }


    var drag by remember { mutableFloatStateOf(0f) }
    var settling by remember { mutableStateOf<Job?>(null) }
    BoxWithConstraints(modifier.fillMaxSize()) {
        val width = constraints.maxWidth.toFloat().coerceAtLeast(1f)
        val height = constraints.maxHeight.toFloat().coerceAtLeast(1f)
        val flick = with(LocalDensity.current) { 450.dp.toPx() }
        // Fit: a single page's drawn size, and how much more it is scaled to fill the chosen side.
        val fitting = remember(leaves, state.files, fit, width, height) {
            { index: Int -> fitOf(leaves.getOrNull(index), state, fit, width, height) }
        }
        val fitNow by rememberUpdatedState(fitting)
        SideEffect {
            pager.startPan = { index ->
                val f = fitting(index)
                // Moved left, a page shows its right side first: where right to left starts.
                clampPan(Offset(if (rtl) -Float.MAX_VALUE else Float.MAX_VALUE, Float.MAX_VALUE), f.scale, width, height, f.content)
            }
        }
        // A change of Fit, or the page's size landing, puts the page at its reading edge.
        LaunchedEffect(fitting(pager.leaf)) {
            if (!pager.zoomed) pager.pan = pager.startPan(pager.leaf)
        }

        fun neighbour(offset: Float): Int? {
            // A drag to the right brings in the page at the left: the next one when reading right to left.
            val next = (offset > 0) == rtl
            val to = if (next) pager.leaf + 1 else pager.leaf - 1
            return to.takeIf { it in leaves.indices }
        }

        fun settle(velocity: Float) {
            val target = neighbour(drag)
            val far = abs(drag) > width * 0.25f || abs(velocity) > flick && (velocity > 0) == (drag > 0)
            // A swipe across credit pages brought back steps through them as a tap would.
            val step = if (far) latest.creditStep((drag > 0) == rtl, pager.leaf) else null
            if (step != null) {
                if (!motion) {
                    drag = 0f
                    pager.show(step, TurnHow.SWIPE, creditNow, latest.shownCredit)
                    return
                }
                val end = if (drag > 0) width else -width
                val from = latest.chapter
                settling = scope.launch {
                    animate(drag, end, velocity, Motion.standard()) { v, _ -> drag = v }
                    // Another chapter opened under the swipe: its step names a page there no more.
                    if (latest.chapter == from) {
                        pager.creditSlid = true
                        pager.show(step, TurnHow.SWIPE, creditNow, latest.shownCredit)
                    }
                    pager.under = null
                    drag = 0f
                }
                return
            }
            // On from the end page: the next chapter opens while the page eases back.
            val crossing = target == null && far && (drag > 0) == rtl && pager.endLast && pager.leaf == leaves.lastIndex
            if (crossing) pager.pastEnd()
            if (target != null && far) {
                if (!motion) {
                    drag = 0f
                    pager.go(target, TurnHow.SWIPE)
                    return
                }
                val end = if (drag > 0) width else -width
                val from = latest.chapter
                settling = scope.launch {
                    animate(drag, end, velocity, Motion.standard()) { v, _ -> drag = v }
                    // Another chapter opened under the swipe: its leaf number names another page
                    // there, so the new chapter stays where it opened.
                    if (latest.chapter == from) pager.go(target, TurnHow.SWIPE)
                    drag = 0f
                }
            } else if (motion) {
                settling = scope.launch { animate(drag, 0f, velocity, Motion.standard()) { v, _ -> drag = v } }
            } else {
                drag = 0f
            }
        }

        val pageWords = if (state.onUnkeptCredit) Strings.readerCreditPageLabel(context)
            else leaves.getOrNull(pager.leaf)?.let { leaf -> describe(context, leaf, state) }.orEmpty()
        Box(
            Modifier
                .fillMaxSize()
                .semantics {
                    contentDescription = pageWords
                    customActions = listOf(
                        CustomAccessibilityAction(Strings.readerActionNext(context)) { pager.forward() },
                        CustomAccessibilityAction(Strings.readerActionPrevious(context)) { pager.back() },
                        CustomAccessibilityAction(Strings.readerZoneShowControls(context)) { onControls(); true },
                    )
                }
                .pointerInput(pager.zoomed) {
                    if (pager.zoomed) {
                        // A double tap returns to fit; a single tap shows the controls.
                        detectTapGestures(
                            onDoubleTap = {
                                pager.zoom = 1f
                                pager.pan = pager.startPan(pager.leaf)
                            },
                            onTap = { onControls() },
                        )
                    } else {
                        detectTapGestures { at ->
                            when (zonesNow.actionAt(readingNow, at.x / size.width, at.y / size.height)) {
                                TapAction.NEXT -> pager.forward()
                                TapAction.PREVIOUS -> pager.back()
                                TapAction.CONTROLS -> onControls()
                            }
                        }
                    }
                }
                .pointerInput(rtl, motion, cuts, swipes) {
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
                        // A strip cut into screens moves on with an upward swipe, as a strip does.
                        var upDown = false
                        var total = Offset.Zero
                        var moved = Offset.Zero
                        var event: androidx.compose.ui.input.pointer.PointerEvent
                        do {
                            event = awaitPointerEvent()
                            val pressed = event.changes.filter { it.pressed }
                            if (pressed.size >= 2) {
                                // Pinch: zoom about the centre, pan within the page.
                                val f = fitNow(pager.leaf)
                                val before = pager.zoom
                                val zoom = (before * event.calculateZoom()).coerceIn(1f, 4f)
                                val ratio = zoom / before
                                val middle = Offset(size.width / 2f, size.height / 2f)
                                // The point under the fingers stays under them.
                                val held = pager.pan * ratio + (event.calculateCentroid() - middle) * (1f - ratio)
                                pager.zoom = zoom
                                pager.pan = clampPan(held + event.calculatePan(), f.scale * zoom, size.width.toFloat(), size.height.toFloat(), f.content)
                                if (dragging) {
                                    dragging = false
                                    drag = 0f
                                }
                                event.changes.forEach { it.consume() }
                            } else if (pressed.size == 1) {
                                val change = pressed.first()
                                val f = fitNow(pager.leaf)
                                var delta = change.positionChange()
                                if (pager.zoomed) {
                                    pager.pan = clampPan(pager.pan + delta, f.scale * pager.zoom, size.width.toFloat(), size.height.toFloat(), f.content)
                                    if (delta != Offset.Zero) change.consume()
                                } else {
                                    // A page fitted past the screen pans first, once the finger has moved
                                    // past a tap's slop; what is left at its edge turns.
                                    moved += delta
                                    if (f.scale > 1.01f && !dragging && moved.getDistance() > viewConfiguration.touchSlop) {
                                        val wanted = pager.pan + delta
                                        val panned = clampPan(wanted, f.scale, size.width.toFloat(), size.height.toFloat(), f.content)
                                        if (panned != pager.pan) change.consume()
                                        pager.pan = panned
                                        delta = Offset(wanted.x - panned.x, 0f)
                                    }
                                    total += delta
                                    if (!swipes) {
                                        // A hand resting on the glass: past a tap's slop it is no tap either.
                                        if (moved.getDistance() > viewConfiguration.touchSlop) change.consume()
                                    } else {
                                        if (!dragging && abs(total.x) > viewConfiguration.touchSlop && abs(total.x) > abs(total.y)) dragging = true
                                        if (cuts && !dragging && !upDown && abs(total.y) > viewConfiguration.touchSlop && abs(total.y) > abs(total.x)) upDown = true
                                        if (upDown) change.consume()
                                    }
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
                            val f = fitNow(pager.leaf)
                            pager.zoom = 1f
                            pager.pan = clampPan(pager.pan, f.scale, size.width.toFloat(), size.height.toFloat(), f.content)
                        }
                        if (upDown) {
                            if (total.y < 0) pager.forward(TurnHow.SWIPE) else pager.back(TurnHow.SWIPE)
                        } else if (dragging) {
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
                        val f = fitting(index)
                        translationX = shift + if (current) pager.pan.x else 0f
                        translationY = if (current) pager.pan.y else 0f
                        if (current) {
                            scaleX = pager.zoom * f.scale
                            scaleY = pager.zoom * f.scale
                        } else if (f.scale > 1.01f) {
                            // A neighbour comes in fitted too, at its reading edge.
                            val edge = clampPan(Offset(if (rtl) -Float.MAX_VALUE else Float.MAX_VALUE, Float.MAX_VALUE), f.scale, width, height, f.content)
                            translationX += edge.x
                            translationY = edge.y
                            scaleX = f.scale
                            scaleY = f.scale
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
                            ask = current,
                            onRetry = onRetry,
                            instant = !motion,
                            onReady = {
                                if (current) {
                                    drawn = index
                                    val was = drawnLeaf
                                    drawnLeaf = leaf
                                    if (motion || leaf == was) {
                                        onReady(index)
                                    } else {
                                        // A held frame gives way in the next frame: the leaf is ready once it is on screen.
                                        scope.launch {
                                            withFrameNanos { }
                                            onReady(index)
                                        }
                                    }
                                }
                            },
                            waiting = waiting,
                            failed = failed,
                        )
                    }
                }
            }
            @Composable
            fun Credit(page: CreditPage, alpha: Float, shift: Float = 0f) {
                CreditPageView(
                    page,
                    images,
                    creditGround,
                    Modifier.fillMaxSize().graphicsLayer {
                        this.alpha = alpha
                        translationX = shift
                    },
                )
            }
            if (leaves.isNotEmpty()) {
                val under = pager.under
                if (under != null && motion) {
                    Show(under)
                    pager.underCredit?.let { Credit(it, 1f) }
                }
                if (drag != 0f && motion) {
                    val shift = if (drag > 0) drag - width else drag + width
                    // What the swipe brings in: a credit page brought back, or the leaf after it.
                    when (val step = state.creditStep((drag > 0) == rtl, pager.leaf)) {
                        is CreditStep.Credit -> state.chapterCredits(state.chapter ?: "").firstOrNull { it.page == step.page }?.let {
                            if (step.leaf != pager.leaf) Show(step.leaf, shift = shift)
                            Credit(it, 1f, shift)
                        }
                        is CreditStep.Leaf -> if (step.leaf != pager.leaf) Show(step.leaf, shift = shift)
                        null -> neighbour(drag)?.let { Show(it, shift = shift) }
                    }
                }
                // The frame between a turn and its fade's start draws the old leaf alone, not the new one
                // whole for a frame before it fades in.
                val alpha = when {
                    under == null || !motion -> 1f
                    pager.fading != pager.turns -> 0f
                    else -> fade.value
                }
                val landing = state.jump?.leaf?.takeIf { fresh }?.coerceIn(0, leaves.lastIndex)
                val shift = if (motion) drag else 0f
                val leafAlpha = if (landing != null) 1f else alpha
                // A credit step back to its own leaf slides the credit page away and leaves the leaf in place.
                val stepsToSelf = drag != 0f && state.creditStep((drag > 0) == rtl, pager.leaf).let { it is CreditStep.Leaf && it.leaf == pager.leaf }
                val showing = landing ?: pager.leaf
                val hold = if (motion || landing != null) null else held(heldFrame, leaves.getOrNull(showing), drawnLeaf, state)
                Show(showing, alpha = if (hold != null) 0f else leafAlpha, shift = if (stepsToSelf) 0f else shift, current = true)
                if (hold != null) {
                    // The old frame as it was drawn, over the new leaf that draws unseen under it.
                    LeafView(
                        leaf = hold.leaf,
                        state = hold.state,
                        images = images,
                        onMissing = {},
                        onRetry = {},
                        modifier = Modifier.fillMaxSize().graphicsLayer {
                            translationX = hold.pan.x
                            translationY = hold.pan.y
                            scaleX = hold.scale
                            scaleY = hold.scale
                        },
                        instant = true,
                        waiting = { _, _ -> },
                        failed = { _, _, _ -> },
                    )
                } else if (!motion) {
                    leaves.getOrNull(showing)?.takeIf { it == drawnLeaf }?.let { leaf ->
                        heldFrame[0] = HeldFrame(leaf, state, pager.pan, pager.zoom * fitting(showing).scale)
                    }
                }
                // The leaf shown, the ones beside it and the frame held stay decoded whatever the
                // cache's size, so a held frame never leaves the screen to decode again.
                val shownFiles = leaves.getOrNull(showing)?.parts.orEmpty().mapNotNull(state::file)
                SideEffect {
                    val held = heldFrame[0]?.let { h -> h.leaf.parts.mapNotNull(h.state::file) }.orEmpty()
                    images.keep(shownFiles + near + held)
                }
                if (landing == null) {
                    leavingCredit?.let { Credit(it, leafAlpha, shift) }
                    heldCredit?.let { Credit(it, leafAlpha * creditFade.value, shift) }
                }
            }
        }
    }
}

/** A leaf drawn whole with motion off, as it was drawn: the state it was drawn from, its pan and scale. */
internal class HeldFrame(val leaf: Leaf, val state: ReaderState, val pan: Offset, val scale: Float)

/**
 * The frame to keep on screen over [showing], which is not drawn whole yet: the last one drawn
 * whole in this chapter, until [showing] is drawn or the plate's wait runs out. A new layout, the
 * device turned, keeps it until the plate's words would show: the pages are cut again first. Null
 * to draw [showing] as it is.
 */
@Composable
private fun held(frame: Array<HeldFrame?>, showing: Leaf?, drawnLeaf: Leaf?, state: ReaderState): HeldFrame? {
    val relaid = frame[0]?.let { it.state.layout != state.layout } == true
    // Each leaf waits afresh, also one shown before under the same layout.
    var waitedOut by remember(showing, relaid) { mutableStateOf(false) }
    LaunchedEffect(showing, relaid) {
        delay(if (relaid) PageWaits.WORDS_MS else PageWaits.PLATE_MS)
        waitedOut = true
    }
    if (showing == null || showing.endPage || showing == drawnLeaf || waitedOut) return null
    return frame[0]?.takeIf { it.state.chapter == state.chapter && it.leaf != showing }
}

/**
 * A credit page brought back, drawn whole on [ground] and faded in once it has decoded. Until then
 * the leaf under it shows for 600 ms, then the reader's waiting plate; a page that cannot be had
 * gets the failed page with Try again. In a [strip] the plate is the strip's own.
 */
@Composable
fun CreditPageView(credit: CreditPage, images: PageImages, ground: Color, modifier: Modifier = Modifier, strip: Boolean = false) {
    val context = LocalContext.current
    var tries by remember(credit.file) { mutableStateOf(0) }
    val first = remember(credit.file) { images.cached(credit.file) }
    val result by produceState(first?.let { PageDecodeResult.Decoded(it) } as PageDecodeResult?, credit.file, tries) {
        if (value is PageDecodeResult.Decoded) return@produceState
        value = null
        value = withContext(Dispatchers.IO) { images.load(credit.file) }
    }
    val alpha = remember(credit.file) { Animatable(if (first != null) 1f else 0f) }
    val decoded = result as? PageDecodeResult.Decoded
    LaunchedEffect(decoded != null) {
        if (decoded != null && alpha.value < 1f) atFullSpeed { alpha.animateTo(1f, Motion.fadeIn()) }
    }
    val page = credit.page.toInt()
    Box(
        modifier.semantics {
            contentDescription = Strings.readerCreditPageLabel(context)
            role = Role.Image
        },
    ) {
        when (val now = result) {
            is PageDecodeResult.Decoded -> PageCanvas(
                now.page,
                rotate = false,
                modifier = Modifier.fillMaxSize().graphicsLayer { this.alpha = alpha.value }.background(ground),
            )
            null -> key(tries) {
                var plate by remember { mutableStateOf(0) }
                LaunchedEffect(Unit) {
                    delay(PageWaits.PLATE_MS)
                    plate = 1
                    delay(PageWaits.WORDS_MS - PageWaits.PLATE_MS)
                    plate = 2
                }
                if (plate > 0) {
                    if (strip) ImageWaiting(page, plate > 1) else PageWaiting(page, plate > 1)
                }
            }
            is PageDecodeResult.Missing -> Box(Modifier.fillMaxSize().background(ground)) {
                PageFailed(page, ReadingFailure.NotFound) { tries++ }
            }
            is PageDecodeResult.Unreadable -> Box(Modifier.fillMaxSize().background(ground)) {
                PageFailed(page, ReadingFailure.Other(now.reason)) { tries++ }
            }
        }
    }
}

/** What a screen reader says for a leaf: its page, or its two pages. */
internal fun describe(context: android.content.Context, leaf: Leaf, state: ReaderState): String {
    val pages = state.shownPages
    val numbers = leaf.parts.map { state.shownPage(it.page.toInt()) + 1 }.distinct()
    return when (numbers.size) {
        0 -> ""
        1 -> Strings.readerPageOf(context, numbers[0], pages)
        else -> Strings.readerPagesOf(context, numbers.min(), numbers.max(), pages)
    }
}

/**
 * Keeps a zoomed page's edges on screen. [content] is what is drawn at no zoom, centred: the whole
 * screen unless a page fitted inside it leaves bands.
 */
internal fun clampPan(pan: Offset, zoom: Float, width: Float, height: Float, content: Size = Size(width, height)): Offset {
    val x = ((content.width * zoom - width) / 2).coerceAtLeast(0f)
    val y = ((content.height * zoom - height) / 2).coerceAtLeast(0f)
    return Offset(pan.x.coerceIn(-x, x), pan.y.coerceIn(-y, y))
}

/** A leaf under Fit: the size it is drawn at to fit the screen, and the scale that fills the chosen side. */
internal data class LeafFit(val content: Size, val scale: Float)

/**
 * How Fit draws [leaf] on a [width] by [height] screen. Screen fits the whole page; Height scales a
 * page that leaves bands above and below until it fills the height, Width one with bands at the
 * sides until it fills the width. Only a leaf of one page with its size known is scaled.
 */
internal fun fitOf(leaf: Leaf?, state: ReaderState, fit: String?, width: Float, height: Float): LeafFit {
    val whole = LeafFit(Size(width, height), 1f)
    if (leaf == null || leaf.endPage || leaf.parts.size != 1 || (fit != "width" && fit != "height")) return whole
    val part = leaf.parts.first()
    val file = state.file(part) ?: return whole
    val boxW = part.place.width * width
    val boxH = part.place.height * height
    val w = (file.crop?.width ?: file.width).toFloat().coerceAtLeast(1f)
    val h = (file.crop?.height ?: file.height).toFloat().coerceAtLeast(1f)
    val (iw, ih) = if (part.rotate) h to w else w to h
    val inside = minOf(boxW / iw, boxH / ih)
    val drawn = Size(iw * inside, ih * inside)
    val scale = when (fit) {
        "height" -> if (drawn.height < height - 1f) height / drawn.height else 1f
        else -> if (drawn.width < width - 1f) width / drawn.width else 1f
    }
    return LeafFit(drawn, scale.coerceIn(1f, 4f))
}
