// The cast controller: the mini over the navigation bar while a TV plays, and the full controller
// it grows into. One surface morphs between them on the standard spring (SharedTransitionLayout),
// and a back swipe seeks it back into the mini. The engine's card says what both read
// (CastModel); nothing here keeps a state of its own beyond a dragged seek held until the
// engine's next card.

package app.kaeri.ui

import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.togetherWith
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.snap
import androidx.compose.ui.graphics.Color
import androidx.compose.material3.ButtonDefaults
import androidx.compose.runtime.key
import androidx.compose.material3.SnackbarHost
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.BoundsTransform
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.SeekableTransitionState
import androidx.compose.animation.core.Transition
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.animation.core.animateDp
import androidx.compose.animation.core.rememberTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.SliderState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.unit.Velocity
import kotlin.math.abs
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.ShowTitle
import app.kaeri.design.tabular
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.ffi.FfiCastUpNext
import app.kaeri.engine.ffi.FfiNowCard
import app.kaeri.engine.ffi.FfiTvCommand
import app.kaeri.engine.ffi.FfiTvTrack
import app.kaeri.engine.ffi.FfiTvTrackKind
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.SkipKind
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.CastItem
import app.kaeri.store.CastModel
import app.kaeri.store.CastSessionState
import app.kaeri.store.CastShown
import app.kaeri.store.CastStage
import app.kaeri.store.text

// The morph

/** Where the controller grows from when no mini is there yet: the Play button that started it. */
@Stable
class CastOrigin {
    var rect by mutableStateOf<Rect?>(null)
}

/** The morph's layout and transition, for the mini in the bottom bar and the controller over all. */
@OptIn(ExperimentalSharedTransitionApi::class)
@Stable
internal class CastMorph(val layout: SharedTransitionScope, val transition: Transition<Boolean>, val still: Boolean, val origin: CastOrigin, val drag: CastDrag, private val seeking: () -> Boolean) {
    /** The surface's flight on the standard spring, or, while a back swipe or a drag seeks it, a straight line in time. */
    val bounds = BoundsTransform { _, _ ->
        if (seeking()) tween(300, easing = LinearEasing) else spring(dampingRatio = 1f, stiffness = 440f, visibilityThreshold = Rect.VisibilityThreshold)
    }

    /** The mini is in its slot: it plays, or joins again, and the controller has been closed once since. */
    var miniPresent by mutableStateOf(false)
}

internal val LocalCastMorph = staticCompositionLocalOf<CastMorph?> { null }

/**
 * The drawer's drag: up from the mini opens the controller and down on the controller closes it,
 * the surface following the finger through the morph. A flick settles the way it went, a slower
 * release on the nearer end, quicker the faster the finger went. A drag caught mid-flight picks
 * the surface up where it is. With animations removed nothing follows the finger: a drag past a
 * fifth of the way, or a flick, opens or closes it as a tap does.
 */
@Stable
internal class CastDrag(
    private val state: SeekableTransitionState<Boolean>,
    private val scope: CoroutineScope,
    private val cast: CastModel,
    private val still: Boolean,
    /** The speed in px/s past which a release is a flick. */
    private val flick: Float,
) {
    /** From the mini's top to the window's top, in px: how far a whole open travels. */
    var travel by mutableFloatStateOf(0f)

    /** The window's height, in px: the travel of a controller that opened with no mini under it. */
    var height by mutableFloatStateOf(0f)

    /** A drag, or the settle after it, holds the morph. */
    var active by mutableStateOf(false)
        private set

    private var dragging = false
    private var openness = 0f
    private var from = 0f
    private var follow: Job? = null
    private var settle: Job? = null
    private val wanted = Channel<Float>(Channel.CONFLATED)

    private fun span(): Float = if (travel > 0f) travel else height

    /** How open the morph is now, mid-flight included. */
    private fun opennessNow(): Float = when {
        state.currentState == state.targetState -> if (state.currentState) 1f else 0f
        state.targetState -> state.fraction
        else -> 1f - state.fraction
    }

    fun start() {
        settle?.cancel()
        settle = null
        follow?.cancel()
        active = true
        dragging = true
        openness = opennessNow()
        from = openness
        if (still) return
        follow = scope.launch {
            for (open in wanted) {
                if (state.currentState) state.seekTo(1f - open, targetState = false) else state.seekTo(open, targetState = true)
            }
        }
    }

    /** The finger moved [dy] px, down positive. */
    fun drag(dy: Float) {
        if (!dragging) start()
        val span = span()
        if (span <= 0f) return
        openness = (openness - dy / span).coerceIn(0f, 1f)
        if (!still) wanted.trySend(openness)
    }

    /** The finger let go at [velocity] px/s, down positive. */
    fun release(velocity: Float) {
        if (!dragging) return
        dragging = false
        follow?.cancel()
        follow = null
        val open = when {
            abs(velocity) >= flick -> velocity < 0
            still -> if (from >= 0.5f) openness > from - STILL_PAST else openness > from + STILL_PAST
            else -> openness > 0.5f
        }
        val left = if (open) 1f - openness else openness
        val millis = (left * span() / maxOf(abs(velocity), flick) * 1000f).toInt().coerceIn(SETTLE_SHORTEST, SETTLE_LONGEST)
        settle = scope.launch {
            try {
                if (open) cast.openCard() else cast.closeCard()
                if (still) state.animateTo(open) else state.animateTo(open, tween(millis, easing = LinearOutSlowInEasing))
            } finally {
                active = dragging
            }
        }
    }

    /**
     * The controller's scroll hands its leftover to the drag: down at the top of the list closes
     * it, and back up before the list scrolls again. A fling ends the drag.
     */
    val connection = object : NestedScrollConnection {
        override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
            if (!dragging || source != NestedScrollSource.UserInput || available.y >= 0f || openness >= 1f) return Offset.Zero
            drag(available.y)
            return Offset(0f, available.y)
        }

        override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset {
            if (source != NestedScrollSource.UserInput || available.y <= 0f) return Offset.Zero
            drag(available.y)
            return Offset(0f, available.y)
        }

        override suspend fun onPreFling(available: Velocity): Velocity {
            if (!dragging) return Velocity.Zero
            release(available.y)
            return available
        }
    }

    private companion object {
        /** With animations removed, how far of the way a slow drag goes before it counts. */
        const val STILL_PAST = 0.2f
        const val SETTLE_SHORTEST = 120
        const val SETTLE_LONGEST = 320
    }
}

/**
 * Vertical drags that start here stay here: a slider takes its own horizontal drag, and a drag up
 * or down on it neither scrolls the controller nor closes it.
 */
private fun Modifier.keepsVerticalDrags(): Modifier = pointerInput(Unit) {
    detectVerticalDragGestures { change, _ -> change.consume() }
}

/** Where a Play that goes to the TV started, for the controller to grow from. `null` outside [CastMorphHost]. */
val LocalCastOrigin = staticCompositionLocalOf<CastOrigin?> { null }

/** A release faster than this is a flick: Material's sheets' threshold. */
private val FLICK = 125.dp

/** The key both ends of the morph's surface share. */
private const val SURFACE = "cast-surface" // i18n-skip

/** The key of the cover, 40 square in the mini and 16:9 in the controller. */
private const val COVER = "cast-cover" // i18n-skip

/** How long the controller's status line stays blank as an episode starts. */
private const val QUIET_FOR = 600L

/**
 * The app's frame with the cast controller over it. [content] places [CastMiniSlot] in its bottom
 * bar; the controller fills the window over everything while it is open, unless [hidden].
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun CastMorphHost(hidden: Boolean, content: @Composable () -> Unit) {
    val cast = LocalAppStore.current.cast
    val open by cast.open.collectAsStateWithLifecycle()
    val state = remember { SeekableTransitionState(false) }
    val transition = rememberTransition(state, label = "cast controller") // i18n-skip
    val still = app.kaeri.design.rememberAnimationsRemoved()
    val origin = remember { CastOrigin() }
    var seeking by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val flick = with(LocalDensity.current) { FLICK.toPx() }
    val drag = remember(state, scope, cast, still, flick) { CastDrag(state, scope, cast, still, flick) }
    LaunchedEffect(open) {
        // A drag's release settles the morph itself, at the finger's speed.
        if (!drag.active && (state.targetState != open || state.currentState != open)) state.animateTo(open)
    }
    // A Play button's place serves the one open it started.
    LaunchedEffect(transition.currentState) { if (transition.currentState) origin.rect = null }
    val shown by cast.shown.collectAsStateWithLifecycle()
    val session by cast.session.collectAsStateWithLifecycle()
    val joining by cast.joining.collectAsStateWithLifecycle()
    val last by cast.lastItem.collectAsStateWithLifecycle()
    val stage = shown?.stage
    val has = stage == CastStage.Starting || stage == CastStage.Playing || session.state == CastSessionState.Playing || (joining != null && last != null)
    SharedTransitionLayout {
        val morph = remember(this, transition, still, drag) { CastMorph(this, transition, still, origin, drag) { seeking || drag.active } }
        // A controller that opened from a Play button leaves the mini's slot empty until it closes.
        LaunchedEffect(morph, has, open) {
            if (!has) morph.miniPresent = false else if (!open) morph.miniPresent = true
        }
        CompositionLocalProvider(LocalCastMorph provides morph, LocalCastOrigin provides origin) {
            Box(Modifier.fillMaxSize().onGloballyPositioned { drag.height = it.size.height.toFloat() }) {
                content()
                if (!hidden) CastExpandedLayer(morph)
                // A notice while the remote is up sits over the remote's foot, not under the remote.
                if (!hidden && open) {
                    SnackbarHost(
                        LocalSnackbars.current.host,
                        Modifier.align(Alignment.BottomCenter).windowInsetsPadding(WindowInsets.navigationBars),
                    ) { NoticeSnackbar(it) }
                }
            }
        }
    }
    // A back swipe seeks the controller back into the mini, as far as the finger goes.
    PredictiveBackHandler(enabled = open && !hidden) { progress ->
        try {
            seeking = true
            progress.collect { event -> state.seekTo(event.progress, targetState = false) }
            seeking = false
            cast.closeCard()
            scope.launch { state.animateTo(false) }
        } catch (e: CancellationException) {
            seeking = false
            scope.launch { state.animateTo(true) }
            throw e
        }
    }
}

/**
 * The mini's slot over the navigation bar: 66 high while something plays, whether the mini
 * shows or has grown into the controller, so nothing under it moves as it morphs.
 */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
fun CastMiniSlot() {
    val morph = LocalCastMorph.current ?: return
    val cast = LocalAppStore.current.cast
    AnimatedVisibility(
        morph.miniPresent,
        enter = expandVertically(Motion.standard(), expandFrom = Alignment.Top) + fadeIn(Motion.fadeIn()),
        exit = shrinkVertically(Motion.standard(), shrinkTowards = Alignment.Top) + fadeOut(Motion.fadeOut()),
    ) {
        val shown by cast.shown.collectAsStateWithLifecycle()
        val reconnecting = castReconnecting(cast)
        val drag = morph.drag
        Box(
            Modifier
                .fillMaxWidth()
                .height(66.dp)
                .onGloballyPositioned { drag.travel = it.boundsInWindow().top }
                // Up from the mini opens the controller, following the finger, as a tap does at once.
                .draggable(
                    rememberDraggableState { drag.drag(it) },
                    Orientation.Vertical,
                    enabled = shown != null && !reconnecting,
                    onDragStarted = { drag.start() },
                    onDragStopped = { drag.release(it) },
                ),
        ) {
            with(morph.layout) {
                morph.transition.AnimatedVisibility(
                    visible = { !it },
                    enter = if (morph.still) fadeIn(Motion.reveal()) else fadeIn(tween(200, delayMillis = 100)),
                    exit = if (morph.still) fadeOut(Motion.reveal()) else fadeOut(tween(100)),
                ) {
                    Box(
                        Modifier
                            .fillMaxSize()
                            .then(if (morph.still) Modifier else Modifier.sharedBounds(rememberSharedContentState(SURFACE), this, fadeIn(tween(1)), fadeOut(tween(300, delayMillis = 150)), morph.bounds, clipInOverlayDuringTransition = OverlayClip(RectangleShape)))
                            .background(MaterialTheme.colorScheme.surfaceContainerHigh),
                    ) {
                        CastMini(cast, this@AnimatedVisibility, morph)
                    }
                }
            }
        }
    }
}

/** The controller over the whole window while it is open, grown from the mini or the Play button. */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
private fun CastExpandedLayer(morph: CastMorph) {
    val density = LocalDensity.current
    with(morph.layout) {
        // With no mini yet, the surface grows from the Play button that started the cast.
        if (!morph.miniPresent && !morph.still) {
            morph.transition.AnimatedVisibility(visible = { !it }, enter = EnterTransition.None, exit = ExitTransition.None) {
                val rect = morph.origin.rect
                Box(Modifier.fillMaxSize()) {
                    val anchor = if (rect != null) {
                        with(density) {
                            Modifier
                                .offset { IntOffset(rect.left.toInt(), rect.top.toInt()) }
                                .size(rect.width.toDp(), rect.height.toDp())
                        }
                    } else {
                        Modifier.align(Alignment.BottomCenter).windowInsetsPadding(WindowInsets.navigationBars).padding(bottom = 80.dp).fillMaxWidth().height(66.dp)
                    }
                    Box(anchor.sharedBounds(rememberSharedContentState(SURFACE), this@AnimatedVisibility, fadeIn(tween(1)), fadeOut(tween(1)), morph.bounds))
                }
            }
        }
        morph.transition.AnimatedVisibility(
            visible = { it },
            enter = if (morph.still) fadeIn(Motion.reveal()) else EnterTransition.None,
            exit = if (morph.still) fadeOut(Motion.reveal()) else ExitTransition.None,
        ) {
            val scope = this
            Box(
                Modifier
                    .fillMaxSize()
                    .then(
                        if (morph.still) {
                            Modifier
                        } else {
                            Modifier.sharedBounds(
                                rememberSharedContentState(SURFACE),
                                scope,
                                enter = fadeIn(tween(150)),
                                exit = fadeOut(tween(300, delayMillis = 150)),
                                boundsTransform = morph.bounds,
                                resizeMode = SharedTransitionScope.ResizeMode.RemeasureToBounds,
                                clipInOverlayDuringTransition = OverlayClip(RectangleShape),
                            )
                        },
                    )
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                    // Down on the controller, with its list at the top, closes it into the mini.
                    .nestedScroll(morph.drag.connection),
            ) {
                Box(
                    Modifier
                        .fillMaxSize()
                        .then(if (morph.still) Modifier else Modifier.skipToLookaheadSize())
                        .animateEnterExit(
                            enter = if (morph.still) EnterTransition.None else fadeIn(tween(200, delayMillis = 100)),
                            exit = if (morph.still) ExitTransition.None else fadeOut(tween(100)),
                        ),
                ) {
                    CastControllerContent(scope, morph)
                }
            }
        }
    }
}

/** The cover, which flies between the mini and the controller with its radius going from 8 to 16. */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
private fun Modifier.castCover(scope: AnimatedVisibilityScope, morph: CastMorph): Modifier {
    val radius by morph.transition.animateDp(label = "cover radius") { if (it) 16.dp else 8.dp } // i18n-skip
    val shaped = if (morph.still) this else with(morph.layout) {
        this@castCover.sharedElement(rememberSharedContentState(COVER), scope, boundsTransform = morph.bounds)
    }
    return shaped.clip(RoundedCornerShape(radius))
}

/** The show's art: the banner, else the still the engine sent, else its cover. */
@Composable
internal fun CastArt(id: AnilistId?, image: String?, modifier: Modifier = Modifier) {
    val entry = id?.let { LocalAppStore.current.titles[it] }
    val url = entry?.bannerUrl ?: image ?: entry?.coverUrl
    Box(modifier.background(ObiSilk.RaisedSelected)) {
        ArtworkImage(url, plate = null, maxPixelSize = pixels(364.dp), modifier = Modifier.fillMaxSize())
    }
}

// The mini

/** What the mini and the controller read: the engine's card, else the session's item while it joins again. */
private data class CastNow(
    val id: AnilistId?,
    val title: String,
    val episodeLine: String,
    val image: String?,
    val position: Double,
    val duration: Double?,
    val paused: Boolean,
    val starting: Boolean,
)

@Composable
private fun castNow(cast: CastModel): CastNow? {
    val shown by cast.shown.collectAsStateWithLifecycle()
    val session by cast.session.collectAsStateWithLifecycle()
    val last by cast.lastItem.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val held = shown
    val item = session.item ?: last
    if (held != null) {
        val now = CastModel.now(held)
        return CastNow(
            id = now.id,
            title = now.title,
            episodeLine = now.episodeLine.ifEmpty { Strings.castEpisode(context, now.episode) },
            image = item?.image,
            position = now.position,
            duration = now.duration,
            paused = now.paused,
            starting = held.stage == CastStage.Starting,
        )
    }
    item ?: return null
    return CastNow(item.id, item.title, castEpisodeLine(context, item), item.image, item.position, item.duration, item.paused, item.loading)
}

/**
 * The TV's connection is made again, or a session left running is joined: the mini and the remote
 * read Reconnecting, and the mini takes no tap or drag.
 */
@Composable
private fun castReconnecting(cast: CastModel): Boolean {
    val shown by cast.shown.collectAsStateWithLifecycle()
    val session by cast.session.collectAsStateWithLifecycle()
    val joining by cast.joining.collectAsStateWithLifecycle()
    val held = shown
    return joining != null || held?.reconnecting == true || ((held == null || held.live) && session.reconnecting)
}

/** Whether [waits] has held for the 600 ms a loading state stays quiet: a short wait says nothing. */
@Composable
private fun rememberQuietWait(waits: Boolean): Boolean {
    var past by remember { mutableStateOf(false) }
    LaunchedEffect(waits) {
        past = false
        if (waits) {
            delay(QUIET_FOR)
            past = true
        }
    }
    return waits && past
}

/** What the mini's second line says: the TV's name beside the glyph, or a wait beside a spinner. */
private enum class MiniLine { Name, Reconnecting, Waiting }

/**
 * The mini: a 2 dp playhead along its top, the cover, the title over the TV's name, pause or play,
 * and the filled cast glyph for the TV's dialog. A tap anywhere else opens the controller. While
 * the TV reconnects or waits, the line says so beside a spinner and play or pause dims.
 */
@Composable
private fun CastMini(cast: CastModel, scope: AnimatedVisibilityScope, morph: CastMorph) {
    val now = castNow(cast) ?: return
    val shown by cast.shown.collectAsStateWithLifecycle()
    val session by cast.session.collectAsStateWithLifecycle()
    val joining by cast.joining.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val name = joining ?: session.deviceName ?: shown?.tvName.orEmpty()
    val reconnecting = castReconnecting(cast)
    val held = shown
    val waits = !reconnecting && (held?.waiting != null || ((held == null || held.stage == CastStage.Playing) && session.item?.waiting != null))
    val waiting = rememberQuietWait(waits)
    val usable = !now.starting && !reconnecting && !waiting
    val line = when {
        reconnecting -> MiniLine.Reconnecting
        waiting -> MiniLine.Waiting
        else -> MiniLine.Name
    }
    val position by produceState(now.position, now, usable) {
        while (true) {
            value = cast.playhead() ?: cast.itemPlayhead() ?: now.position
            if (now.paused || !usable) break
            delay(1000)
        }
    }
    val opens = stringResource(R.string.cast_mini_opens)
    val status = when (line) {
        MiniLine.Reconnecting -> stringResource(R.string.tv_status_reconnecting, name)
        MiniLine.Waiting -> stringResource(R.string.tv_waiting_for, name)
        MiniLine.Name -> stringResource(if (now.paused) R.string.cast_status_paused else R.string.tv_status_playing, name)
    }
    val dim by animateFloatAsState(if (usable) 1f else 0.38f, Motion.quick(), label = "mini dim") // i18n-skip
    Column(
        Modifier
            .fillMaxSize()
            .clickable(onClickLabel = opens, enabled = !reconnecting) { cast.openCard() }
            .semantics(mergeDescendants = false) { contentDescription = "${now.title}, $status" }, // i18n-skip
    ) {
        val duration = now.duration?.takeIf { it > 0 }
        Box(Modifier.fillMaxWidth().height(2.dp).background(ObiSilk.Edge)) {
            if (duration != null && !now.starting) {
                Box(Modifier.fillMaxWidth((position / duration).toFloat().coerceIn(0f, 1f)).height(2.dp).background(colors.onSurface))
            }
        }
        Row(
            Modifier.fillMaxSize().padding(start = 16.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            CastArt(now.id, now.image, Modifier.size(40.dp).castCover(scope, morph))
            Column(Modifier.weight(1f)) {
                Text(now.title, style = ShowTitle.caption, color = colors.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
                // The glyph and the name crossfade with the spinner and the wait's words.
                AnimatedContent(
                    line,
                    transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
                    contentAlignment = Alignment.CenterStart,
                    label = "mini line", // i18n-skip
                ) { kind ->
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        if (kind == MiniLine.Name) {
                            CastGlyph(CastGlyphState.Filled, size = 16.dp, tint = colors.onSurfaceVariant)
                        } else {
                            CircularProgressIndicator(Modifier.size(12.dp), strokeWidth = 1.5.dp, color = colors.onSurfaceVariant)
                        }
                        Text(
                            when (kind) {
                                MiniLine.Name -> name
                                MiniLine.Reconnecting -> stringResource(R.string.tv_status_reconnecting, name)
                                MiniLine.Waiting -> stringResource(R.string.tv_waiting_for, name)
                            },
                            style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp),
                            color = colors.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
            Box(Modifier.alpha(dim)) {
                TvGlyph(
                    if (now.paused) Icons.PlayRow else Icons.Pause,
                    stringResource(if (now.paused) R.string.action_play else R.string.action_pause),
                    size = 22.dp,
                    enabled = usable,
                ) { cast.send(FfiTvCommand.TogglePause) }
            }
            IconButton(onClick = cast::pressButton, modifier = Modifier.size(48.dp).semantics { contentDescription = context.getString(R.string.cast_button_connected, name) }) {
                CastGlyph(CastGlyphState.Filled, size = 22.dp, tint = colors.onSurface)
            }
        }
    }
}

// The controller

@Composable
private fun CastControllerContent(scope: AnimatedVisibilityScope, morph: CastMorph) {
    val cast = LocalAppStore.current.cast
    val shown by cast.shown.collectAsStateWithLifecycle()
    val held = shown ?: return
    val navigator = LocalNavigatorOrNull.current
    CastController(cast, held, scope, morph) { id ->
        // The controller closes into the mini, and the title page comes up under it.
        cast.closeCard()
        navigator?.openTitle(id, "cast") // i18n-skip
    }
}

/** What the status line says, and with which mark: a spinner, the water dot or the vermilion dot. */
private enum class CastLineKind { Blank, Starting, Playing, Reconnecting, Waiting, Ended }

private data class CastLine(val kind: CastLineKind, val text: String)

/**
 * The status line: a spinner while it starts, reconnects or waits, water while it plays or
 * pauses, vermilion once it ended. A change of kind crossfades the mark and the words together.
 */
@Composable
private fun CastStatus(line: CastLine) {
    val colors = MaterialTheme.colorScheme
    AnimatedContent(
        line,
        contentKey = { it.kind },
        transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
        contentAlignment = Alignment.Center,
        label = "cast status", // i18n-skip
    ) { shownLine ->
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (shownLine.kind != CastLineKind.Blank) {
                when (shownLine.kind) {
                    CastLineKind.Playing -> StateDot(ObiSilk.Water)
                    CastLineKind.Ended -> StateDot(ObiSilk.Vermilion)
                    else -> CircularProgressIndicator(Modifier.size(12.dp), strokeWidth = 1.5.dp, color = colors.onSurfaceVariant)
                }
                Text(shownLine.text, style = MaterialTheme.typography.bodyMedium.copy(fontSize = 14.sp), color = colors.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun CastController(cast: CastModel, shown: CastShown, scope: AnimatedVisibilityScope, morph: CastMorph, onShow: (AnilistId) -> Unit) {
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val session by cast.session.collectAsStateWithLifecycle()
    val now = CastModel.now(shown)
    val stage = shown.stage
    val playing = stage == CastStage.Playing
    val starting = stage == CastStage.Starting
    val over = stage == CastStage.Failed || stage == CastStage.Lost
    val entry = store.titles[now.id]
    val name = shown.tvName
    var choosingSource by remember { mutableStateOf(false) }
    // The status line stays blank for the first 600 ms of a start, as a loading state does.
    var quiet by remember(shown.startedAt) { mutableStateOf(starting) }
    LaunchedEffect(shown.startedAt) {
        delay((QUIET_FOR - (System.nanoTime() / 1_000_000 - shown.startedAt)).coerceIn(0, QUIET_FOR))
        quiet = false
    }
    // Reconnecting shows at once, as the silence before the close has passed already. A wait,
    // for the stream or while the engine learns why the TV stopped, shows after 600 ms.
    val reconnecting = shown.reconnecting || (shown.live && session.reconnecting)
    val waitingFor = if (playing && !reconnecting) shown.waiting?.toLong() ?: session.item?.waiting else null
    val waiting = rememberQuietWait(waitingFor != null) && waitingFor != null
    // The TV went quiet: everything but Stop casting dims and takes no input, in place.
    val still = reconnecting || waiting
    val status = when {
        reconnecting -> CastLine(CastLineKind.Reconnecting, context.getString(R.string.tv_status_reconnecting, name))
        stage == CastStage.Starting -> if (quiet) CastLine(CastLineKind.Blank, "") else CastLine(CastLineKind.Starting, shown.cast.phaseLine?.text(context) ?: Strings.tvSessionStarting(context, name))
        waiting && waitingFor != null -> CastLine(
            CastLineKind.Waiting,
            if (waitingFor >= CastModel.COUNT_FROM_SECONDS) Msg.CastPhaseWaiting(name, waitingFor).text(context) else context.getString(R.string.tv_waiting_for, name),
        )
        stage == CastStage.Playing -> CastLine(CastLineKind.Playing, if (now.paused) context.getString(R.string.cast_status_paused, name) else Strings.tvStatusPlaying(context, name))
        stage == CastStage.Failed -> CastLine(CastLineKind.Ended, Strings.castCardFailed(context, name))
        else -> CastLine(CastLineKind.Ended, Strings.castCardStopped(context))
    }
    val upNext = shown.cast.upNext
    Column(
        Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.statusBars)
            .windowInsetsPadding(WindowInsets.navigationBars)
            .verticalScroll(rememberScrollState()),
    ) {
        // The header: close, the status line, the TV's dialog.
        Row(Modifier.fillMaxWidth().height(56.dp).padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            TvGlyph(Icons.Down, stringResource(R.string.cast_controller_close)) { cast.closeCard() }
            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) { CastStatus(status) }
            IconButton(onClick = cast::pressButton, modifier = Modifier.size(48.dp).semantics { contentDescription = context.getString(R.string.cast_button_connected, name) }) {
                CastGlyph(CastGlyphState.Filled, size = 22.dp, tint = colors.onSurface)
            }
        }
        Column(Modifier.padding(horizontal = 24.dp)) {
            Spacer(Modifier.height(8.dp))
            // The art, 16:9, 120 high while Up next shows.
            androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth()) {
                val artHeight by androidx.compose.animation.core.animateDpAsState(
                    if (upNext != null) 120.dp else maxWidth * 9f / 16f,
                    Motion.standard(),
                    label = "art height", // i18n-skip
                )
                CastArt(now.id, session.item?.image, Modifier.fillMaxWidth().height(artHeight).castCover(scope, morph))
            }
            Spacer(Modifier.height(16.dp))
            // The show's name, which opens its title page.
            val openLabel = stringResource(R.string.cast_open_show, now.title)
            Row(
                Modifier
                    .heightIn(min = 48.dp)
                    .clickable(onClickLabel = openLabel) { onShow(now.id) }
                    .semantics(mergeDescendants = true) { contentDescription = openLabel },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(now.title, style = ShowTitle.page, color = colors.onSurface, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                Icon(Icons.Chevron, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(20.dp))
            }
            Text(
                now.episodeLine.ifEmpty { Strings.castEpisode(context, now.episode) },
                style = MaterialTheme.typography.bodyMedium.copy(fontSize = 15.sp),
                color = colors.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            val usable = playing && !still
            if (!over) {
                if (playing) {
                    now.skip?.let { skip ->
                        OutlinedButton(onClick = { cast.send(FfiTvCommand.Skip) }, enabled = usable, modifier = Modifier.padding(top = 8.dp)) { Text(skip.label, color = colors.onSurface) }
                    }
                }
                shown.cast.undo?.let { CastUndoNote(cast, it.kind) }
            }
            // The seek line stays, dimmed with its times, while the TV is quiet and once the cast ended.
            CastSeekLine(cast, now, shown, usable = usable, dimmed = still || over)
            // Once the cast ended, the reason takes the transport's row.
            AnimatedContent(
                over,
                transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
                label = "transport or reason", // i18n-skip
            ) { ended ->
                if (ended) {
                    Box(Modifier.fillMaxWidth().padding(top = 12.dp).heightIn(min = 64.dp), contentAlignment = Alignment.CenterStart) {
                        shown.cast.notice?.let { ProblemLine(it.text(context)) }
                    }
                } else {
                    CastTransport(cast, now, usable = usable)
                }
            }
            if (!over) {
                AnimatedVisibility(upNext != null, enter = expandVertically(Motion.standard()) + fadeIn(Motion.fadeIn()), exit = shrinkVertically(Motion.standard()) + fadeOut(Motion.fadeOut())) {
                    val last = remember { LastUpNext() }
                    upNext?.let { last.value = it }
                    last.value?.let { CastUpNext(cast, it, now.id) }
                }
            }
            Spacer(Modifier.height(8.dp))
            // The TV's tracks and volume, dimmed while it starts, while it is quiet, or once it ended.
            CastTrackRow(stringResource(R.string.player_subtitles), FfiTvTrackKind.SUBTITLE, now.subtitles, now.subtitlesOff, cast, enabled = usable)
            TvDivider()
            // The engine lists audio only when there is a choice: one track has no row, as on iOS.
            if (now.audio.isNotEmpty()) {
                CastTrackRow(stringResource(R.string.player_audio), FfiTvTrackKind.AUDIO, now.audio, off = false, cast, enabled = usable)
                TvDivider()
            }
            CastVolumeRow(cast, shown.level, shown.cast.muted == true || session.muted == true, now.volume, enabled = usable)
            // The footer: what each state offers in two places that keep their order, then More.
            val notice = shown.cast.notice
            val playHere = stringResource(R.string.cast_play_here)
            val stop = stringResource(R.string.cast_stop)
            val another = stringResource(R.string.tv_choose_another)
            val actions = when {
                // Play here waits for Casting stopped while the TV reconnects; Stop casting stays.
                notice == null -> listOf(CastAction(playHere, primary = false, enabled = !reconnecting, cast::playHere), CastAction(stop, primary = false, enabled = true, cast::disconnect))
                notice is Msg.NoticeCastNeverFetches -> listOf(
                    CastAction(stringResource(R.string.action_try_again), primary = true, enabled = true, cast::tryAgain),
                    CastAction(playHere, primary = false, enabled = true, cast::playHere),
                )
                notice is Msg.NoticeCastNotSameNetwork || notice is Msg.NoticeCastLost || notice is Msg.NoticeCastNoDevice || notice is Msg.NoticeCastStoppedElsewhere -> listOf(
                    CastAction(playHere, primary = true, enabled = true, cast::playHere),
                    CastAction(another, primary = false, enabled = true, cast::chooseAnother),
                )
                else -> listOfNotNull(
                    entry?.let { CastAction(stringResource(R.string.player_choose_source), primary = true, enabled = true) { choosingSource = true } },
                    CastAction(playHere, primary = false, enabled = true, cast::playHere).takeIf { notice !is Msg.NoticeCastDeepLink },
                )
            }
            val reduced = app.kaeri.design.rememberAnimationsRemoved()
            Row(Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                FlowRow(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    actions.forEachIndexed { place, action -> key(place) { CastFooterButton(action, reduced) } }
                }
                var more by remember { mutableStateOf(false) }
                Box {
                    IconButton(onClick = { more = true }) {
                        Icon(Icons.More, contentDescription = stringResource(R.string.action_more), tint = colors.onSurface)
                    }
                    Menu(expanded = more, onDismissRequest = { more = false }) {
                        DropdownMenuItem(
                            text = { Text(another, style = MaterialTheme.typography.labelLarge, modifier = Modifier.offWith(!reconnecting)) },
                            enabled = !reconnecting,
                            modifier = Modifier.heightIn(min = 48.dp),
                            onClick = {
                                more = false
                                cast.chooseAnother()
                            },
                        )
                        if (notice == null) {
                            // Disconnect while the held TV plays nothing, Stop casting while something plays.
                            val idle = session.held && session.state == CastSessionState.Connected
                            DropdownMenuItem(
                                text = { Text(stringResource(if (idle) R.string.action_disconnect else R.string.cast_stop), style = MaterialTheme.typography.labelLarge) },
                                modifier = Modifier.heightIn(min = 48.dp),
                                onClick = {
                                    more = false
                                    cast.disconnect()
                                },
                            )
                        }
                    }
                }
            }
            // A notice over the remote's foot leaves room to scroll the footer clear of it.
            if (LocalSnackbars.current.host.currentSnackbarData != null) Spacer(Modifier.height(80.dp))
        }
    }
    if (choosingSource && entry != null) SourcePickerSheet(entry) { choosingSource = false }
}

/** The Up next card the controller last showed, kept out of the snapshot for it to leave with. */
private class LastUpNext {
    var value: FfiCastUpNext? = null
}

/** A footer action: amber when it is the view's primary one, else an outlined pill in silk. */
internal data class CastAction(val text: String, val primary: Boolean, val enabled: Boolean, val onClick: () -> Unit)

/**
 * A control's words are off while the control is. Compose lists them as a node of their own
 * under the control's, and without this one that reads the screen finds them enabled while the
 * control they name is dimmed and off.
 */
internal fun Modifier.offWith(enabled: Boolean): Modifier = if (enabled) this else semantics { disabled() }

/**
 * One of the footer's two places. As the state changes, its fill crossfades between outlined and
 * amber, its words crossfade, and its width follows on standard, or at once with motion reduced.
 */
@Composable
internal fun CastFooterButton(action: CastAction, reduced: Boolean) {
    val colors = MaterialTheme.colorScheme
    val container by animateColorAsState(if (action.primary) colors.primary else Color.Transparent, Motion.fadeIn(), label = "footer fill") // i18n-skip
    val content by animateColorAsState(if (action.primary) colors.onPrimary else colors.onSurface, Motion.fadeIn(), label = "footer words") // i18n-skip
    OutlinedButton(
        onClick = action.onClick,
        enabled = action.enabled,
        colors = ButtonDefaults.outlinedButtonColors(containerColor = container, contentColor = content),
        border = if (action.primary) null else ButtonDefaults.outlinedButtonBorder(action.enabled),
    ) {
        AnimatedContent(
            action.text,
            transitionSpec = {
                (fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut())) using SizeTransform(clip = false) { _, _ -> if (reduced) snap() else Motion.standard() }
            },
            label = "footer action", // i18n-skip
        ) { Text(it, Modifier.offWith(action.enabled)) }
    }
}

/** After an automatic skip: what it did, and Undo. */
@Composable
private fun CastUndoNote(cast: CastModel, kind: SkipKind) {
    val context = LocalContext.current
    Row(Modifier.fillMaxWidth().padding(top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(
            when (kind) {
                SkipKind.OPENING -> Strings.castCardSkippedOpening(context)
                SkipKind.ENDING -> Strings.castCardSkippedEnding(context)
                SkipKind.RECAP -> Strings.playerSkippedRecap(context)
            },
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
        QuietButton(stringResource(R.string.cast_card_undo)) { cast.undoSkip() }
    }
}

/**
 * The seek bar: a 16 dp silk track up to a 4 by 44 thumb, a 6 dp gap each side of it, and a
 * 4 dp track on after it; the position and the time left in tabular figures under it. While the
 * episode starts there is no thumb and no times. Dragged, it seeks on release and holds there
 * until a card with a higher `rev` arrives: the engine holds the asked value meanwhile.
 * [dimmed] while the TV is quiet or the cast ended: 38%, with its times, taking no input. When the
 * TV answers again, the playhead moves on standard to where the TV is now.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CastSeekLine(cast: CastModel, now: FfiNowCard, shown: CastShown, usable: Boolean, dimmed: Boolean) {
    val colors = MaterialTheme.colorScheme
    val playing = shown.stage == CastStage.Playing
    val duration = now.duration?.takeIf { it > 0 } ?: 0.0
    val ticks = playing && !shown.quiet
    val position by produceState(cast.playhead() ?: now.position, now, ticks, shown.frozen) {
        while (true) {
            value = cast.playhead() ?: now.position
            if (now.paused || !ticks) break
            delay(1000)
        }
    }
    var dragging by remember(now.id, now.episode) { mutableStateOf<Float?>(null) }
    var sent by remember(now.id, now.episode) { mutableStateOf<Pair<Float, ULong>?>(null) }
    val rev = shown.cast.card.rev
    // Let go once the engine has drawn the cast again since the seek was sent.
    sent?.let { (_, at) -> if (rev > at) sent = null }
    val target = ((dragging ?: sent?.first)?.toDouble() ?: position).coerceIn(0.0, maxOf(duration, 0.0))
    // The playhead glides to the TV's position once after it was quiet, and steps every other time.
    val reduced = app.kaeri.design.rememberAnimationsRemoved()
    val head = remember(now.id, now.episode) { Animatable(target.toFloat()) }
    val rejoin = remember(now.id, now.episode) { CastRejoin(shown.quiet) }
    if (rejoin.quiet && !shown.quiet && shown.live) rejoin.glide = true
    rejoin.quiet = shown.quiet
    LaunchedEffect(target) {
        if (rejoin.glide && !reduced && dragging == null) {
            rejoin.glide = false
            head.animateTo(target.toFloat(), Motion.standard())
        } else {
            rejoin.glide = false
            head.snapTo(target.toFloat())
        }
    }
    val at = if (dragging != null) target else head.value.toDouble()
    val times = (playing || shown.played) && duration > 0
    val dim by animateFloatAsState(if (dimmed) 0.38f else 1f, Motion.quick(), label = "seek dim") // i18n-skip
    val label = stringResource(R.string.tv_sheet_position)
    val state = stringResource(R.string.tv_sheet_position_state, TvWords.clock(at), TvWords.clock(duration))
    val time = MaterialTheme.typography.labelMedium.tabular()
    Column(Modifier.padding(top = 12.dp).alpha(dim)) {
        val range = 0f..maxOf(duration.toFloat(), 1f)
        val slider = remember(range) { SliderState(at.toFloat(), 0, range) }
        val seekable = usable && duration > 0
        if (dragging == null && slider.value != at.toFloat()) slider.value = at.toFloat()
        Slider(
            state = slider,
            onValueChange = {
                slider.value = it
                dragging = it
            },
            onValueChangeFinished = {
                dragging?.let {
                    cast.send(FfiTvCommand.SeekTo(it.toDouble()))
                    sent = it to rev
                }
                dragging = null
            },
            enabled = seekable,
            thumb = {
                if (times) Box(Modifier.size(4.dp, 44.dp).background(colors.onSurface, RoundedCornerShape(2.dp)))
            },
            track = { CastSeekTrack(it.coercedValueAsFraction, thumb = times) },
            // The slider's semantics are set here whole, with nothing under them: with nodes under
            // it, the slider hands a screen reader its name on a node of its own, and that one
            // reads as on while the slider is off.
            modifier = Modifier.fillMaxWidth().height(44.dp).keepsVerticalDrags().clearAndSetSemantics {
                contentDescription = label
                if (times) stateDescription = state
                progressBarRangeInfo = ProgressBarRangeInfo(slider.value, range)
                if (seekable) {
                    setProgress { asked ->
                        val to = asked.coerceIn(range)
                        if (to == slider.value) return@setProgress false
                        slider.value = to
                        cast.send(FfiTvCommand.SeekTo(to.toDouble()))
                        sent = to to rev
                        true
                    }
                } else {
                    disabled()
                }
            },
        )
        Box(Modifier.fillMaxWidth().heightIn(min = 20.dp)) {
            if (times) {
                Text(TvWords.clock(at), style = time, color = colors.onSurfaceVariant, modifier = Modifier.align(Alignment.CenterStart))
                Text("−" + TvWords.clock(duration - at), style = time, color = colors.onSurfaceVariant, modifier = Modifier.align(Alignment.CenterEnd)) // i18n-skip
            }
        }
    }
}

/** Whether the TV was quiet at the last look, and whether its next position glides there. */
private class CastRejoin(var quiet: Boolean) {
    var glide = false
}

/** The seek bar's track: silk 16 high up to the thumb's gap, 4 high in RaisedSelected after it. */
@Composable
private fun CastSeekTrack(fraction: Float, thumb: Boolean) {
    val silk = MaterialTheme.colorScheme.onSurface
    Canvas(Modifier.fillMaxWidth().height(16.dp)) {
        val gap = if (thumb) 6.dp.toPx() + 2.dp.toPx() else 0f
        val x = size.width * fraction
        val end = (x - gap).coerceAtLeast(0f)
        val start = if (thumb) (x + gap).coerceAtMost(size.width) else x
        val thin = 4.dp.toPx()
        drawRoundRect(ObiSilk.RaisedSelected, topLeft = Offset(start, (size.height - thin) / 2), size = Size(size.width - start, thin), cornerRadius = CornerRadius(thin / 2))
        if (end > 0f && thumb) {
            val path = Path().apply {
                addRoundRect(
                    RoundRect(
                        Rect(0f, 0f, end, size.height),
                        topLeft = CornerRadius(8.dp.toPx()),
                        bottomLeft = CornerRadius(8.dp.toPx()),
                        topRight = CornerRadius(2.dp.toPx()),
                        bottomRight = CornerRadius(2.dp.toPx()),
                    ),
                )
            }
            drawPath(path, silk)
        }
    }
}

/** Back 10, pause in its 64 tonal square, forward 10 and next, 18 apart; at 0.38 while the TV starts or is quiet. */
@Composable
private fun CastTransport(cast: CastModel, now: FfiNowCard, usable: Boolean) {
    val dim by animateFloatAsState(if (usable) 1f else 0.38f, Motion.quick(), label = "transport dim") // i18n-skip
    Row(
        Modifier.fillMaxWidth().padding(top = 12.dp).alpha(dim),
        horizontalArrangement = Arrangement.spacedBy(18.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TvStep(Icons.Replay, stringResource(R.string.player_back_10), enabled = usable) { cast.send(FfiTvCommand.Seek(-10.0)) }
        val label = stringResource(if (now.paused) R.string.action_play else R.string.action_pause)
        FilledTonalIconButton(
            onClick = { cast.send(FfiTvCommand.TogglePause) },
            enabled = usable,
            shape = RoundedCornerShape(16.dp),
            modifier = Modifier.size(64.dp).semantics { contentDescription = label },
        ) {
            Icon(if (now.paused) Icons.PlayRow else Icons.Pause, contentDescription = null, modifier = Modifier.size(32.dp))
        }
        TvStep(Icons.Ahead, stringResource(R.string.player_forward_10), enabled = usable) { cast.send(FfiTvCommand.Seek(10.0)) }
        TvGlyph(Icons.SkipNext, stringResource(R.string.player_next_episode), enabled = usable) { cast.send(FfiTvCommand.Next) }
    }
}

/** Up next under the transport: its still, "Up next · plays in 8 s" in water over the episode, then Cancel and Play now. */
@Composable
private fun CastUpNext(cast: CastModel, next: FfiCastUpNext, id: AnilistId) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val left = next.secondsLeft?.toInt()
    Column(Modifier.fillMaxWidth().padding(top = 20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            CastArt(id, next.still, Modifier.size(96.dp, 54.dp).clip(RoundedCornerShape(8.dp)))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    if (left != null) Strings.castCardUpNextIn(context, left) else Strings.castCardUpNext(context),
                    style = MaterialTheme.typography.labelLarge,
                    color = ObiSilk.Water,
                )
                Text(
                    next.title?.let { Strings.castEpisodeTitled(context, next.number, it) } ?: Strings.castEpisode(context, next.number),
                    style = ShowTitle.row,
                    color = colors.onSurface,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
            OutlinedButton(onClick = cast::cancelNext) { Text(stringResource(R.string.cast_card_up_next_cancel), color = colors.onSurface) }
            Button(onClick = cast::playNext) { Text(stringResource(R.string.cast_card_up_next_play)) }
        }
    }
}

/**
 * The subtitle or audio row: its name, the track on and ⇕ at the end, a menu of the tracks. With
 * no tracks to choose it says None, with no ⇕, and takes no tap.
 */
@Composable
internal fun CastTrackRow(title: String, kind: FfiTvTrackKind, tracks: List<FfiTvTrack>, off: Boolean, cast: CastModel, enabled: Boolean, startOpen: Boolean = false) {
    val choosable = tracks.isNotEmpty()
    var open by remember { mutableStateOf(startOpen && choosable) }
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val value = if (choosable) TvWords.chosen(context, tracks, off) else Strings.valueNone(context)
    val dim by animateFloatAsState(if (enabled) 1f else 0.38f, Motion.quick(), label = "track dim") // i18n-skip
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 56.dp)
                .then(if (choosable) Modifier.clickable(enabled = enabled, role = Role.DropdownList) { open = true } else Modifier)
                .alpha(dim)
                .semantics(mergeDescendants = true) {},
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = colors.onSurface, modifier = Modifier.weight(1f).offWith(enabled))
            Text(value, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 200.dp).offWith(enabled))
            if (choosable) Icon(Icons.DropDown, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(18.dp))
        }
        Menu(expanded = open, onDismissRequest = { open = false }) {
            val choices = buildList {
                if (kind == FfiTvTrackKind.SUBTITLE) add(null to Strings.valueOff(context))
                tracks.forEach { add(it to TvWords.track(context, it)) }
            }
            val selected = stringResource(R.string.value_selected)
            for ((track, label) in choices) {
                val picked = if (track == null) off else !off && track.selected
                DropdownMenuItem(
                    text = { Text(label, style = MaterialTheme.typography.labelLarge) },
                    leadingIcon = {
                        Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                            if (picked) Icon(Icons.Check, contentDescription = null, tint = colors.onSurface)
                        }
                    },
                    modifier = Modifier.heightIn(min = 48.dp).semantics { if (picked) stateDescription = selected },
                    onClick = {
                        open = false
                        cast.send(FfiTvCommand.Track(kind, track?.id))
                    },
                )
            }
        }
    }
}

/**
 * TV volume: "Set on the TV" with the outlined Mute pill for a TV whose volume is fixed, or a
 * slider for one that takes a level, sending the difference on release.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CastVolumeRow(cast: CastModel, level: Boolean, muted: Boolean, volume: Double?, enabled: Boolean) {
    val colors = MaterialTheme.colorScheme
    val label = stringResource(R.string.tv_sheet_volume)
    val muteLabel = stringResource(if (muted) R.string.cast_card_unmute else R.string.cast_card_mute)
    val dim by animateFloatAsState(if (enabled) 1f else 0.38f, Motion.quick(), label = "volume dim") // i18n-skip
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).alpha(dim),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = colors.onSurface, modifier = Modifier.offWith(enabled))
        if (!level) {
            Spacer(Modifier.weight(1f))
            Text(stringResource(R.string.cast_volume_fixed), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, modifier = Modifier.offWith(enabled))
            // Off while the row is dimmed; the row's dim alone draws it, so its outline stays.
            OutlinedButton(
                onClick = cast::toggleMute,
                enabled = enabled,
                border = ButtonDefaults.outlinedButtonBorder(enabled = true),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 14.dp),
                modifier = Modifier.heightIn(min = 40.dp),
            ) {
                Icon(if (muted) Icons.SoundOff else Icons.VolumeDown, contentDescription = null, tint = colors.onSurface, modifier = Modifier.size(16.dp))
                Spacer(Modifier.width(6.dp))
                Text(muteLabel, color = colors.onSurface, modifier = Modifier.offWith(enabled))
            }
            return@Row
        }
        var dragging by remember { mutableStateOf<Float?>(null) }
        val at = dragging ?: (volume ?: 0.0).toFloat()
        val volumeWords = Strings.tvSheetVolumeValue(LocalContext.current, at.toInt())
        TvGlyph(if (muted) Icons.SoundOff else Icons.VolumeDown, muteLabel, enabled = enabled, onClick = cast::toggleMute)
        Slider(
            value = at,
            valueRange = 0f..100f,
            onValueChange = { dragging = it },
            onValueChangeFinished = {
                dragging?.let { cast.setVolume(it.toDouble()) }
                dragging = null
            },
            enabled = enabled,
            colors = SliderDefaults.colors(thumbColor = colors.onSurface, activeTrackColor = colors.onSurface, inactiveTrackColor = ObiSilk.RaisedSelected),
            modifier = Modifier.weight(1f).keepsVerticalDrags().semantics {
                contentDescription = label
                stateDescription = volumeWords
            },
        )
    }
}
