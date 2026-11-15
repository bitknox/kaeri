// A title's muted banner trailer (trailers boards, TrailersAndroid and TrailerDesk): a title that
// may play one opens with a taller banner, decided once per visit, so the page never moves. The
// video sits in its own 16:9 place below the top bar's buttons, the art fades to ground over it
// at its first frame and comes back half a second before the end, and a scroll brings the art back
// before the video pauses under it. The TV keeps no autoplay.

package app.kaeri.ui

import android.annotation.SuppressLint
import android.content.Context
import android.view.MotionEvent
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.compose.animation.EnterExitState
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.compose.currentStateAsState
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.EmbedStyle
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.SettingKey
import app.kaeri.engine.model.TrailerControl
import app.kaeri.i18n.R
import app.kaeri.store.TrailerPlayer
import app.kaeri.store.TrailerVisit
import kotlinx.coroutines.delay

/** The page's measurements with room for a trailer, below the status bar. */
internal object TrailerRoom {
    /** The phone's banner: the video and its foot, 336 with a 36 status bar. */
    val band = 300.dp

    /** The phone's video, 16:9 across the full width, just below the top bar's Back and More. */
    val videoTop = 60.dp

    /** The phone's poster and title: below the band, so nothing sits over the video. */
    val headTop = 312.dp

    /** The tablet's band, as the wide layout has it. */
    val wideBand = 328.dp

    /** The tablet's video at the band's trailing end. */
    val wideVideoWidth = 448.dp
    val wideVideoHeight = 252.dp

    /** From the tablet's page edge, inside the list's own 24 of padding, to the video: 28 in all. */
    val wideVideoEnd = 4.dp

    /** The sound button, 8 under the video's trailing corner on a tablet. */
    val sound = 48.dp
    val soundGap = 8.dp

    /** The phone's title leaves room for the sound button at the head's trailing edge. */
    val titleGap = sound + 16.dp

    /** How long the page rests after its own transition before the trailer loads. */
    const val SETTLE_MILLIS = 500L

    /** Scrolling past this brings the art back and pauses the trailer under it. */
    val scrollAway = 8.dp
}

/** The canvas's fade-in, PathInterpolator(0, 0, 0.58, 1), as the art goes at the first frame. */
private val ArtAway = tween<Float>(durationMillis = 200, easing = CubicBezierEasing(0f, 0f, 0.58f, 1f))

/** The canvas's fade-out, PathInterpolator(0.42, 0, 1, 1), as the art comes back. */
private val ArtBack = tween<Float>(durationMillis = TrailerVisit.FADE_OUT.toInt(), easing = CubicBezierEasing(0.42f, 0f, 1f, 1f))

/** The title whose page has room for a trailer on this visit, or none. */
internal val LocalTrailerRoom = compositionLocalOf<AnilistId?> { null }

/**
 * The visit's trailer, held by the page rather than its head, so the web view outlives the head
 * leaving the list as the page scrolls: paused under the art, it comes back within 30 s.
 */
internal val LocalBannerTrailer = compositionLocalOf<BannerTrailer?> { null }

/** Where a page's room for a trailer is decided, once. */
private class RoomLatch {
    var value: Boolean? = null
}

/**
 * Whether the page opened as [entry] makes room for a trailer, decided once per visit so nothing
 * moves afterwards: the title has a trailer the engine lets start by itself now, Play trailers
 * allows it on this network, and the device holds nothing back (Data Saver, Battery Saver, Remove
 * animations, a low-memory device). Undecided until the title says whether it has a trailer.
 */
@Composable
internal fun rememberTrailerRoom(entry: Entry?): Boolean {
    val store = LocalAppStore.current
    val latch = remember { RoomLatch() }
    val spoilers = entry?.let { spoilerProtected(it.id) } ?: true
    if (latch.value == null && entry != null) {
        if (entry.trailer != null) {
            latch.value = store.trailerMayAutoplay(entry, spoilers)
        } else if (entry.detail?.partial == false) {
            latch.value = false
        }
    }
    return latch.value == true
}

/** What the visit's web view is now, for the visit's controls to reach. */
private class PlayerRef {
    var player: TrailerPlayer? = null
}

/** One visit's banner trailer as a page draws it. */
internal class BannerTrailer(
    /** The web view while it exists. */
    val player: TrailerPlayer?,
    /** The art's opacity: 0 while the video plays in front, 1 otherwise. */
    val art: State<Float>,
    /** Whether the video is in front, which the sound button follows. */
    val shown: Boolean,
)

/**
 * The trailer of the page opened as [entry], for as long as the page is on screen: it loads once
 * the page has settled and plays while it is [active], the app is in front and nothing covers the
 * page. A sheet, the background, the player, another page or the start of a back gesture stops it
 * for the visit.
 */
@Composable
internal fun rememberBannerTrailer(entry: Entry, active: Boolean, away: State<Boolean>): BannerTrailer {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val trailer = entry.trailer
    val scope = rememberCoroutineScope()
    val ref = remember(entry.id) { PlayerRef() }
    val visit = remember(entry.id) { TrailerVisit(scope, send = { ref.player?.control(it) }) }
    val live by visit.live.collectAsStateWithLifecycle()
    val shown by visit.shown.collectAsStateWithLifecycle()
    val spoilers = spoilerProtected(entry.id)

    // Only while the app is in front, the page has finished coming in and is not going (a back
    // gesture seeks its exit from the gesture's start), and no sheet or menu has the window.
    val resumed by LocalLifecycleOwner.current.lifecycle.currentStateAsState()
    val focused = LocalWindowInfo.current.isWindowFocused
    val page = LocalPageAnimation.current
    val settled = page?.transition?.let { it.currentState == EnterExitState.Visible && it.targetState == EnterExitState.Visible } ?: true
    val front = active && trailer != null && settled && resumed.isAtLeast(Lifecycle.State.RESUMED) && focused
    LaunchedEffect(front) {
        if (front) {
            delay(TrailerRoom.SETTLE_MILLIS)
            if (store.trailerMayAutoplay(entry, spoilers)) visit.begin()
        } else {
            visit.leave()
        }
    }
    DisposableEffect(visit) { onDispose { visit.release() } }
    LaunchedEffect(visit) { snapshotFlow { away.value }.collect { visit.scrolledAway(it) } }

    val subtitles = store.settings.row(SettingKey.SUBTITLES)?.value.orEmpty()
    val player = remember(live, trailer) {
        if (live && trailer != null) TrailerPlayer(context, trailer, EmbedStyle.BANNER, subtitles, visit::signal) else null
    }
    ref.player = player
    DisposableEffect(player) {
        onDispose {
            player?.release()
            if (ref.player === player) ref.player = null
        }
    }
    val art = animateFloatAsState(if (shown) 0f else 1f, if (shown) ArtAway else ArtBack, label = "trailer art") // i18n-skip
    return BannerTrailer(player, art, shown)
}

/** Whether [list] has scrolled past [TrailerRoom.scrollAway]. */
@Composable
internal fun rememberScrolledAway(list: LazyListState): State<Boolean> {
    val limit = with(LocalDensity.current) { TrailerRoom.scrollAway.toPx() }
    return remember(list, limit) { derivedStateOf { list.firstVisibleItemIndex > 0 || list.firstVisibleItemScrollOffset > limit } }
}

/** Whether [grid] has scrolled past [TrailerRoom.scrollAway]. */
@Composable
internal fun rememberScrolledAway(grid: androidx.compose.foundation.lazy.grid.LazyGridState): State<Boolean> {
    val limit = with(LocalDensity.current) { TrailerRoom.scrollAway.toPx() }
    return remember(grid, limit) { derivedStateOf { grid.firstVisibleItemIndex > 0 || grid.firstVisibleItemScrollOffset > limit } }
}

/**
 * The phone's head with room for a trailer: the art across the band, the video in its 16:9 place
 * under it, and the poster, title and sound button below the band. [trailer] is the visit's, on
 * the page on screen; a page beneath keeps the same room with the art alone.
 */
@Composable
internal fun TrailerHead(
    trailer: BannerTrailer?,
    list: LazyListState,
    inset: Dp,
    art: @Composable (Modifier) -> Unit,
    header: @Composable (Modifier) -> Unit,
) {
    Box {
        trailer?.player?.let { player ->
            TrailerVideo(
                player,
                Modifier
                    .padding(top = inset + TrailerRoom.videoTop)
                    .fillMaxWidth()
                    .aspectRatio(16f / 9f),
            )
        }
        art(
            Modifier
                .fillMaxWidth()
                .height(inset + TrailerRoom.band)
                .graphicsLayer {
                    alpha = trailer?.art?.value ?: 1f
                    // The banner scrolls at half the page's speed (T1's parallax).
                    translationY = (if (list.firstVisibleItemIndex == 0) list.firstVisibleItemScrollOffset else 0) * 0.5f
                },
        )
        header(Modifier.padding(top = inset + TrailerRoom.headTop))
        if (trailer != null) {
            TrailerSoundButton(
                trailer,
                Modifier
                    .align(Alignment.TopEnd)
                    .padding(top = inset + TrailerRoom.headTop, end = 16.dp),
            )
        }
    }
}

/**
 * The trailer's web view in its place, which takes no touch: a drag that starts on the video
 * scrolls the page, and nothing reaches the player. Nothing is drawn over it while it plays.
 */
@Composable
internal fun TrailerVideo(player: TrailerPlayer, modifier: Modifier = Modifier) {
    AndroidView(
        factory = { context -> Untouchable(context).apply { addView(player.view) } },
        modifier = modifier,
        onRelease = { it.removeAllViews() },
    )
}

/** A holder that lets no touch, key or focus reach what it holds, and says it took none. */
@SuppressLint("ViewConstructor")
private class Untouchable(context: Context) : FrameLayout(context) {
    init {
        descendantFocusability = ViewGroup.FOCUS_BLOCK_DESCENDANTS
        isFocusable = false
        importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
    }

    override fun dispatchTouchEvent(event: MotionEvent): Boolean = false

    override fun dispatchGenericMotionEvent(event: MotionEvent): Boolean = false
}

/**
 * The sound button, 48 dp, plain and never amber: in while the video is in front, fading with the
 * art. Sound is off at every start and the choice lasts for this trailer only.
 */
@Composable
internal fun TrailerSoundButton(trailer: BannerTrailer, modifier: Modifier = Modifier) {
    val player = trailer.player ?: return
    var muted by remember(player) { mutableStateOf(true) }
    val alpha = 1f - trailer.art.value
    if (alpha <= 0f && !trailer.shown) return
    val label = stringResource(R.string.title_trailer_sound)
    val state = stringResource(if (muted) R.string.value_off else R.string.value_on)
    IconButton(
        onClick = {
            muted = !muted
            player.control(if (muted) TrailerControl.MUTE else TrailerControl.UNMUTE)
        },
        enabled = trailer.shown,
        modifier = modifier
            .size(TrailerRoom.sound)
            .graphicsLayer { this.alpha = 1f - trailer.art.value }
            .semantics {
                contentDescription = label
                stateDescription = state
            },
    ) {
        Box(
            Modifier
                .size(TrailerRoom.sound)
                .background(MaterialTheme.colorScheme.surfaceContainer, CircleShape)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Icon(if (muted) Icons.SoundOff else Icons.VolumeUp, contentDescription = null, tint = MaterialTheme.colorScheme.onSurface, modifier = Modifier.size(20.dp))
        }
    }
}
