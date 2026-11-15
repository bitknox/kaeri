// The cast controller: the mini over the navigation bar while a TV plays, and the full controller
// it grows into. One surface morphs between them on the standard spring (SharedTransitionLayout),
// and a back swipe seeks it back into the mini. The engine's card says what both read
// (CastModel); nothing here keeps a state of its own beyond a dragged seek held until the
// engine's next card.

package app.kaeri.ui

import androidx.activity.compose.PredictiveBackHandler
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
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
internal class CastMorph(val layout: SharedTransitionScope, val transition: Transition<Boolean>, val still: Boolean, val origin: CastOrigin, private val seeking: () -> Boolean) {
    /** The surface's flight on the standard spring, or, while a back swipe seeks it, a straight line in time. */
    val bounds = BoundsTransform { _, _ ->
        if (seeking()) tween(300, easing = LinearEasing) else spring(dampingRatio = 1f, stiffness = 440f, visibilityThreshold = Rect.VisibilityThreshold)
    }

    /** The mini is in its slot: it plays, or joins again, and the controller has been closed once since. */
    var miniPresent by mutableStateOf(false)
}

internal val LocalCastMorph = staticCompositionLocalOf<CastMorph?> { null }

/** Where a Play that goes to the TV started, for the controller to grow from. `null` outside [CastMorphHost]. */
val LocalCastOrigin = staticCompositionLocalOf<CastOrigin?> { null }

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
    LaunchedEffect(open) {
        if (state.targetState != open || state.currentState != open) state.animateTo(open)
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
        val morph = remember(this, transition, still) { CastMorph(this, transition, still, origin) { seeking } }
        // A controller that opened from a Play button leaves the mini's slot empty until it closes.
        LaunchedEffect(morph, has, open) {
            if (!has) morph.miniPresent = false else if (!open) morph.miniPresent = true
        }
        CompositionLocalProvider(LocalCastMorph provides morph, LocalCastOrigin provides origin) {
            Box(Modifier.fillMaxSize()) {
                content()
                if (!hidden) CastExpandedLayer(morph)
            }
        }
    }
    // A back swipe seeks the controller back into the mini, as far as the finger goes.
    val scope = rememberCoroutineScope()
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
        Box(Modifier.fillMaxWidth().height(66.dp)) {
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
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh),
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
 * The mini: a 2 dp playhead along its top, the cover, the title over the TV's name, pause or play,
 * and the filled cast glyph for the TV's dialog. A tap anywhere else opens the controller.
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
    val rejoining = joining != null
    val usable = !now.starting && !rejoining
    val position by produceState(now.position, now, usable) {
        while (true) {
            value = cast.playhead() ?: cast.itemPlayhead() ?: now.position
            if (now.paused || !usable) break
            delay(1000)
        }
    }
    val opens = stringResource(R.string.cast_mini_opens)
    val status = if (rejoining) stringResource(R.string.tv_status_reconnecting, name) else stringResource(if (now.paused) R.string.cast_status_paused else R.string.tv_status_playing, name)
    Column(
        Modifier
            .fillMaxSize()
            .clickable(onClickLabel = opens, enabled = !rejoining) { cast.openCard() }
            .semantics(mergeDescendants = false) { contentDescription = "${now.title}, $status" }, // i18n-skip
    ) {
        val duration = now.duration?.takeIf { it > 0 }
        Box(Modifier.fillMaxWidth().height(2.dp).background(ObiSilk.Edge)) {
            if (duration != null && !now.starting) {
                Box(Modifier.fillMaxWidth((position / duration).toFloat().coerceIn(0f, 1f)).height(2.dp).background(colors.primary))
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
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    if (rejoining) {
                        CircularProgressIndicator(Modifier.size(12.dp), strokeWidth = 1.5.dp, color = colors.onSurfaceVariant)
                    } else {
                        CastGlyph(CastGlyphState.Filled, size = 16.dp, tint = colors.onSurfaceVariant)
                    }
                    Text(
                        if (rejoining) status else name,
                        style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp),
                        color = colors.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            Box(Modifier.alpha(if (usable) 1f else 0.38f)) {
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

/** The status line's dot: water while it plays or pauses, vermilion once it failed, a spinner as it starts. */
@Composable
private fun CastStatus(text: String?, stage: CastStage) {
    val colors = MaterialTheme.colorScheme
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (text != null) {
            when (stage) {
                CastStage.Starting -> CircularProgressIndicator(Modifier.size(12.dp), strokeWidth = 1.5.dp, color = colors.onSurfaceVariant)
                CastStage.Playing -> StateDot(ObiSilk.Water)
                CastStage.Failed, CastStage.Lost -> StateDot(ObiSilk.Vermilion)
            }
            Text(text, style = MaterialTheme.typography.bodyMedium.copy(fontSize = 14.sp), color = colors.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
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
    val status = when (stage) {
        CastStage.Starting -> if (quiet) null else shown.cast.phaseLine?.text(context) ?: Strings.tvSessionStarting(context, name)
        CastStage.Playing -> if (now.paused) stringResource(R.string.cast_status_paused, name) else Strings.tvStatusPlaying(context, name)
        CastStage.Failed -> Strings.castCardFailed(context, name)
        CastStage.Lost -> Strings.castCardStopped(context)
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
            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) { CastStatus(status, stage) }
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
            if (over) {
                shown.cast.notice?.let { ProblemLine(it.text(context)) }
            } else {
                if (playing) {
                    now.skip?.let { skip ->
                        OutlinedButton(onClick = { cast.send(FfiTvCommand.Skip) }, modifier = Modifier.padding(top = 8.dp)) { Text(skip.label, color = colors.onSurface) }
                    }
                }
                shown.cast.undo?.let { CastUndoNote(cast, it.kind) }
                CastSeekLine(cast, now, shown, waiting = session.item?.waiting)
                CastTransport(cast, now, usable = playing)
                AnimatedVisibility(upNext != null, enter = expandVertically(Motion.standard()) + fadeIn(Motion.fadeIn()), exit = shrinkVertically(Motion.standard()) + fadeOut(Motion.fadeOut())) {
                    val last = remember { LastUpNext() }
                    upNext?.let { last.value = it }
                    last.value?.let { CastUpNext(cast, it, now.id) }
                }
            }
            Spacer(Modifier.height(8.dp))
            // The TV's tracks and volume, dimmed while it starts or once it failed.
            val usable = playing
            CastTrackRow(stringResource(R.string.player_subtitles), FfiTvTrackKind.SUBTITLE, now.subtitles, now.subtitlesOff, cast, enabled = usable)
            TvDivider()
            CastTrackRow(stringResource(R.string.player_audio), FfiTvTrackKind.AUDIO, now.audio, off = false, cast, enabled = usable)
            TvDivider()
            CastVolumeRow(cast, shown.level, shown.cast.muted == true || session.muted == true, now.volume, enabled = usable)
            // The footer: what each state offers, then More.
            val notice = shown.cast.notice
            Row(Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                FlowRow(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    val playHere = @Composable { CastPill(stringResource(R.string.cast_play_here), cast::playHere) }
                    when {
                        notice == null -> {
                            playHere()
                            CastPill(stringResource(R.string.cast_stop), cast::disconnect)
                        }
                        notice is Msg.NoticeCastNeverFetches -> {
                            Button(onClick = cast::tryAgain) { Text(stringResource(R.string.action_try_again)) }
                            playHere()
                        }
                        notice is Msg.NoticeCastNotSameNetwork || notice is Msg.NoticeCastLost || notice is Msg.NoticeCastNoDevice -> {
                            playHere()
                            CastPill(stringResource(R.string.tv_choose_another), cast::chooseAnother)
                        }
                        else -> {
                            if (entry != null) Button(onClick = { choosingSource = true }) { Text(stringResource(R.string.player_choose_source)) }
                            if (notice !is Msg.NoticeCastDeepLink) playHere()
                        }
                    }
                }
                var more by remember { mutableStateOf(false) }
                Box {
                    IconButton(onClick = { more = true }) {
                        Icon(Icons.More, contentDescription = stringResource(R.string.action_more), tint = colors.onSurface)
                    }
                    Menu(expanded = more, onDismissRequest = { more = false }) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.tv_choose_another), style = MaterialTheme.typography.labelLarge) },
                            modifier = Modifier.heightIn(min = 48.dp),
                            onClick = {
                                more = false
                                cast.chooseAnother()
                            },
                        )
                        if (notice == null) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.cast_stop), style = MaterialTheme.typography.labelLarge) },
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
        }
    }
    if (choosingSource && entry != null) SourcePickerSheet(entry) { choosingSource = false }
}

/** The Up next card the controller last showed, kept out of the snapshot for it to leave with. */
private class LastUpNext {
    var value: FfiCastUpNext? = null
}

/** An outlined pill in silk, never amber. */
@Composable
private fun CastPill(text: String, onClick: () -> Unit) {
    OutlinedButton(onClick = onClick) { Text(text, color = MaterialTheme.colorScheme.onSurface) }
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
 * The seek bar: a 16 dp amber track up to a 4 by 44 thumb, a 6 dp gap each side of it, and a
 * 4 dp track on after it; the position and the time left in tabular figures under it. While the
 * episode starts there is no thumb and no times. Dragged, it seeks on release and holds there
 * until a card with a higher `rev` arrives: the engine holds the asked value meanwhile.
 * Mid-file waiting shows under the bar after 4 s, and never dims anything.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CastSeekLine(cast: CastModel, now: FfiNowCard, shown: CastShown, waiting: Long?) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val playing = shown.stage == CastStage.Playing
    val duration = now.duration?.takeIf { it > 0 } ?: 0.0
    val position by produceState(cast.playhead() ?: now.position, now, playing) {
        while (true) {
            value = cast.playhead() ?: now.position
            if (now.paused || !playing) break
            delay(1000)
        }
    }
    var dragging by remember(now.id, now.episode) { mutableStateOf<Float?>(null) }
    var sent by remember(now.id, now.episode) { mutableStateOf<Pair<Float, ULong>?>(null) }
    val rev = shown.cast.card.rev
    // Let go once the engine has drawn the cast again since the seek was sent.
    sent?.let { (_, at) -> if (rev > at) sent = null }
    val at = ((dragging ?: sent?.first)?.toDouble() ?: position).coerceIn(0.0, maxOf(duration, 0.0))
    val usable = playing && duration > 0
    val label = stringResource(R.string.tv_sheet_position)
    val state = stringResource(R.string.tv_sheet_position_state, TvWords.clock(at), TvWords.clock(duration))
    val time = MaterialTheme.typography.labelMedium.tabular()
    Column(Modifier.padding(top = 12.dp)) {
        val range = 0f..maxOf(duration.toFloat(), 1f)
        val slider = remember(range) { SliderState(at.toFloat(), 0, range) }
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
            enabled = usable,
            thumb = {
                if (usable) Box(Modifier.size(4.dp, 44.dp).background(colors.primary, RoundedCornerShape(2.dp)))
            },
            track = { CastSeekTrack(it.coercedValueAsFraction, thumb = usable) },
            modifier = Modifier.fillMaxWidth().height(44.dp).semantics {
                contentDescription = label
                if (playing) stateDescription = state
            },
        )
        Box(Modifier.fillMaxWidth().heightIn(min = 20.dp)) {
            if (playing && duration > 0) {
                Text(TvWords.clock(at), style = time, color = colors.onSurfaceVariant, modifier = Modifier.align(Alignment.CenterStart))
                Text("−" + TvWords.clock(duration - at), style = time, color = colors.onSurfaceVariant, modifier = Modifier.align(Alignment.CenterEnd)) // i18n-skip
            }
            // Waiting for the stream mid-file: a spinner after 600 ms on this device's clock, the words from 4 s.
            var spinning by remember { mutableStateOf(false) }
            LaunchedEffect(waiting != null) {
                spinning = false
                if (waiting != null) {
                    delay(QUIET_FOR)
                    spinning = true
                }
            }
            if (playing && waiting != null && spinning) {
                Row(Modifier.align(Alignment.Center), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    CircularProgressIndicator(Modifier.size(10.dp), strokeWidth = 1.5.dp, color = colors.onSurfaceVariant)
                    if (waiting >= CastModel.COUNT_FROM_SECONDS) {
                        Text(Msg.CastPhaseWaiting(shown.tvName, waiting).text(context), style = MaterialTheme.typography.labelMedium, color = colors.onSurfaceVariant, maxLines = 1)
                    }
                }
            }
        }
    }
}

/** The seek bar's track: amber 16 high up to the thumb's gap, 4 high in RaisedSelected after it. */
@Composable
private fun CastSeekTrack(fraction: Float, thumb: Boolean) {
    val amber = MaterialTheme.colorScheme.primary
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
            drawPath(path, amber)
        }
    }
}

/** Back 10, pause in its 64 tonal square, forward 10 and next, 18 apart; at 0.38 while the TV starts. */
@Composable
private fun CastTransport(cast: CastModel, now: FfiNowCard, usable: Boolean) {
    Row(
        Modifier.fillMaxWidth().padding(top = 12.dp).alpha(if (usable) 1f else 0.38f),
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

/** The subtitle or audio row: its name, the track on and ⇕ at the end, a menu of the tracks. */
@Composable
private fun CastTrackRow(title: String, kind: FfiTvTrackKind, tracks: List<FfiTvTrack>, off: Boolean, cast: CastModel, enabled: Boolean) {
    var open by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val value = if (tracks.isEmpty()) "" else TvWords.chosen(context, tracks, off)
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 56.dp)
                .clickable(enabled = enabled && tracks.isNotEmpty()) { open = true }
                .alpha(if (enabled) 1f else 0.38f)
                .semantics(mergeDescendants = true) {},
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = colors.onSurface, modifier = Modifier.weight(1f))
            Text(value, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.widthIn(max = 200.dp))
            Icon(Icons.DropDown, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(18.dp))
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
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).alpha(if (enabled) 1f else 0.38f),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = colors.onSurface)
        if (!level) {
            Spacer(Modifier.weight(1f))
            Text(stringResource(R.string.cast_volume_fixed), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1)
            OutlinedButton(
                onClick = { if (enabled) cast.toggleMute() },
                contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 14.dp),
                modifier = Modifier.heightIn(min = 40.dp),
            ) {
                Icon(if (muted) Icons.SoundOff else Icons.VolumeDown, contentDescription = null, tint = colors.onSurface, modifier = Modifier.size(16.dp))
                Spacer(Modifier.width(6.dp))
                Text(muteLabel, color = colors.onSurface)
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
            modifier = Modifier.weight(1f).semantics {
                contentDescription = label
                stateDescription = volumeWords
            },
        )
    }
}
