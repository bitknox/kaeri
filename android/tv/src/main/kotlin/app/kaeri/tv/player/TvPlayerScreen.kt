// The TV player: the video full screen through :player, the starting screen until the first
// frame, and the chrome over a 35 % dim (title and clock at the top, the seek bar and the button
// row at the bottom), hidden after 5 s with no key. The D-pad follows the brief's player table.

package app.kaeri.tv.player

import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import app.kaeri.i18n.Strings
import app.kaeri.i18n.R
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onKeyEvent
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Text
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.playerScrim
import app.kaeri.engine.model.NextEpisode
import app.kaeri.engine.model.PlayerChoice
import app.kaeri.engine.model.TimeRange
import app.kaeri.player.Device
import app.kaeri.player.PlayerModel
import app.kaeri.player.SeekSegment
import app.kaeri.player.Track
import app.kaeri.store.AirTime
import app.kaeri.store.AppClock
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.ArtImage
import app.kaeri.player.SkipWords
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.panelFocus
import app.kaeri.tv.ui.panelWidth
import app.kaeri.tv.ui.rememberScreenReader
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.screens.subtitleStyleRows
import app.kaeri.tv.ui.LocalReducedMotion
import app.kaeri.tv.ui.PANEL_END
import app.kaeri.tv.ui.ReleasesPanel
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TvIcons
import app.kaeri.tv.ui.TvListRow
import app.kaeri.tv.ui.TvTitle
import app.kaeri.tv.ui.enterFade
import app.kaeri.tv.ui.entryFocus
import app.kaeri.tv.ui.exitFade
import app.kaeri.tv.ui.focusSoon
import app.kaeri.tv.ui.tryFocus

/** The player's panels, each the 400-wide column on the right. */
enum class PlayerPanel { Episodes, Tracks, Sources, Info, SkipTimes }

/**
 * The way to add a plugin store, while this TV has no source that can play: a playback that
 * failed then offers it first. Null once a source is there, or where nothing provides it.
 */
val LocalAddStore = compositionLocalOf<(() -> Unit)?> { null }

/** What the chrome puts focus on as it comes up. */
private enum class ChromeFocus { Play, SeekBar, Episodes }

/** Previous goes to the episode before within its first 3 s; after, to this one's start. */
private const val PREVIOUS_WITHIN = 3.0

/**
 * Where the subtitles sit, in dp above the screen's foot: their own place, and while the chrome
 * is up, clear of its times, seek bar and buttons, which reach 125 up from the foot.
 */
private const val SUBTITLES_DOWN = 32f
private const val SUBTITLES_UP = 160f

@Composable
fun TvPlayerScreen(
    model: PlayerModel,
    modifier: Modifier = Modifier,
    video: @Composable (Modifier) -> Unit = { VideoSurface(model, it) },
) {
    val root = remember { FocusRequester() }
    val context = LocalContext.current
    // The corner card's button (Play now, or Switch source), which keeps focus as the chrome goes.
    val cardFocus = remember { FocusRequester() }
    val playFocus = remember { FocusRequester() }
    val barFocus = remember { FocusRequester() }
    val episodesFocus = remember { FocusRequester() }
    var panel by remember { mutableStateOf<PlayerPanel?>(null) }
    var wanted by remember { mutableStateOf<ChromeFocus?>(null) }
    val run = remember { SeekRun() }
    var presses by remember { mutableIntStateOf(0) }
    var runShown by remember { mutableStateOf(false) }
    var head by remember { mutableStateOf<ScrubHead?>(null) }
    var headAt by remember { mutableStateOf<Double?>(null) }
    var heldSince by remember { mutableLongStateOf(0L) }

    // A screen reader keeps the chrome up, and Up next waits for it: it reads only what takes focus.
    val screenReader by rememberScreenReader()
    LaunchedEffect(screenReader) { model.assisted = screenReader }

    // A playback that failed keeps the starting screen up, saying so.
    val starting = model.starting != null || !model.hasPicture || model.failure != null
    val chrome = !starting && (model.paused || panel != null || model.marking != null || headAt != null || model.clock - model.lastInput < CHROME_HIDE || model.assisted)
    val skip = model.skip?.takeIf { model.corner == PlayerModel.Corner.Skip }
    val skipFresh = skip != null && model.clock - skip.since < SKIP_FRESH

    fun show(focus: ChromeFocus) {
        model.poke()
        wanted = focus
    }

    fun openPanel(which: PlayerPanel) {
        panel = which
        model.hold("panel", true)
    }

    fun closePanel() {
        panel = null
        model.hold("panel", false)
        wanted = ChromeFocus.Play
    }

    fun nextEpisode() {
        val episodes = model.menus?.episodes ?: return
        val at = episodes.indexOfFirst { it.current }
        episodes.getOrNull(at + 1)?.let { model.choose(PlayerChoice.Episode(it.number)) }
    }

    // Previous: in the first seconds, the episode before; later, this one from its start.
    fun previous() {
        val episodes = model.menus?.episodes
        val at = episodes?.indexOfFirst { it.current } ?: -1
        val before = episodes?.getOrNull(at - 1)
        if (model.position < PREVIOUS_WITHIN && before != null) model.choose(PlayerChoice.Episode(before.number)) else model.seekTo(0.0)
    }

    // Back, top to bottom: a panel closes, a scrub cancels, Up Next cancels, else the player closes.
    fun back() {
        when {
            model.choosingSource -> model.cancelChoosingSource()
            model.asksToShare -> model.answerShare(false)
            model.marking != null -> model.cancelMark()
            panel != null -> closePanel()
            model.corner == PlayerModel.Corner.Prompt -> model.dismissSkipPrompt()
            model.corner == PlayerModel.Corner.Stall -> model.keepWaiting()
            headAt != null -> {
                head = null
                headAt = null
            }
            model.corner == PlayerModel.Corner.UpNext -> model.cancelNext()
            else -> model.viewerClosed()
        }
    }
    BackHandler(enabled = model.isPresented, onBack = ::back)

    // The settled seek: one seek half a second after the last press, the count shown a moment more.
    LaunchedEffect(presses) {
        if (presses == 0) return@LaunchedEffect
        delay(SEEK_SETTLE)
        model.seekBy(run.take())
        presses = 0
        delay(SEEK_SETTLE * 2)
        runShown = false
    }

    // Focus follows the chrome: on what asked for it as it comes up, back on the root as it goes.
    // Whether the root itself holds focus, as it does while the chrome is hidden.
    var onRoot by remember { mutableStateOf(false) }

    // The starting screen's own buttons (another source, Back) keep the focus they take: the root
    // taking it back would leave them unreachable.
    val startFocus = remember { StartFocus() }
    val layerButtons = starting && (model.failure != null && !model.choosingSource || startFocus.held)
    // A corner card up: the chrome going hands focus to it, not to the root behind it.
    val cardUp = !starting && panel == null && model.marking == null && (
        model.corner == PlayerModel.Corner.Prompt && model.skipPrompt != null ||
            model.corner == PlayerModel.Corner.Stall && model.stall != null && !model.choosingSource ||
            model.corner == PlayerModel.Corner.UpNext && model.upNext != null
        )
    LaunchedEffect(chrome, wanted, onRoot, layerButtons, cardUp) {
        if (!chrome) {
            if (layerButtons) return@LaunchedEffect
            val home = if (cardUp) cardFocus else root
            repeat(3) {
                androidx.compose.runtime.withFrameNanos { }
                if (home.tryFocus()) return@LaunchedEffect
            }
            return@LaunchedEffect
        }
        // Up with nothing asked for (paused by a media key, or at the end of an episode): on
        // play/pause, so the D-pad is never left on the hidden root while the chrome shows.
        val asked = wanted ?: if (onRoot) ChromeFocus.Play else return@LaunchedEffect
        val target = when (asked) {
            ChromeFocus.Play -> playFocus
            ChromeFocus.SeekBar -> barFocus
            ChromeFocus.Episodes -> episodesFocus
        }
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (target.tryFocus()) {
                wanted = null
                return@LaunchedEffect
            }
        }
    }

    fun hiddenKey(event: KeyEvent): Boolean {
        if (event.type != KeyEventType.KeyDown) return false
        return when (event.key) {
            Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> {
                if (model.corner == PlayerModel.Corner.Undo) {
                    model.undoSkip()
                } else if (skipFresh) {
                    model.takeSkip()
                } else {
                    model.togglePause()
                    show(ChromeFocus.Play)
                }
                true
            }
            Key.DirectionLeft, Key.DirectionRight -> {
                run.press(forward = event.key == Key.DirectionRight)
                presses = run.presses
                runShown = true
                true
            }
            Key.DirectionUp -> {
                show(ChromeFocus.SeekBar)
                true
            }
            Key.DirectionDown -> {
                show(ChromeFocus.Episodes)
                true
            }
            else -> false
        }
    }

    // The subtitles rise above the controls while they are up and fall back with them, as on the
    // phone: sub-pos, a share of the screen's height, which the controls never cover.
    var height by remember { mutableIntStateOf(0) }
    val density = LocalDensity.current
    val lift by animateFloatAsState(
        if (chrome) SUBTITLES_UP else SUBTITLES_DOWN,
        if (LocalReducedMotion.current) snap() else Motion.standard(),
        label = "subtitles", // i18n-skip
    )
    LaunchedEffect(height) {
        if (height == 0) return@LaunchedEffect
        val tall = with(density) { height.toDp().value }
        snapshotFlow { lift }.collectLatest { above ->
            model.liftSubtitles(100.0 - (above - SUBTITLES_DOWN) / tall * 100.0)
            delay(40)
        }
    }

    val rootName = "${model.showTitle}, ${model.episodeLine(context)}" // i18n-skip
    Box(
        modifier
            .fillMaxSize()
            .onSizeChanged { height = it.height }
            .background(Color.Black)
            .onPreviewKeyEvent { event ->
                if (event.type == KeyEventType.KeyDown && event.nativeKeyEvent.repeatCount == 0) heldSince = model.clock
                // The remote's Back, here before Compose spends it on moving focus out to this
                // focusable root, which would leave a panel open with focus behind it.
                if (event.key == Key.Back) {
                    if (event.type == KeyEventType.KeyUp && !event.nativeKeyEvent.isCanceled) back()
                    return@onPreviewKeyEvent true
                }
                if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                // While a start runs nothing holds focus; the first direction press lands on Choose source.
                if (starting && model.failure == null && !startFocus.held && startFocus.ready &&
                    event.key in setOf(Key.DirectionUp, Key.DirectionDown, Key.DirectionLeft, Key.DirectionRight)
                ) {
                    startFocus.target.tryFocus()
                    return@onPreviewKeyEvent true
                }
                // Any key, the TV's remote's or a phone's: somebody is there.
                model.viewerThere()
                when (event.key) {
                    Key.MediaPlayPause -> { model.togglePause(); true }
                    Key.MediaPlay -> { model.setPlaying(true); true }
                    Key.MediaPause -> { model.setPlaying(false); true }
                    Key.MediaRewind -> { model.seekBy(if (event.nativeKeyEvent.repeatCount > 0) -30.0 else -10.0); true }
                    Key.MediaFastForward -> { model.seekBy(if (event.nativeKeyEvent.repeatCount > 0) 30.0 else 10.0); true }
                    Key.MediaNext -> { nextEpisode(); true }
                    Key.MediaPrevious -> { previous(); true }
                    else -> {
                        if (chrome) model.poke()
                        false
                    }
                }
            }
            .focusRequester(root)
            // With the chrome hidden the root holds focus; it says what is playing rather than nothing.
            .semantics { if (!chrome && !starting) contentDescription = rootName }
            .onKeyEvent { event -> if (!chrome && !starting) hiddenKey(event) else false }
            .onFocusChanged { onRoot = it.isFocused }
            .focusable(),
    ) {
        video(Modifier.fillMaxSize())
        StartingLayer(model, starting, startFocus, onClose = { model.viewerClosed() })
        AnimatedVisibility(
            visible = chrome,
            // A panel over it is all a screen reader reaches.
            modifier = if (panel != null) Modifier.clearAndSetSemantics { } else Modifier,
            enter = enterFade(),
            exit = exitFade(),
        ) {
            Chrome(
                model,
                headAt = headAt,
                playFocus = playFocus,
                barFocus = barFocus,
                episodesFocus = episodesFocus,
                onPanel = ::openPanel,
                onNext = ::nextEpisode,
                hasNext = model.menus?.episodes?.let { list -> list.indexOfFirst { it.current }.let { it >= 0 && it < list.lastIndex } } == true,
                onBarKey = { event ->
                    val duration = model.duration ?: return@Chrome false
                    if (event.type != KeyEventType.KeyDown) return@Chrome false
                    when (event.key) {
                        Key.DirectionLeft, Key.DirectionRight -> {
                            val scrub = head ?: ScrubHead(model.position, duration, model.segments).also { head = it }
                            val held = if (event.nativeKeyEvent.repeatCount > 0) model.clock - heldSince else 0L
                            scrub.move(forward = event.key == Key.DirectionRight, heldMillis = held)
                            headAt = scrub.at
                            model.poke()
                            true
                        }
                        Key.DirectionCenter, Key.Enter, Key.NumPadEnter -> {
                            headAt?.let { model.seekTo(it) }
                            head = null
                            headAt = null
                            true
                        }
                        Key.DirectionUp, Key.DirectionDown -> {
                            // Leaving the bar leaves the head; playback carries on where it was.
                            head = null
                            headAt = null
                            false
                        }
                        else -> false
                    }
                },
            )
        }
        if (!chrome && runShown) SeekCount(model, run.label(context))
        // Mid-episode waiting: the indicator at the centre, until playback goes on.
        Fading(model.showsBuffering && !chrome && model.stall == null, Modifier.align(Alignment.Center)) { Spinner() }
        val pill = when {
            chrome -> null
            model.corner == PlayerModel.Corner.Undo -> model.undo?.let { Strings.tvPlayerUndoPill(context, it.label(context)) }
            skip != null -> skip.label(context)
            else -> null
        }
        var lastPill by remember { mutableStateOf(pill) }
        if (pill != null) lastPill = pill
        Fading(pill != null, Modifier.align(Alignment.BottomEnd).padding(end = 48.dp, bottom = 540.dp - 400.dp)) {
            SkipPill(lastPill.orEmpty(), fresh = pill == null || model.corner == PlayerModel.Corner.Undo || skipFresh, Modifier)
        }
        // Never over the starting screen: the card belongs to the episode on screen.
        val upNext = model.upNext?.takeIf { !starting && model.corner == PlayerModel.Corner.UpNext && panel == null }
        var lastNext by remember { mutableStateOf(upNext?.next) }
        if (upNext != null) lastNext = upNext.next
        Fading(upNext != null, Modifier.align(Alignment.BottomEnd).padding(end = 48.dp, bottom = 27.dp)) {
            lastNext?.let { UpNextCard(model, it, cardFocus, Modifier) }
        }
        val stall = model.stall?.takeIf { !starting && model.corner == PlayerModel.Corner.Stall && panel == null && !model.choosingSource }
        var lastStall by remember { mutableStateOf(stall) }
        if (stall != null) lastStall = stall
        Fading(stall != null, Modifier.align(Alignment.BottomEnd).padding(end = 48.dp, bottom = 27.dp)) {
            lastStall?.let { StallPrompt(model, it, cardFocus) }
        }
        val prompt = model.skipPrompt?.takeIf { !starting && model.corner == PlayerModel.Corner.Prompt && panel == null && model.marking == null }
        var lastPrompt by remember { mutableStateOf(prompt) }
        if (prompt != null) lastPrompt = prompt
        Fading(prompt != null, Modifier.align(Alignment.BottomEnd).padding(end = 48.dp, bottom = 27.dp)) {
            lastPrompt?.let { SkipPromptCard(model, it, cardFocus) }
        }
        // A card going away hands focus back to the root, so the D-pad still works the player.
        LaunchedEffect(stall != null, upNext != null, prompt != null) {
            if (stall == null && upNext == null && prompt == null && !chrome && panel == null) root.tryFocus()
        }
        panel?.let { open ->
            Panel(model, open, onDone = { closePanel() }, modifier = Modifier.align(Alignment.CenterEnd))
        }
        if (model.choosingSource) {
            ReleasesPanel(model.sourceChoices, onPick = model::play, onClose = model::cancelChoosingSource, modifier = Modifier.align(Alignment.CenterEnd))
        }
        model.marking?.let { view -> MarkPanel(model, view, Modifier.align(Alignment.CenterEnd)) }
        // What was done to the skip times, once, at the top.
        val said = model.skipNotice
        Fading(said != null && !starting, Modifier.align(Alignment.TopCenter).padding(top = 80.dp)) {
            var last by remember { mutableStateOf(said) }
            if (said != null) last = said
            last?.let { SkipPill(SkipWords.notice(context, it.notice), fresh = false, Modifier) }
        }
        if (model.asksToShare) ShareSkipTimesDialog(model)
        if (model.stillWatching) StillWatchingDialog(model)
    }
    LaunchedEffect(Unit) { root.tryFocus() }
}

/** The video plane: mpv's SurfaceView, new for each surface generation. */
@Composable
fun VideoSurface(model: PlayerModel, modifier: Modifier) {
    val density = LocalDensity.current.density
    key(model.surfaceGeneration) {
        AndroidView(
            factory = { context ->
                SurfaceView(context).apply {
                    holder.addCallback(object : SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: SurfaceHolder) = model.surfaceCreated(holder.surface)

                        override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                            model.surfaceChanged(width, height)
                            display?.let { model.surfaceDisplay(Device.display(it, density)) }
                        }

                        override fun surfaceDestroyed(holder: SurfaceHolder) = model.surfaceDestroyed()
                    })
                }
            },
            modifier = modifier,
        )
    }
}

/** The 32 dp circular indicator, an amber arc turning. */
@Composable
internal fun Spinner() {
    // A preview, and a screenshot, draws it still.
    val still = androidx.compose.ui.platform.LocalInspectionMode.current || LocalReducedMotion.current
    val turning = rememberInfiniteTransition(label = "spinner") // i18n-skip
    val turned by turning.animateFloat(0f, 360f, infiniteRepeatable(tween(1000, easing = LinearEasing)), label = "angle") // i18n-skip
    val angle = if (still) 0f else turned
    Canvas(Modifier.size(32.dp)) {
        val stroke = 3.dp.toPx()
        drawArc(ObiSilk.Edge, 0f, 360f, useCenter = false, style = Stroke(stroke))
        drawArc(ObiSilk.Amber, angle, 90f, useCenter = false, style = Stroke(stroke, cap = androidx.compose.ui.graphics.StrokeCap.Round))
    }
}

@Composable
private fun Chrome(
    model: PlayerModel,
    headAt: Double?,
    playFocus: FocusRequester,
    barFocus: FocusRequester,
    episodesFocus: FocusRequester,
    onPanel: (PlayerPanel) -> Unit,
    onNext: () -> Unit,
    hasNext: Boolean,
    onBarKey: (KeyEvent) -> Boolean,
) {
    val duration = model.duration ?: 0.0
    val position = model.position
    Box(Modifier.fillMaxSize().playerScrim(edge = 120.dp)) {
        // Top left, the show and the episode; top right, the clock and when it ends.
        Column(Modifier.align(Alignment.TopStart).padding(start = 48.dp, top = 27.dp).width(560.dp)) {
            Text(model.showTitle, style = TvTitle.player, color = ObiSilk.Silk, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(model.episodeLine(LocalContext.current), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Column(Modifier.align(Alignment.TopEnd).padding(end = 48.dp, top = 27.dp), horizontalAlignment = Alignment.End) {
            // Read afresh each time; the minute's turn only makes sure that happens while paused.
            wallMinute()
            val now = AppClock.now()
            Text(AirTime.clock(now), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Haze)
            if (duration > 0) {
                val left = ((duration - position) / model.speed.coerceAtLeast(0.1)).toLong()
                Text(stringResource(R.string.player_ends_at, AirTime.clock(now.plusSeconds(left))), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Haze)
            }
        }
        // Bottom: the times over the seek bar, then the buttons, which end on the safe area's edge.
        Column(Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(start = 48.dp, end = 48.dp, bottom = 27.dp)) {
            // The seek bar says the times itself; reading these as well would say them twice.
            Row(Modifier.fillMaxWidth().clearAndSetSemantics { }) {
                Text(clockTime(headAt ?: position), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Silk)
                Spacer(Modifier.weight(1f))
                if (duration > 0) Text(remainingTime(headAt ?: position, duration), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Silk)
            }
            Spacer(Modifier.height(6.dp))
            SeekBar(
                position, duration, headAt, model.segments, model.buffered, Modifier.focusRequester(barFocus), onBarKey,
                onSeekTo = model::seekTo, onSeekBy = model::seekBy, provisional = model.provisional,
            )
            Spacer(Modifier.height(20.dp))
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                TonalButton(
                    onClick = { model.togglePause() },
                    icon = if (model.paused) TvIcons.Play else TvIcons.Pause,
                    modifier = Modifier.focusRequester(playFocus),
                ) { ButtonLabel(stringResource(if (model.paused) R.string.action_play else R.string.action_pause)) }
                TonalButton(onClick = { onPanel(PlayerPanel.Episodes) }, icon = TvIcons.Episodes, modifier = Modifier.focusRequester(episodesFocus)) {
                    ButtonLabel(stringResource(R.string.player_episodes))
                }
                TonalButton(onClick = { onPanel(PlayerPanel.Tracks) }, icon = TvIcons.Subtitles) { ButtonLabel(stringResource(R.string.player_subtitles_and_audio)) }
                TonalButton(onClick = { onPanel(PlayerPanel.Sources) }, icon = TvIcons.Source) { ButtonLabel(stringResource(R.string.show_settings_source)) }
                TonalButton(onClick = { onPanel(PlayerPanel.Info) }, icon = TvIcons.Info) { ButtonLabel(stringResource(R.string.player_info)) }
                TonalButton(onClick = { onPanel(PlayerPanel.SkipTimes) }, icon = TvIcons.SkipNext) { ButtonLabel(stringResource(R.string.player_skip_times)) }
                Spacer(Modifier.weight(1f))
                if (hasNext) TonalButton(onClick = onNext, icon = TvIcons.SkipNext) { ButtonLabel(stringResource(R.string.player_next_episode)) }
            }
        }
    }
}

/**
 * What a screen reader gets of the seek bar: its name, the place in words, a range in 10 s steps
 * that TalkBack's adjust gesture moves, and the same two jumps as the remote's keys.
 */
@Composable
internal fun Modifier.seekSemantics(
    position: Double,
    duration: Double,
    headAt: Double?,
    onSeekTo: (Double) -> Unit,
    onSeekBy: (Double) -> Unit,
): Modifier {
    val context = LocalContext.current
    val name = stringResource(R.string.player_position)
    val back = stringResource(R.string.player_back_10)
    val forward = stringResource(R.string.player_forward_10)
    val at = (headAt ?: position).coerceIn(0.0, maxOf(duration, 0.0))
    val state = if (duration > 0) Strings.playerPositionState(context, spokenTime(at), spokenTime(duration)) else null
    return semantics(mergeDescendants = true) {
        contentDescription = name
        state?.let { stateDescription = it }
        if (duration > 0) {
            progressBarRangeInfo = ProgressBarRangeInfo(at.toFloat(), 0f..duration.toFloat(), maxOf(0, (duration / SEEK_STEP).toInt() - 1))
            setProgress { target ->
                onSeekTo(target.toDouble().coerceIn(0.0, duration))
                true
            }
            customActions = listOf(
                CustomAccessibilityAction(back) { onSeekBy(-SEEK_STEP); true },
                CustomAccessibilityAction(forward) { onSeekBy(SEEK_STEP); true },
            )
        }
    }
}

/** The step TalkBack's adjust gesture and the two custom actions move the bar by. */
private const val SEEK_STEP = 10.0

/**
 * The seek bar, 6 high: amber played on the rule colour, what is buffered ahead in silk at 20 %, and
 * the opening and ending in water on top of both, so they show behind the playhead too.
 * Focused, it shows a head that Left and Right move without seeking, and Center jumps to.
 */
@Composable
private fun SeekBar(
    position: Double,
    duration: Double,
    headAt: Double?,
    segments: List<SeekSegment>,
    buffered: List<TimeRange>,
    modifier: Modifier,
    onKey: (KeyEvent) -> Boolean,
    onSeekTo: (Double) -> Unit,
    onSeekBy: (Double) -> Unit,
    provisional: Pair<Double, Double>? = null,
) {
    val interactions = remember { MutableInteractionSource() }
    val focused by interactions.collectIsFocusedAsState()
    BoxWithConstraints(
        modifier
            .fillMaxWidth()
            .height(24.dp)
            .onKeyEvent(onKey)
            .seekSemantics(position, duration, headAt, onSeekTo, onSeekBy)
            .focusable(interactionSource = interactions),
        contentAlignment = Alignment.CenterStart,
    ) {
        val width = maxWidth
        Canvas(Modifier.fillMaxWidth().height(24.dp)) {
            // Focused, the line thickens, so the remote's focus shows on more than a dot.
            val bar = (if (focused) 10 else 6).dp.toPx()
            val top = (size.height - bar) / 2
            val radius = CornerRadius(bar / 2)
            drawRoundRect(ObiSilk.Edge, Offset(0f, top), Size(size.width, bar), radius)
            if (duration > 0) drawTimeline(position, duration, segments, buffered, top, bar)
            // The stretch being marked, at half water.
            if (duration > 0 && provisional != null) {
                val from = (provisional.first / duration).toFloat().coerceIn(0f, 1f) * size.width
                val to = (provisional.second / duration).toFloat().coerceIn(0f, 1f) * size.width
                if (to > from) drawRect(ObiSilk.Water.copy(alpha = 0.5f), Offset(from, top), Size(to - from, bar))
            }
        }
        if (duration > 0 && (focused || headAt != null)) {
            val at = ((headAt ?: position) / duration).toFloat().coerceIn(0f, 1f)
            Box(
                Modifier.offset(x = width * at - 8.dp)
                    .size(16.dp)
                    .background(ObiSilk.Silk, CircleShape),
            )
            if (headAt != null) {
                Text(
                    clockTime(headAt),
                    style = MaterialTheme.typography.labelLarge,
                    color = ObiSilk.Silk,
                    modifier = Modifier.offset(x = width * at - 24.dp, y = (-28).dp),
                )
            }
        }
    }
}

/**
 * A timeline's fill, over its rule-coloured track [bar] high from [top]: what is buffered ahead in
 * silk at 20 %, the played part in amber, and the opening and ending in water on top of both.
 */
private fun DrawScope.drawTimeline(position: Double, duration: Double, segments: List<SeekSegment>, buffered: List<TimeRange>, top: Float, bar: Float) {
    fun x(seconds: Double) = (seconds / duration).toFloat().coerceIn(0f, 1f) * size.width
    val played = x(position)
    buffered.forEach { range ->
        val start = maxOf(played, x(range.start))
        val end = x(range.end)
        if (end > start) drawRect(ObiSilk.Silk.copy(alpha = 0.2f), Offset(start, top), Size(end - start, bar))
    }
    drawRoundRect(ObiSilk.Amber, Offset(0f, top), Size(played, bar), CornerRadius(bar / 2))
    segments.filter { it.kind != SeekSegment.Kind.Plain }.forEach { segment ->
        val start = x(segment.start)
        val end = x(segment.end)
        if (end > start) drawRect(ObiSilk.Water, Offset(start, top), Size(end - start, bar))
    }
}

/** The running count beside a slim timeline at the bottom, while the chrome is hidden. */
@Composable
private fun SeekCount(model: PlayerModel, label: String) {
    val duration = model.duration ?: return
    Box(Modifier.fillMaxSize()) {
        Column(Modifier.align(Alignment.BottomStart).padding(start = 48.dp, end = 48.dp, bottom = 40.dp).fillMaxWidth()) {
            Text(label, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Silk)
            Spacer(Modifier.height(8.dp))
            Canvas(Modifier.fillMaxWidth().height(4.dp)) {
                drawRoundRect(ObiSilk.Edge, Offset.Zero, size, CornerRadius(size.height / 2))
                if (duration > 0) drawTimeline(model.position, duration, model.segments, model.buffered, 0f, size.height)
            }
        }
    }
}

/** The skip offer: silk while it holds Center, dimmed once it lets go. */
@Composable
private fun SkipPill(label: String, fresh: Boolean, modifier: Modifier) {
    Box(
        modifier
            .alpha(if (fresh) 1f else 0.6f)
            .background(if (fresh) ObiSilk.Silk else ObiSilk.RaisedSelected, CircleShape)
            .padding(horizontal = 20.dp, vertical = 12.dp),
    ) {
        Text(label, style = MaterialTheme.typography.labelLarge, color = if (fresh) ObiSilk.Ground else ObiSilk.Silk)
    }
}

/** A panel: the 400-wide column on the right; focus starts on the current item. */
@Composable
private fun Panel(model: PlayerModel, panel: PlayerPanel, onDone: () -> Unit, modifier: Modifier) {
    val store = LocalAppStore.current
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val current = remember { FocusRequester() }
    val list = remember { FocusRequester() }
    val title = stringResource(when (panel) {
        PlayerPanel.Episodes -> R.string.player_episodes
        PlayerPanel.Tracks -> R.string.player_subtitles_and_audio
        PlayerPanel.Sources -> R.string.show_settings_source
        PlayerPanel.Info -> R.string.player_info
        PlayerPanel.SkipTimes -> R.string.player_skip_times
    })
    Column(
        modifier
            .width(panelWidth())
            .panelFocus(title)
            .fillMaxHeight()
            .background(ObiSilk.RaisedHigh)
            .padding(start = 16.dp, end = PANEL_END, top = 27.dp, bottom = 27.dp),
    ) {
        Text(title, style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk, modifier = Modifier.padding(start = 16.dp, bottom = 12.dp).semantics { heading() })
        LazyColumn(Modifier.focusRequester(list).focusGroup(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            when (panel) {
                PlayerPanel.Episodes -> items(model.menus?.episodes.orEmpty(), key = { it.number }) { episode ->
                    TvListRow(
                        episode.title?.let { stringResource(R.string.tv_player_episode_row, episode.number, it) } ?: episode.label,
                        onClick = {
                            if (!episode.current) model.choose(PlayerChoice.Episode(episode.number))
                            onDone()
                        },
                        supporting = when {
                            episode.current -> stringResource(R.string.tv_player_episode_playing)
                            episode.watched -> stringResource(R.string.episode_watched)
                            else -> null
                        },
                        selected = episode.current,
                        modifier = if (episode.current) Modifier.focusRequester(current) else Modifier,
                    )
                }
                PlayerPanel.Tracks -> {
                    val audio = model.tracks.filter { it.kind == Track.Kind.Audio }
                    val subtitles = model.tracks.filter { it.kind == Track.Kind.Subtitle }
                    item(key = "audio") { PanelHeading(stringResource(R.string.player_audio)) }
                    items(audio, key = { "audio/${it.id}" }) { track ->
                        TvListRow(
                            track.name(LocalContext.current),
                            onClick = { model.selectAudio(track.id) },
                            selected = track.selected,
                            modifier = if (track.selected) Modifier.focusRequester(current) else Modifier,
                        )
                    }
                    item(key = "subtitles") { PanelHeading(stringResource(R.string.player_subtitles)) }
                    item(key = "subtitles/off") {
                        val off = subtitles.none { it.selected }
                        TvListRow(stringResource(R.string.value_off), onClick = { model.selectSubtitle(null) }, selected = off)
                    }
                    items(subtitles, key = { "sub/${it.id}" }) { track ->
                        TvListRow(track.name(LocalContext.current), onClick = { model.selectSubtitle(track.id) }, selected = track.selected)
                    }
                    item(key = "style/heading") { PanelHeading(stringResource(R.string.player_subs_style)) }
                    subtitleStyleRows(store, settings?.subtitleStyle)
                }
                PlayerPanel.SkipTimes -> skipTimesRows(model, current, onDone)
                PlayerPanel.Info -> item(key = "info") { InfoPanel(model, Modifier.focusRequester(current)) }
                PlayerPanel.Sources -> items(model.menus?.sources.orEmpty(), key = { it.id }) { source ->
                    TvListRow(
                        source.label,
                        onClick = {
                            if (!source.current) model.choose(PlayerChoice.Source(source.id))
                            onDone()
                        },
                        supporting = source.detail,
                        selected = source.current,
                        modifier = if (source.current) Modifier.focusRequester(current) else Modifier,
                    )
                }
            }
        }
    }
    LaunchedEffect(panel) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (current.tryFocus()) return@LaunchedEffect
        }
        // Nothing current (subtitles off, an empty list): the panel's first row.
        list.tryFocus()
    }
}

@Composable
private fun PanelHeading(text: String) {
    Text(text, style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze, modifier = Modifier.padding(start = 16.dp, top = 12.dp, bottom = 4.dp))
}

/** A corner card or pill, faded in and out on the tokens, or at once with Remove animations. */
@Composable
private fun Fading(visible: Boolean, modifier: Modifier, content: @Composable () -> Unit) {
    AnimatedVisibility(visible, modifier, enter = enterFade(), exit = exitFade()) { content() }
}

/**
 * The stream stalled: "Source is buffering · 12 s" counting, that another stream may play without
 * waiting, and Switch source focused beside Keep waiting. Back keeps waiting.
 */
@Composable
private fun StallPrompt(model: PlayerModel, stall: app.kaeri.player.StallCard, switch: FocusRequester) {
    val stallPane = stringResource(R.string.player_stall_detail)
    Column(
        Modifier
            // It comes up in place of the hidden root, which holds focus: taken through the card,
            // so Switch source is drawn focused, not only focused.
            .entryFocus(switch)
            .width(UP_NEXT_WIDTH)
            // Said once as it comes up; the counting title is not read again each second.
            .semantics { paneTitle = stallPane }
            .background(ObiSilk.RaisedHigh, RoundedCornerShape(16.dp))
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(stringResource(R.string.player_stall_title, stall.held(model.clock).toInt()), style = MaterialTheme.typography.titleMedium, color = ObiSilk.Silk)
        Text(stringResource(R.string.player_stall_detail), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze)
        Row(Modifier.padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TonalButton(onClick = { model.switchSource() }, modifier = Modifier.focusRequester(switch)) { ButtonLabel(stringResource(R.string.player_switch_source)) }
            TonalButton(onClick = { model.keepWaiting() }) { ButtonLabel(stringResource(R.string.player_keep_waiting)) }
        }
    }
}

/**
 * TVPlayerUpNext: the next episode bottom right as the ending starts, its still (the spoiler rule:
 * blurred, as it is not watched yet), "Plays in 8 s" over the bar, Play now focused and Cancel.
 */
@Composable
private fun UpNextCard(model: PlayerModel, next: NextEpisode, play: FocusRequester, modifier: Modifier) {
    LaunchedEffect(next.number) {
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (play.tryFocus()) return@LaunchedEffect
        }
    }
    val upNextPane = stringResource(R.string.player_up_next)
    Row(
        modifier
            .width(UP_NEXT_WIDTH)
            .semantics { paneTitle = upNextPane }
            .background(ObiSilk.RaisedHigh, RoundedCornerShape(16.dp))
            .padding(12.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        ArtImage(
            next.thumbnail,
            plate = ObiSilk.Raised,
            maxPixelSize = 368,
            spoiler = true,
            modifier = Modifier.size(184.dp, 104.dp).clip(RoundedCornerShape(8.dp)),
        )
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                next.title?.let { stringResource(R.string.tv_player_up_next_line, next.number, it) } ?: stringResource(R.string.tv_player_up_next_bare, next.number),
                style = TvTitle.player,
                color = ObiSilk.Silk,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            model.countdown?.let { (left, progress) ->
                Text(stringResource(R.string.player_plays_in, left.toInt()), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Haze)
                Box(Modifier.fillMaxWidth().height(4.dp).background(ObiSilk.Edge, CircleShape)) {
                    Box(Modifier.fillMaxWidth(progress).fillMaxHeight().background(ObiSilk.Amber, CircleShape))
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TonalButton(onClick = { model.playNext() }, modifier = Modifier.focusRequester(play)) { ButtonLabel(stringResource(R.string.action_play_now)) }
                TonalButton(onClick = { model.cancelNext() }) { ButtonLabel(stringResource(R.string.action_cancel)) }
            }
        }
    }
}

private val UP_NEXT_WIDTH = 560.dp

/** "Still watching?" after episodes in a row with nobody there: paused, Continue focused. */
@Composable
private fun StillWatchingDialog(model: PlayerModel) {
    androidx.compose.ui.window.Dialog(onDismissRequest = { model.continueWatching() }) {
        val keep = remember { FocusRequester() }
        val stillWatchingPane = stringResource(R.string.player_still_watching)
        Column(
            Modifier
                .entryFocus(keep)
                .width(480.dp)
                .semantics { paneTitle = stillWatchingPane }
                .background(ObiSilk.RaisedHigh, RoundedCornerShape(28.dp))
                .padding(32.dp),
            verticalArrangement = Arrangement.spacedBy(24.dp),
        ) {
            Text(stringResource(R.string.player_still_watching), style = MaterialTheme.typography.headlineSmall, color = ObiSilk.Silk)
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                TonalButton(onClick = { model.continueWatching() }, modifier = Modifier.focusRequester(keep)) { ButtonLabel(stringResource(R.string.nav_continue)) }
                TonalButton(onClick = { model.stopWatching() }) { ButtonLabel(stringResource(R.string.action_stop)) }
            }
        }
    }
}

/** TVPlayerInfo: the episode, then how it plays, where heavy subtitles show their cost. */
@Composable
private fun InfoPanel(model: PlayerModel, modifier: Modifier) {
    val store = LocalAppStore.current
    val episodes by store.titles.episodes.collectAsStateWithLifecycle()
    val number = model.episode?.number
    val row = model.titleId?.let { episodes[it] }?.firstOrNull { it.number == number }
    Column(
        // Focusable, so Back and the D-pad stay in the panel.
        modifier.focusable().padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(model.episodeLine(LocalContext.current), style = TvTitle.player, color = ObiSilk.Silk)
        row?.description?.takeIf { it.isNotBlank() }?.let {
            Text(it, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 8, overflow = TextOverflow.Ellipsis)
        }
        Text(stringResource(R.string.player_stats), style = MaterialTheme.typography.labelLarge, color = ObiSilk.Haze, modifier = Modifier.padding(top = 12.dp))
        // The engine's lines, the same every app shows; until it knows the picture, says so.
        if (model.statsSummary.isEmpty()) {
            Text(stringResource(R.string.player_stats_waiting), style = MaterialTheme.typography.labelMedium, color = ObiSilk.Haze)
        }
        model.statsSummary.forEach { line ->
            Text(line, style = MaterialTheme.typography.labelMedium, color = ObiSilk.Silk)
        }
    }
}

/**
 * The wall clock, moved on as each minute turns: the chrome's clock and "Ends at" keep the time
 * themselves, as the player's own clock rests while it is paused.
 */
@Composable
private fun wallMinute(): java.time.Instant {
    val now by produceState(AppClock.now()) {
        while (true) {
            val at = AppClock.now()
            value = at
            delay(60_000L - at.toEpochMilli().mod(60_000L))
        }
    }
    return now
}
