// The phone's reader: a chapter's pages or strip over the engine's reader session, with the one
// chrome group the board draws at the thumb (scrubber with its preview, Close, the series and
// chapter, Chapters and Aa), the notices, the opening and failed states, the end page and the
// sheets. Paper mode's parts act here: pages at once, the paper look by day, the dimmed page. The engine decides what is shown and when; this
// draws it. The status bar goes with the chrome.

package app.kaeri.reader.phone

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Stable
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntOffset
import app.kaeri.engine.model.ChapterVersion
import app.kaeri.engine.model.ReaderWait
import app.kaeri.reader.PageFileView
import app.kaeri.reader.StripDividerHeight
import app.kaeri.reader.endSkipped
import app.kaeri.reader.onUnkeptCredit
import app.kaeri.reader.paged
import app.kaeri.reader.partMissingAt
import app.kaeri.reader.partStarting
import app.kaeri.reader.partTicks
import app.kaeri.reader.shownCredit
import app.kaeri.reader.shownPage
import app.kaeri.reader.shownPages
import app.kaeri.reader.startSkipped
import app.kaeri.reader.CreditShot
import app.kaeri.reader.openCredits
import app.kaeri.engine.core.ChapterId
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import kotlin.math.abs
import kotlin.math.roundToInt
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.SliderState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.isTraversalGroup
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.traversalIndex
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.LifecycleOwner
import app.kaeri.design.Motion
import app.kaeri.design.SwapFade
import app.kaeri.design.atFullSpeed
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.design.robotoFlex
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderAction as NoticeAction
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.model.TurnHow
import app.kaeri.i18n.Strings
import app.kaeri.reader.LocalPageTone
import app.kaeri.reader.PageImages
import app.kaeri.reader.PageWaits
import app.kaeri.reader.openingClock
import app.kaeri.reader.PagedReader
import app.kaeri.reader.ReaderAction
import app.kaeri.reader.ReaderKeyRules
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.ShownNotice
import app.kaeri.reader.StripReader
import app.kaeri.reader.TapGuide
import app.kaeri.reader.TapGuideLayer
import app.kaeri.reader.ShowTapGuideWhenDue
import app.kaeri.reader.actionAt
import app.kaeri.reader.clearsTapGuide
import app.kaeri.reader.rememberScreenReader
import app.kaeri.reader.tapReading
import app.kaeri.reader.tapZones
import app.kaeri.engine.model.TapAction
import app.kaeri.engine.model.TapMap
import app.kaeri.engine.model.TapReading
import app.kaeri.engine.model.TapZoneSettings
import app.kaeri.engine.model.tapMap
import app.kaeri.engine.model.tapScrollStep
import app.kaeri.reader.onKeyDown
import app.kaeri.reader.readerKeys
import app.kaeri.reader.takesKeyUp
import app.kaeri.reader.readerSurface
import app.kaeri.reader.rememberPagedReaderState
import app.kaeri.reader.rightToLeft
import app.kaeri.reader.text
import app.kaeri.store.text
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull

/** What the phone reader leaves to the app around it. */
class PhoneReaderActions(
    val onClose: () -> Unit,
    val onSeries: () -> Unit = onClose,
    val onDownloads: () -> Unit = onClose,
    /** Add a source, from the end of the demo. */
    val onAddSource: () -> Unit = onClose,
)

/** The setting [key]'s current choice name, or null. */
internal fun ReaderState.choice(key: ReaderSettingKey): String? =
    (settings?.rows?.firstOrNull { it.key == key }?.value as? ReaderValue.Choice)?.name

internal fun ReaderState.toggle(key: ReaderSettingKey): Boolean =
    (settings?.rows?.firstOrNull { it.key == key }?.value as? ReaderValue.Toggle)?.on == true

private enum class Sheet { None, Chapters, Settings }

private tailrec fun Context.activity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.activity()
    else -> null
}

/**
 * The phone reader for [session]. Pages and strips are drawn here; a novel is drawn by [novel],
 * which the app supplies with the engine's book server.
 */
@Composable
fun PhoneReader(
    session: ReaderSession,
    state: ReaderState,
    images: PageImages,
    actions: PhoneReaderActions,
    novel: @Composable (place: NovelPlace, onControls: () -> Unit) -> Unit,
    modifier: Modifier = Modifier,
    quiet: QuietPhone? = null,
    /** The series' cover, drawn dimmed where the page goes while the chapter opens. */
    cover: (@Composable (Modifier) -> Unit)? = null,
    /** A settings page asked for from outside, shown once the reader has a chapter. */
    settingsAsked: SettingsPane? = null,
    onSettingsShown: () -> Unit = {},
    /** Off while the reader leaves: touches go nowhere. */
    enabled: Boolean = true,
) {
    val context = LocalContext.current
    val view = state.view
    val density = LocalDensity.current
    val direction = view?.direction ?: ReadDirection.RIGHT_TO_LEFT
    val paper = state.toggle(ReaderSettingKey.PAPER_MODE)
    val novelOpen = state.novel != null || view?.medium == Medium.NOVEL
    val systemDark = isSystemInDarkTheme()
    // Paper mode's parts, each acting only while it is on.
    val paperDay = paper && state.toggle(ReaderSettingKey.PAPER_LOOK) && !systemDark
    val instant = paper && state.toggle(ReaderSettingKey.PAPER_INSTANT)
    // The chrome turns light with the Paper, Light and Sepia looks, and with Paper mode by day.
    val light = when {
        paperDay -> true
        novelOpen -> when (state.choice(ReaderSettingKey.LOOK)) {
            "paper", "light", "sepia" -> true
            "system" -> !systemDark
            else -> false
        }
        else -> false
    }
    val colours = if (light) LightChrome else DarkChrome
    var chrome by rememberSaveable { mutableStateOf(false) }
    var sheet by rememberSaveable { mutableStateOf(Sheet.None) }
    var settingsStart by remember { mutableStateOf(SettingsPane.Main) }
    val pager = rememberPagedReaderState()
    val volume = state.toggle(ReaderSettingKey.VOLUME_KEYS_TURN) || paper && state.toggle(ReaderSettingKey.PAPER_VOLUME)
    val place = remember { NovelPlace() }
    // The ring on the scrubber where a jump left from; it stays until the chapter closes.
    val scrub = remember(view?.chapter?.id) { ScrubState() }

    // The Quiet rule's settings link: the Paper page, once there is a chapter to show it over.
    LaunchedEffect(settingsAsked, view != null) {
        val pane = settingsAsked ?: return@LaunchedEffect
        if (view == null) return@LaunchedEffect
        settingsStart = pane
        sheet = Sheet.Settings
        onSettingsShown()
    }

    // While TalkBack runs the chrome stays shown, and the tap zones are off.
    val exploring = rememberScreenReader()
    val shown = chrome || exploring

    // The engine's tap zones for what is open, and their guide.
    val zones = state.tapZones()
    val reading = state.tapReading(zoomed = pager.zoomed, screenReader = exploring)
    val guide = remember { TapGuide() }
    fun guideSeen(map: TapMap) {
        if (!exploring) session.tapGuideSeen(map)
    }
    // The guide is drawn over the whole page, so the controls go when it comes, as Show zones has it.
    LaunchedEffect(guide.up) { if (guide.up) chrome = false }

    // A Fixtures shot's step, once the credit pages or parts it staged have come.
    val creditShot = CreditShot.step
    val staged = state.openCredits.isNotEmpty() || state.parts != null
    val stripList = rememberLazyListState()
    LaunchedEffect(creditShot, staged) {
        if (creditShot == null || !staged) return@LaunchedEffect
        delay(600)
        val now = session.state.value
        val chapter = now.chapter ?: return@LaunchedEffect
        // Without the fixture's resume notice, as a chapter opened fresh.
        now.notice?.let { session.dismissNotice(it.id) }
        when (creditShot) {
            "line" -> if (now.paged) pager.go(0, TurnHow.JUMP) else stripList.scrollToItem(0)
            "chrome", "parts" -> {
                if (creditShot == "parts") now.partTicks.firstOrNull()?.let { pager.go(now.leafOf((it.page + 2).toUInt()) ?: 0, TurnHow.JUMP) } else pager.go(0, TurnHow.JUMP)
                chrome = true
            }
            "shown", "shownchrome", "kept" -> {
                if (now.paged) {
                    session.showStartCredits()?.let { pager.show(it, TurnHow.TURN, session::showCredit, null) }
                } else {
                    session.reveal(now.startSkipped, chapter)
                    stripList.scrollToItem(0)
                }
                delay(400)
                if (creditShot == "kept") session.keepCredit()
                if (creditShot != "shown") chrome = true
            }
            "end" -> pager.go(now.leaves.lastIndex, TurnHow.JUMP)
        }
    }
    // A demo work opens with the chrome and its credit held for 3 s; the credit returns with the
    // chrome on every tap.
    val demo = view?.demoCredit
    LaunchedEffect(demo != null, view?.chapter?.id) {
        if (demo == null) return@LaunchedEffect
        chrome = true
        delay(DEMO_CREDIT_HOLD_MILLIS)
        chrome = false
    }

    SystemBars(context, shown, light)

    val quietOn = state.toggle(ReaderSettingKey.QUIET_PHONE)
    val owner = remember { context.activity() as? LifecycleOwner }
    var started by remember { mutableStateOf(true) }
    var askedAccess by remember { mutableStateOf(false) }
    var quietDenied by remember { mutableStateOf(false) }
    DisposableEffect(owner) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START -> started = true
                Lifecycle.Event.ON_STOP -> {
                    started = false
                    // A kept credit page reaches the engine before the app can be killed in the background.
                    session.commitKeep()
                }
                Lifecycle.Event.ON_RESUME -> if (askedAccess) {
                    askedAccess = false
                    // Back from the access page: on when it was granted, otherwise it stays off and says why.
                    if (quiet?.allowed == true) {
                        quietDenied = false
                        session.setSetting(ReaderSettingKey.QUIET_PHONE, SettingScope.DEVICE, ReaderValue.Toggle(true))
                    } else {
                        quietDenied = true
                    }
                }
                else -> {}
            }
        }
        owner?.lifecycle?.addObserver(observer)
        onDispose { owner?.lifecycle?.removeObserver(observer) }
    }
    // The rule ends when the reader stops or closes, and starts again when it is back.
    LaunchedEffect(quietOn, quiet, started) { quiet?.set(quietOn && started) }
    DisposableEffect(quiet) { onDispose { quiet?.set(false) } }
    var localNote by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(quietOn) {
        if (quietOn && quiet != null && quiet.supported && quiet.allowed && !quiet.noticeSeen) {
            quiet.noticeSeen = true
            localNote = Strings.readerQuietNotice(context)
            delay(4000)
            localNote = null
        }
    }
    fun toggleSetting(key: ReaderSettingKey, on: Boolean) {
        if (key == ReaderSettingKey.QUIET_PHONE && on) {
            if (quiet == null || !quiet.supported) return
            if (!quiet.allowed) {
                askedAccess = true
                runCatching { context.startActivity(quiet.accessIntent()) }
                return
            }
        }
        session.setSetting(key, SettingScope.DEVICE, ReaderValue.Toggle(on))
    }

    fun act(action: ReaderAction) {
        // A key clears the guide as a touch does, and still does what it asks.
        guide.clear()?.let(::guideSeen)
        // The last page of the chapter's story, before its end page.
        val last = state.leaves.indexOfLast { !it.endPage }.coerceAtLeast(0)
        when (action) {
            // On the end page a forward key crosses to the next chapter, as its hint says.
            ReaderAction.FORWARD -> when {
                novelOpen && place.ended -> session.crossOn()
                novelOpen -> place.turn(true)
                else -> pager.forward(TurnHow.KEY)
            }
            ReaderAction.BACK -> when {
                novelOpen && place.ended -> place.ended = false
                novelOpen -> place.turn(false)
                else -> pager.back(TurnHow.KEY)
            }
            ReaderAction.CONTROLS -> chrome = !chrome
            ReaderAction.CLEAN -> {}
            ReaderAction.FIRST -> if (novelOpen) place.go(0) else pager.go(0, TurnHow.JUMP)
            ReaderAction.LAST -> if (novelOpen) place.go((place.pages - 1).coerceAtLeast(0)) else pager.go(last, TurnHow.JUMP)
            ReaderAction.ESCAPE -> when {
                sheet != Sheet.None -> sheet = Sheet.None
                chrome -> chrome = false
                else -> actions.onClose()
            }
            ReaderAction.GO_TO -> chrome = true
        }
    }
    val keyRules = ReaderKeyRules(direction, volumeTurns = volume, volumeDownForward = !ReaderLook.volumeSwapped(context))
    // The novel's web view hears keys first and hands the reader's own over.
    SideEffect {
        place.keys = { event ->
            when (event.action) {
                android.view.KeyEvent.ACTION_DOWN -> keyRules.onKeyDown(event)?.let { answer ->
                    answer.action?.let(::act)
                    true
                } ?: false
                android.view.KeyEvent.ACTION_UP -> keyRules.takesKeyUp(event.keyCode)
                else -> false
            }
        }
    }

    // Back closes the reader, even with the chrome up; it follows the finger toward the series.
    var backProgress by remember { mutableFloatStateOf(0f) }
    PredictiveBackHandler(enabled = sheet == Sheet.None) { progress ->
        try {
            progress.collect { backProgress = it.progress }
            actions.onClose()
        } catch (e: CancellationException) {
            backProgress = 0f
            throw e
        }
    }

    val background = when {
        novelOpen -> colours.ground
        paperDay -> Chrome.paper
        else -> when (state.choice(ReaderSettingKey.BACKGROUND)) {
            "black" -> Color.Black
            "white" -> Color.White
            else -> colours.ground
        }
    }
    // Paper mode dims the page's white instead of tinting the page: by a third of Dim the page.
    val dim = if (paper) (state.amount(ReaderSettingKey.PAPER_DIM) ?: 0.3).toFloat().coerceIn(0f, 1f) else 0f
    val tone = remember(dim) {
        val scale = 1f - dim / 3f
        if (dim > 0f) ColorFilter.colorMatrix(ColorMatrix().apply { setToScale(scale, scale, scale - 0.02f, 1f) }) else null
    }
    val fades = !instant

    CompositionLocalProvider(LocalChrome provides colours, LocalPageTone provides tone) {
      Box(modifier.fillMaxSize().then(if (enabled) Modifier else Modifier.pointerInput(Unit) { awaitPointerEventScope { while (true) awaitPointerEvent(PointerEventPass.Initial).changes.forEach { it.consume() } } })) {
        Box(
            Modifier
                .fillMaxSize()
                .graphicsLayer {
                    scaleX = 1f - 0.1f * backProgress
                    scaleY = 1f - 0.1f * backProgress
                    alpha = 1f - 0.2f * backProgress
                }
                .background(background)
                .readerSurface(density, eink = false, dark = !light) { surface ->
                    if (state.view == null && !state.opening) return@readerSurface
                    session.surface(surface)
                }
                .readerKeys(keyRules, ::act)
                // The guide's first touch clears it, and still turns, scrolls or opens the controls.
                .clearsTapGuide(guide, ::guideSeen),
        ) {
            val failure = state.failure
            val strip = view?.mode == ReaderMode.STRIP || view?.mode == ReaderMode.VERTICAL
            val scope = rememberCoroutineScope()
            var showingAbove by remember(view?.chapter?.id) { mutableStateOf(false) }
            var openingHeld by remember { mutableStateOf(true) }
            var plateGone by remember { mutableStateOf(false) }
            LaunchedEffect(state.leaves.isEmpty()) {
                if (state.leaves.isEmpty()) {
                    openingHeld = true
                    plateGone = false
                }
            }
            // Leaving a chapter from its end page, by Continue, a swipe or the chapters list, holds that
            // end page over the next chapter until its first page is drawn, then fades it away: the same
            // crossfade as a turn, never a cut and never the empty ground. A first page slower than the
            // plate's wait hands over to the opening plate instead, with the same fade.
            val here = view?.chapter?.id
            var endShown by remember { mutableStateOf<ReaderState?>(null) }
            var landed by remember { mutableStateOf<Any?>(null) }
            val handoff = remember { Animatable(0f) }
            val removed = rememberAnimationsRemoved()
            val leaving = endShown?.takeIf { here != null && it.view?.chapter?.id != here }
            if (leaving == null && !strip && !novelOpen && here != null && pager.chapter == here && state.leaves.isNotEmpty()) {
                val atEnd = state.leaves.getOrNull(pager.leaf)?.endPage == true
                SideEffect { endShown = if (atEnd) state else null }
            }
            LaunchedEffect(leaving?.view?.chapter?.id, here) {
                if (leaving == null) return@LaunchedEffect
                landed = null
                withTimeoutOrNull(PageWaits.PLATE_MS + 100) { snapshotFlow { landed == here }.first { it } }
                val fade: suspend () -> Unit = { handoff.animateTo(1f, Motion.quick()) }
                when {
                    !fades -> handoff.snapTo(1f)
                    removed -> atFullSpeed { fade() }
                    else -> fade()
                }
                endShown = null
                handoff.snapTo(0f)
            }
            when {
                failure != null && state.leaves.isEmpty() && state.novel == null -> {
                    val row = state.openingChapter ?: view?.chapter
                    ChapterFailed(
                        title = Strings.readerFailedChapter(context, row?.let(::chapterNumber) ?: ""),
                        line = failure.text(context),
                        state = state,
                        session = session,
                        onClose = actions.onClose,
                        onRetry = session::retryOpen,
                        instant = instant,
                    )
                }
                novelOpen && view != null -> {
                    novel(place) { chrome = !chrome }
                    // The text comes from the source after the chapter opens: the opening plate, with
                    // what it waits on and for how long, stays over the page until the first one shows.
                    var novelPlateGone by remember(view.chapter.id) { mutableStateOf(false) }
                    if (!novelPlateGone && place.failed == null) {
                        Opening(state, done = place.ready, drawn = place.ready, fades = fades && leaving == null, onGone = { novelPlateGone = true }, session = session, instant = instant, onClose = actions.onClose)
                    }
                    val failed = place.failed
                    when {
                        failed != null -> Box(Modifier.fillMaxSize().background(colours.ground)) {
                            ChapterFailed(
                                title = Strings.readerFailedNovel(context),
                                line = failed,
                                state = state,
                                session = session,
                                onClose = actions.onClose,
                                onRetry = place::retry,
                                instant = instant,
                            )
                        }
                        place.ended -> NovelEndPage(state, session, actions, images, direction, zones, reading, onBack = { place.ended = false }, cover = cover, onFirstPage = {
                            place.ended = false
                            place.go(0)
                        })
                    }
                }
                strip && view != null && (state.leaves.isNotEmpty() || state.previous != null) -> {
                    val gap = if (state.choice(ReaderSettingKey.STRIP_GAP) == "small") 8.dp else 0.dp
                    // Next and Previous scroll by the engine's step of the visible height, at once with
                    // Remove animations or in Paper mode; Off, the default, shows the controls.
                    fun scrollStep(down: Boolean) {
                        val by = stripList.layoutInfo.viewportSize.height * tapScrollStep().toFloat() * (if (down) 1f else -1f)
                        scope.launch { if (fades && !removed) stripList.animateScrollBy(by, Motion.standard()) else stripList.scrollBy(by) }
                    }
                    val scrollDown = Strings.readerZoneScrollDown(context)
                    val scrollUp = Strings.readerZoneScrollUp(context)
                    val showControls = Strings.readerActionControls(context)
                    StripReader(
                        state = state,
                        images = images,
                        onAt = { at, foot -> session.stripAt(at, foot) },
                        onTap = { x, y ->
                            when (reading?.let { zones.actionAt(it, x, y) } ?: TapAction.CONTROLS) {
                                TapAction.NEXT -> scrollStep(true)
                                TapAction.PREVIOUS -> scrollStep(false)
                                TapAction.CONTROLS -> chrome = !chrome
                            }
                        },
                        // TalkBack: the strip offers its moves as actions.
                        modifier = Modifier.semantics {
                            customActions = listOf(
                                CustomAccessibilityAction(scrollDown) { scrollStep(true); true },
                                CustomAccessibilityAction(scrollUp) { scrollStep(false); true },
                                CustomAccessibilityAction(showControls) { chrome = !chrome; true },
                            )
                        },
                        onScrolled = { chrome = false },
                        onMissing = session::needPage,
                        onRetry = { session.retry(it.toUInt()) },
                        onJumped = session::jumped,
                        footer = { PhoneEndPage(state, session, actions, images, onFirstPage = { session.startOver() }, cover = cover, pageGround = background, onShowCredits = { session.showEndCredits() }) },
                        divider = { next -> StripDivider(state, next, onShowCredits = { session.showEndCredits(it) }) },
                        credits = { chapter, pages -> StripCredits(state, session, images, chapter, pages, background) },
                        onFlow = { next -> session.flowInto(next.id) },
                        onShowing = { showingAbove = it },
                        list = stripList,
                        gap = gap,
                    )
                }
                else -> {
                    // The opening plate stays under the first page until it is drawn, so the page fades in over
                    // it, and only then fades itself: the ground never shows between the two.
                    // Under a held end page the plate goes at once when the page is drawn: it is covered then.
                    if (state.leaves.isEmpty() || openingHeld || !plateGone) {
                        Opening(
                            state,
                            done = state.leaves.isNotEmpty(),
                            drawn = !openingHeld,
                            fades = fades && leaving == null,
                            onGone = { plateGone = true },
                            session = session,
                            instant = instant,
                            onClose = actions.onClose,
                        )
                    }
                    if (state.leaves.isNotEmpty()) PagedReader(
                    state = state,
                    pager = pager,
                    images = images,
                    direction = direction,
                    taps = zones,
                    reading = reading ?: TapReading(view?.mode ?: ReaderMode.PAGED, direction, false, exploring),
                    onControls = { chrome = !chrome },
                    onShown = { leaf, how, turned ->
                        session.shown(leaf, how)
                        // A turn takes the chrome away; the opening page and a new layout of the same
                        // place (after a rotation) leave it as it is.
                        if (turned && how != TurnHow.JUMP) chrome = false
                    },
                    onMissing = session::needPage,
                    onRetry = { session.retry(it.toUInt()) },
                    onJumped = session::jumped,
                    endPage = {
                        PhoneEndPage(
                            state, session, actions, images,
                            onFirstPage = { pager.go(0, TurnHow.JUMP) },
                            cover = cover,
                            pageGround = background,
                            onShowCredits = { session.showEndCredits()?.let { pager.show(it, TurnHow.TURN, session::showCredit, state.shownCredit) } },
                        )
                    },
                    onPastEnd = session::crossOn,
                    motion = fades,
                    onReady = {
                        openingHeld = false
                        landed = here
                    },
                    fit = state.choice(ReaderSettingKey.FIT),
                    onCredit = session::showCredit,
                    creditGround = background,
                    )
                    if (leaving != null) {
                        Box(
                            Modifier
                                .fillMaxSize()
                                .graphicsLayer { alpha = 1f - handoff.value }
                                .pointerInput(Unit) { awaitPointerEventScope { while (true) awaitPointerEvent(PointerEventPass.Initial).changes.forEach { it.consume() } } }
                                .clearAndSetSemantics {},
                        ) {
                            PhoneEndPage(leaving, session, actions, images, cover = cover, pageGround = background)
                        }
                    }
                }
            }

            // The tap zone guide, over the page and under the controls: once the page is drawn and
            // nothing covers it, when the engine says it is due.
            val pageUp = when {
                novelOpen -> place.ready && !place.ended && place.failed == null
                strip -> state.leaves.isNotEmpty()
                else -> state.leaves.isNotEmpty() && !openingHeld && leaving == null
            }
            ShowTapGuideWhenDue(
                guide,
                zones = state.settings?.tapZones,
                reading = reading,
                ready = view != null && failure == null && pageUp && sheet == Sheet.None,
            )
            TapGuideLayer(guide, fades)

            if (view != null) {
                AnimatedVisibility(
                    visible = shown,
                    enter = if (fades) fadeIn(Motion.fadeIn()) else EnterTransition.None,
                    exit = if (fades) fadeOut(Motion.fadeOut()) else ExitTransition.None,
                    modifier = Modifier.align(Alignment.BottomCenter),
                ) {
                    // Scrolled back into the chapter kept above, the line names that one.
                    val above = state.previous?.view?.chapter?.takeIf { showingAbove }
                    ChromeGroup(
                        state = state,
                        title = view.title,
                        line = when {
                            novelOpen -> novelLine(context, state)
                            // A credit page shows in place of a number until it is kept: "Chapter 87 · credit page".
                            !strip && state.onUnkeptCredit -> Strings.readerGroupLinePlace(context, chapterWords(context, view.chapter), creditPlaceWords(context, state).orEmpty())
                            else -> chapterWords(context, above ?: view.chapter)
                        },
                        scrubber = when {
                            novelOpen -> { -> NovelScrubber(state, place) }
                            strip -> null
                            else -> { -> Scrubber(state, direction.rightToLeft && view.mode == ReaderMode.PAGED, pager.leaf, scrub) { pager.go(it, TurnHow.JUMP) } }
                        },
                        backTo = state.backTo,
                        onBackTo = session::backTo,
                        rtl = direction.rightToLeft,
                        fades = fades,
                        chaptersOpen = sheet == Sheet.Chapters,
                        settingsOpen = sheet == Sheet.Settings,
                        onClose = actions.onClose,
                        onChapters = { sheet = Sheet.Chapters },
                        onSettings = {
                            settingsStart = SettingsPane.Main
                            sheet = Sheet.Settings
                        },
                        modifier = Modifier.onSizeChanged { scrub.groupHeight = it.height },
                    )
                }
                // The page under the thumb while the scrubber is dragged, 96 dp wide above the group.
                if (shown && !novelOpen && !strip) ScrubPreview(session, state, images, scrub, fades)
                // A demo work's credit, at the top centre with the chrome; not over its end card.
                val onEnd = state.leaves.getOrNull(pager.leaf)?.endPage == true || place.ended
                AnimatedVisibility(
                    visible = demo != null && shown && sheet == Sheet.None && !onEnd,
                    enter = if (fades) fadeIn(Motion.reveal()) else EnterTransition.None,
                    exit = if (fades) fadeOut(Motion.reveal()) else ExitTransition.None,
                    modifier = Modifier.align(Alignment.TopCenter),
                ) {
                    val held = remember { mutableStateOf(demo) }
                    if (demo != null) held.value = demo
                    held.value?.let { CreditCapsule(it) }
                }
                // Page 1's line: the credit pages skipped before it, once each time the chapter
                // opens and for 4 s once the page has settled, and again with the chrome. It leaves
                // with the turn.
                val skipped = state.startSkipped
                var lineUp by remember(view.chapter.id) { mutableStateOf(false) }
                var lineSaid by remember(view.chapter.id) { mutableStateOf(false) }
                val onFirst = state.paged && pager.leaf == 0 && state.onCredit == null && state.leaves.isNotEmpty() && !openingHeld
                LaunchedEffect(view.chapter.id, onFirst, skipped.isNotEmpty()) {
                    if (!onFirst) {
                        lineUp = false
                        return@LaunchedEffect
                    }
                    if (lineSaid || skipped.isEmpty()) return@LaunchedEffect
                    lineSaid = true
                    lineUp = true
                    // A Fixtures shot of the line holds it.
                    if (CreditShot.step == "line") return@LaunchedEffect
                    delay(CREDIT_LINE_MILLIS)
                    lineUp = false
                }
                CreditFade(
                    visible = onFirst && skipped.isNotEmpty() && (lineUp || shown) && sheet == Sheet.None && state.notice == null && demo == null,
                    fades = fades,
                    modifier = Modifier.align(Alignment.TopCenter),
                ) {
                    val count = remember { mutableIntStateOf(1) }
                    if (skipped.isNotEmpty()) count.intValue = skipped.size
                    CreditLineChip(count.intValue) {
                        lineUp = false
                        chrome = false
                        session.showStartCredits()?.let { pager.show(it, TurnHow.TURN, session::showCredit, state.shownCredit) }
                    }
                }
            }

            // Notices sit at the top centre while the chrome is away, as the board has them. A novel's
            // text runs up to the top margin, so its notices take the bottom margin in place of the
            // pages left.
            val notice = state.notice
            val clear = !shown && sheet == Sheet.None
            val plated = notice != null && clear

            // At rest a novel's only line: the pages left in the chapter, in the bottom margin.
            if (novelOpen && !shown && place.failed == null && !place.ended && !plated) PagesLeft(state, place)

            AnimatedVisibility(
                visible = plated,
                enter = if (fades) fadeIn(Motion.fadeIn()) else EnterTransition.None,
                exit = if (fades) fadeOut(Motion.fadeOut()) else ExitTransition.None,
                modifier = Modifier.align(if (novelOpen) Alignment.BottomCenter else Alignment.TopCenter),
            ) {
                val held = remember { mutableStateOf<ShownNotice?>(null) }
                if (notice != null) held.value = notice
                held.value?.let { n ->
                    NoticePlate(n.notice.msg.text(context), actionWords(context, n), Modifier, atFoot = novelOpen) {
                        n.notice.action?.let { action ->
                            when (action) {
                                NoticeAction.START_OVER -> session.startOver()
                                NoticeAction.GO_BACK -> session.goBack()
                                NoticeAction.CHANGE_MODE -> sheet = Sheet.Settings
                            }
                        }
                        session.dismissNotice(n.id)
                    }
                }
            }
            // Every notice goes after the time the engine gave it, button or not, counting only while
            // it is on screen with nothing over it. On e-ink it has none and the next turn takes it.
            if (notice != null) {
                val lasts = notice.notice.lastsMs.toLong()
                val id = notice.id
                LaunchedEffect(id, clear) {
                    if (lasts > 0 && clear) {
                        delay(lasts)
                        session.dismissNotice(id)
                    }
                }
            }
            val note = localNote
            AnimatedVisibility(
                visible = note != null && clear,
                enter = if (fades) fadeIn(Motion.fadeIn()) else EnterTransition.None,
                exit = if (fades) fadeOut(Motion.fadeOut()) else ExitTransition.None,
                modifier = Modifier.align(Alignment.TopCenter),
            ) {
                NoticePlate(note.orEmpty(), null, Modifier) {}
            }
            // A credit page's own notice: at the foot, above the controls while they show. A Fixtures
            // shot holds its Undo, except from the line, where it runs out as in the app.
            if (view != null) CreditSnackbar(state, session, images, visible = sheet == Sheet.None, above = if (shown) scrub.groupHeight else 0, fades = fades, timed = CreditShot.step == null || CreditShot.step == "line", exploring = exploring)
        }

        if (view != null) {
            when (sheet) {
                Sheet.Chapters -> ChaptersSheet(
                    session = session,
                    state = state,
                    images = images,
                    currentLeaf = pager.leaf,
                    rtl = direction.rightToLeft && view.mode == ReaderMode.PAGED,
                    onGo = {
                        pager.go(it, TurnHow.JUMP)
                        sheet = Sheet.None
                    },
                    onDismiss = { sheet = Sheet.None },
                    instant = instant,
                )
                Sheet.Settings -> ReaderSettingsSheet(
                    session,
                    state,
                    onDismiss = { sheet = Sheet.None },
                    // Show zones: the sheet and the controls go, and the guide shows over the page.
                    onShowZones = {
                        sheet = Sheet.None
                        chrome = false
                        reading?.let { guide.show(tapMap(zones, it, null)) }
                    },
                    toggle = ::toggleSetting,
                    instant = instant,
                    start = settingsStart,
                    quietDenied = quietDenied && !quietOn,
                )
                Sheet.None -> {}
            }
        }
      }
    }
}

/** Hides the system bars with the chrome and shows them with it; the icons follow the look. */
@Composable
private fun SystemBars(context: Context, shown: Boolean, light: Boolean) {
    val view = LocalView.current
    val window = remember { context.activity()?.window }
    val controller = remember(window) { window?.let { WindowCompat.getInsetsController(it, view) } }
    LaunchedEffect(controller) {
        controller?.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    }
    LaunchedEffect(controller, shown) {
        if (shown) controller?.show(WindowInsetsCompat.Type.systemBars()) else controller?.hide(WindowInsetsCompat.Type.systemBars())
    }
    LaunchedEffect(controller, light) { controller?.isAppearanceLightStatusBars = light }
    DisposableEffect(controller) {
        val before = controller?.isAppearanceLightStatusBars ?: false
        onDispose {
            controller?.show(WindowInsetsCompat.Type.systemBars())
            controller?.isAppearanceLightStatusBars = before
        }
    }
}

/** The chapter's number as the strings name it. */
internal fun chapterNumber(row: ChapterRow): String = row.number ?: row.label

private fun actionWords(context: Context, notice: ShownNotice): String? = when (notice.notice.action) {
    null -> null
    NoticeAction.START_OVER -> Strings.readerNoticeStartOver(context)
    NoticeAction.GO_BACK -> Strings.readerNoticeGoBack(context)
    NoticeAction.CHANGE_MODE -> Strings.readerNoticeChange(context)
}

/**
 * The one group at the thumb: Back to page while it is offered, the scrubber (page number at its
 * start, the length at its end, in mono 13), then Close, the series and chapter, Chapters and Aa.
 * Solid Raised, radius 28.
 */
@Composable
private fun ChromeGroup(
    state: ReaderState,
    title: String,
    line: String,
    scrubber: (@Composable () -> Unit)?,
    backTo: UInt?,
    onBackTo: () -> Unit,
    rtl: Boolean,
    fades: Boolean,
    chaptersOpen: Boolean,
    settingsOpen: Boolean,
    onClose: () -> Unit,
    onChapters: () -> Unit,
    onSettings: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val shape = RoundedCornerShape(28.dp)
    // Larger text grows the group with it: its words follow the font scale, its buttons are at least 48 dp.
    Column(
        modifier
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal + WindowInsetsSides.Bottom))
            .padding(start = 12.dp, end = 12.dp, bottom = 26.dp)
            .fillMaxWidth()
            .clip(shape)
            .background(Chrome.plate, shape)
            .padding(horizontal = 8.dp, vertical = 8.dp)
            .semantics { isTraversalGroup = true },
    ) {
        // The group grows upwards as it comes, so the scrubber under the finger stays put.
        val held = remember { mutableStateOf(backTo) }
        if (backTo != null) held.value = backTo
        AnimatedVisibility(
            visible = backTo != null,
            enter = if (fades) fadeIn(Motion.fadeIn()) + expandVertically(Motion.standard(), Alignment.Bottom) else EnterTransition.None,
            exit = if (fades) fadeOut(Motion.fadeOut()) + shrinkVertically(Motion.standard(), Alignment.Bottom) else ExitTransition.None,
            modifier = Modifier.semantics { isTraversalGroup = true; traversalIndex = 2f },
        ) {
            held.value?.let { page -> BackToPage(Strings.readerBackToPage(context, page.toInt()), rtl, onBackTo) }
        }
        if (scrubber != null) {
            Box(Modifier.semantics { isTraversalGroup = true; traversalIndex = 1f }) { scrubber() }
        }
        Row(
            Modifier.fillMaxWidth().semantics { isTraversalGroup = true; traversalIndex = 0f },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onClose, modifier = Modifier.sizeIn(minWidth = 48.dp, minHeight = 48.dp)) {
                GlyphIcon(Glyph.Close, Chrome.text, modifier = Modifier.semantics { contentDescription = Strings.readerClose(context) })
            }
            Column(Modifier.weight(1f).padding(horizontal = 6.dp)) {
                Text(title, color = Chrome.text, fontFamily = robotoFlex(FontWeight.Bold, 14.sp), fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                SwapFade(line, animationSpec = Motion.fadeIn()) { Text(it, color = Chrome.faint, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis) }
            }
            IconButton(
                onClick = onChapters,
                modifier = Modifier.sizeIn(minWidth = 48.dp, minHeight = 48.dp).background(if (chaptersOpen) Chrome.selected else Color.Transparent, RoundedCornerShape(24.dp)),
            ) {
                GlyphIcon(Glyph.List, Chrome.text, modifier = Modifier.semantics { contentDescription = Strings.readerChapters(context) })
            }
            // Aa is text, so its button grows with it.
            Box(
                Modifier
                    .sizeIn(minWidth = 48.dp, minHeight = 48.dp)
                    .clip(RoundedCornerShape(24.dp))
                    .background(if (settingsOpen) Chrome.selected else Color.Transparent)
                    .clickable(role = Role.Button, onClick = onSettings)
                    .semantics { contentDescription = Strings.readerSettings(context) }
                    .padding(horizontal = 6.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text("Aa", color = Chrome.text, fontSize = 17.sp, fontWeight = FontWeight.Medium, modifier = Modifier.clearAndSetSemantics {})
            }
        }
    }
}

/** The scrubber's drag, shared with the preview above the group. */
@Stable
internal class ScrubState {
    /** The page under the thumb while dragging, or null. */
    var page by mutableStateOf<Int?>(null)

    /** Where the thumb is, in the reader's own pixels from its left edge. */
    var thumbX by mutableFloatStateOf(0f)

    /** The page a jump left from, ringed on the track until the chapter closes. */
    var mark by mutableStateOf<Int?>(null)

    var groupHeight by mutableIntStateOf(0)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Scrubber(state: ReaderState, rtl: Boolean, shownLeaf: Int, scrub: ScrubState, onGo: (Int) -> Unit) {
    val context = LocalContext.current
    val count = state.pages.coerceAtLeast(1)
    val leaf = shownLeaf.coerceIn(0, (state.leaves.size - 1).coerceAtLeast(0))
    val page = (state.leaves.getOrNull(leaf)?.parts?.firstOrNull()?.page?.toInt() ?: 0).coerceIn(0, count - 1)
    val at = scrub.page ?: page
    // A credit page only shown has no number, and the count stays; one kept counts at once.
    val unnumbered = state.onUnkeptCredit && scrub.page == null
    val ticks = state.partTicks
    val haptics = LocalHapticFeedback.current
    val near = with(LocalDensity.current) { 8.dp.toPx() }
    val colours = SliderDefaults.colors(
        thumbColor = Chrome.text,
        activeTrackColor = Chrome.text,
        inactiveTrackColor = Chrome.rule,
        activeTickColor = Color.Transparent,
        inactiveTickColor = Color.Transparent,
        disabledThumbColor = Chrome.faint,
        disabledActiveTrackColor = Chrome.faint,
        disabledInactiveTrackColor = Chrome.rule,
    )
    val ring = Chrome.text
    val hole = Chrome.plate
    var trackLeft by remember { mutableFloatStateOf(0f) }
    var trackWidth by remember { mutableFloatStateOf(0f) }
    val last = (count - 1).coerceAtLeast(1)
    fun xOf(p: Int): Float {
        val f = p.toFloat() / last
        return trackLeft + (if (rtl) 1f - f else f) * trackWidth
    }
    // A right-to-left chapter's scrubber runs from the right, as its pages do.
    val layout = if (rtl) LayoutDirection.Rtl else LayoutDirection.Ltr
    CompositionLocalProvider(LocalLayoutDirection provides layout) {
        Row(Modifier.fillMaxWidth().heightIn(min = 28.dp).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(if (unnumbered) "" else (state.shownPage(at) + 1).toString(), color = Chrome.text, fontFamily = FontFamily.Monospace, fontSize = 13.sp, modifier = Modifier.widthIn(min = 32.dp))
            val slider = remember(count) {
                SliderState(page.toFloat(), (count - 2).coerceAtLeast(0), 0f..last.toFloat())
            }
            // Away from the thumb, the slider follows the page turned to.
            LaunchedEffect(slider, page) { if (scrub.page == null) slider.value = page.toFloat() }
            Slider(
                state = slider,
                onValueChange = {
                    // The thumb snaps lightly onto a part's tick within 8 dp of it, with a tick felt.
                    val snap = ticks.firstOrNull { tick -> !tick.missing && tick.page < count && abs(tick.page - it) * trackWidth / last < near }
                    if (snap != null && scrub.page != snap.page) haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
                    slider.value = snap?.page?.toFloat() ?: it
                    // The place the drag started from keeps its ring.
                    if (scrub.page == null) scrub.mark = page
                    scrub.page = snap?.page ?: it.roundToInt().coerceIn(0, count - 1)
                    scrub.thumbX = xOf(scrub.page ?: 0)
                },
                onValueChangeFinished = {
                    val target = scrub.page
                    scrub.page = null
                    if (target != null && target != page) state.leafOf(target.toUInt())?.let(onGo) else if (target == page) scrub.mark = null
                },
                enabled = count > 1,
                colors = colours,
                track = { sliderState: SliderState ->
                    Box(
                        Modifier.fillMaxWidth().onGloballyPositioned { box ->
                            trackLeft = box.positionInRoot().x
                            trackWidth = box.size.width.toFloat()
                        },
                        contentAlignment = Alignment.CenterStart,
                    ) {
                        SliderDefaults.Track(sliderState = sliderState, colors = colours, enabled = count > 1, drawStopIndicator = null)
                        if (ticks.isNotEmpty()) {
                            // Where each later part starts: 2 by 9 dp, in the ground over the filled
                            // track and haze past it; a part no group has is fainter.
                            val tick = Chrome.faint
                            val inside = Chrome.ground
                            Canvas(Modifier.matchParentSize()) {
                                val reached = sliderState.value
                                for (t in ticks) {
                                    val f = t.page.coerceIn(0, last).toFloat() / last
                                    val x = if (layoutDirection == LayoutDirection.Rtl) size.width * (1f - f) else size.width * f
                                    val w = 2.dp.toPx()
                                    val h = 9.dp.toPx()
                                    drawRoundRect(
                                        (if (t.page <= reached) inside else tick).copy(alpha = if (t.missing) 0.4f else 1f),
                                        topLeft = Offset(x - w / 2, size.height / 2 - h / 2),
                                        size = Size(w, h),
                                        cornerRadius = CornerRadius(w / 2),
                                    )
                                }
                            }
                        }
                        val mark = scrub.mark
                        if (mark != null) {
                            // The ring at the old place: 11 dp, a 2 dp silk ring on the group's colour.
                            Canvas(Modifier.matchParentSize()) {
                                val f = mark.toFloat() / last
                                val x = if (layoutDirection == LayoutDirection.Rtl) size.width * (1f - f) else size.width * f
                                val r = 5.5.dp.toPx()
                                drawCircle(hole, radius = r, center = Offset(x, size.height / 2))
                                drawCircle(ring, radius = r - 1.dp.toPx(), center = Offset(x, size.height / 2), style = Stroke(2.dp.toPx()))
                            }
                        }
                    }
                },
                modifier = Modifier.weight(1f).semantics {
                    contentDescription = Strings.readerScrubberInChapter(context)
                    stateDescription = state.partStarting(at)?.let { Strings.readerPartStarts(context, it.toInt(), state.shownPage(at) + 1) }
                        ?: if (unnumbered) Strings.readerCreditPageLabel(context) else Strings.readerPageOf(context, state.shownPage(at) + 1, state.shownPages.coerceAtLeast(1))
                },
            )
            SwapFade(state.shownPages.coerceAtLeast(1), Modifier.widthIn(min = 32.dp), animationSpec = Motion.fadeIn()) {
                Text(it.toString(), color = Chrome.faint, fontFamily = FontFamily.Monospace, fontSize = 13.sp)
            }
        }
    }
}

/**
 * The page under the thumb: 96 dp wide in a 2 dp silk ring, its number under it, above the group
 * and never nearer than 12 dp to the screen's edges. It follows the thumb; the page waits for the
 * release.
 */
@Composable
private fun BoxScope.ScrubPreview(session: ReaderSession, state: ReaderState, images: PageImages, scrub: ScrubState, fades: Boolean) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val held = remember { mutableStateOf(0) }
    scrub.page?.let { held.value = it }
    AnimatedVisibility(
        visible = scrub.page != null,
        enter = if (fades) fadeIn(Motion.fadeIn()) else EnterTransition.None,
        exit = if (fades) fadeOut(Motion.fadeOut()) else ExitTransition.None,
        modifier = Modifier.align(Alignment.BottomStart),
    ) {
        val page = held.value
        val part = state.leaves.getOrNull(state.leafOf(page.toUInt()) ?: -1)?.parts?.firstOrNull { it.page.toInt() == page }
        // A page not made yet is asked for, so the preview shows it rather than its number.
        LaunchedEffect(part) { if (part != null && state.file(part) == null) session.needPage(part) }
        val width = 96.dp
        val height = (96 / 0.703).dp
        val ring = Chrome.text
        val gap = Chrome.ground
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            val screen = with(density) { maxWidth.toPx() }
            val half = with(density) { (width / 2 + 4.dp).toPx() }
            val edge = with(density) { 12.dp.toPx() }
            val centre = scrub.thumbX.coerceIn(edge + half, screen - edge - half)
            Column(
                Modifier
                    .offset { IntOffset((centre - half).roundToInt(), 0) }
                    .padding(bottom = with(density) { scrub.groupHeight.toDp() } + 12.dp)
                    .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom)),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                PageFileView(
                    file = part?.let(state::file),
                    images = images,
                    modifier = Modifier
                        .padding(4.dp)
                        .drawBehind {
                            val out = 4.dp.toPx()
                            drawRoundRect(ring, Offset(-out, -out), Size(size.width + 2 * out, size.height + 2 * out), CornerRadius(8.dp.toPx()))
                            drawRoundRect(gap, Offset(-out / 2, -out / 2), Size(size.width + out, size.height + out), CornerRadius(6.dp.toPx()))
                        }
                        .size(width, height)
                        .clip(RoundedCornerShape(4.dp)),
                ) {
                    Box(Modifier.fillMaxSize().background(Chrome.plate), contentAlignment = Alignment.Center) {
                        Text((page + 1).toString(), color = Chrome.faint, fontSize = 15.sp)
                    }
                }
                Text(
                    state.partStarting(page)?.let { part ->
                        // The part before it that no group has is named after it.
                        state.partMissingAt(page)?.let { Strings.readerPartPreviewMissing(context, state.shownPage(page) + 1, part.toInt(), it.toInt()) }
                            ?: Strings.readerPartPreview(context, state.shownPage(page) + 1, part.toInt())
                    }
                        ?: Strings.readerPagePlate(context, state.shownPage(page) + 1),
                    color = Chrome.text,
                    fontSize = 13.sp,
                    fontWeight = FontWeight.SemiBold,
                    // Silk over a light page needs the shadow to be read.
                    style = TextStyle(shadow = Shadow(Color.Black.copy(alpha = 0.7f), blurRadius = 8f)),
                )
            }
        }
    }
}

/**
 * A novel's scrubber over the paginator's pages: the page and the count when paged, and in scroll
 * mode how far in and the minutes left at a minute a screen, since there are no pages.
 */
@Composable
private fun NovelScrubber(state: ReaderState, place: NovelPlace) {
    val context = LocalContext.current
    val pages = place.pages.coerceAtLeast(1)
    val last = (pages - 1).coerceAtLeast(1)
    var dragging by remember { mutableStateOf<Int?>(null) }
    val at = (dragging ?: place.page).coerceIn(0, pages - 1)
    val scroll = state.choice(ReaderSettingKey.MODE) == "scrolled"
    val start = if (scroll) "${(at * 100) / last}%" else (at + 1).toString()
    val end = if (scroll) Strings.readerMinutesLeft(context, (pages - 1 - at).coerceAtLeast(1)) else pages.toString()
    Row(Modifier.fillMaxWidth().heightIn(min = 28.dp).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        // The numbers are the slider's value, which says them as "Page 3 of 8".
        Text(start, color = Chrome.text, fontFamily = FontFamily.Monospace, fontSize = 13.sp, modifier = Modifier.widthIn(min = 32.dp).clearAndSetSemantics {})
        Slider(
            value = at.toFloat(),
            onValueChange = { dragging = it.roundToInt() },
            onValueChangeFinished = {
                dragging?.let(place::go)
                dragging = null
            },
            enabled = pages > 1,
            valueRange = 0f..last.toFloat(),
            colors = SliderDefaults.colors(
                thumbColor = Chrome.text,
                activeTrackColor = Chrome.text,
                inactiveTrackColor = Chrome.rule,
                disabledThumbColor = Chrome.faint,
                disabledActiveTrackColor = Chrome.faint,
                disabledInactiveTrackColor = Chrome.rule,
            ),
            modifier = Modifier.weight(1f).padding(horizontal = 4.dp).semantics {
                contentDescription = Strings.readerScrubberInChapter(context)
                stateDescription = if (scroll) start else Strings.readerPageOf(context, at + 1, pages)
            },
        )
        Text(end, color = Chrome.faint, fontFamily = FontFamily.Monospace, fontSize = 13.sp, textAlign = TextAlign.End, modifier = Modifier.widthIn(min = 32.dp).then(if (scroll) Modifier else Modifier.clearAndSetSemantics {}))
    }
}

/** A novel's line under its title: the chapter and its title when paged, the chapter and scroll when scrolling. */
private fun novelLine(context: Context, state: ReaderState): String {
    val chapter = state.view?.chapter ?: return ""
    return if (state.choice(ReaderSettingKey.MODE) == "scrolled") {
        Strings.readerNovelLineScroll(context, chapterNumber(chapter))
    } else {
        chapterWords(context, chapter)
    }
}

/** "6 pages left in this chapter", in the bottom margin, 28 dp from the foot, when Show pages left is on. */
@Composable
private fun BoxScope.PagesLeft(state: ReaderState, place: NovelPlace) {
    val context = LocalContext.current
    if (state.choice(ReaderSettingKey.FOOT_VALUE) == "none" || state.choice(ReaderSettingKey.MODE) == "scrolled") return
    if (place.pages <= 0) return
    val left = (place.pages - 1 - place.page).coerceAtLeast(0)
    if (left == 0) return
    Text(
        Strings.readerPagesLeft(context, left),
        color = Chrome.faint,
        fontSize = 13.sp,
        textAlign = TextAlign.Center,
        modifier = Modifier
            .align(Alignment.BottomCenter)
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom))
            .padding(bottom = 28.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
    )
}

/**
 * A novel's end page over its last page: the same page as manga's. A swipe back returns to the
 * text, a swipe on goes to the next chapter.
 */
@Composable
private fun NovelEndPage(
    state: ReaderState,
    session: ReaderSession,
    actions: PhoneReaderActions,
    images: PageImages,
    direction: ReadDirection,
    zones: TapZoneSettings,
    reading: TapReading?,
    onBack: () -> Unit,
    cover: (@Composable (Modifier) -> Unit)?,
    onFirstPage: () -> Unit,
) {
    val density = LocalDensity.current
    val far = with(density) { 80.dp.toPx() }
    val next = (state.end?.next ?: state.view?.next)
    Box(
        Modifier.fillMaxSize().pointerInput(zones, reading) {
            // A tap in a Next zone goes on, in a Previous zone returns to the text; the page's
            // buttons take their own taps first.
            detectTapGestures { at ->
                when (reading?.let { zones.actionAt(it, at.x / size.width, at.y / size.height) }) {
                    TapAction.NEXT -> session.crossOn()
                    TapAction.PREVIOUS -> onBack()
                    else -> {}
                }
            }
        }.pointerInput(next?.id, direction) {
            var total = 0f
            detectHorizontalDragGestures(
                onDragStart = { total = 0f },
                onHorizontalDrag = { change, amount ->
                    total += amount
                    change.consume()
                },
                onDragEnd = {
                    // Right to left, the next page is to the left: a drag to the right goes on.
                    val forward = if (direction.rightToLeft) total > far else total < -far
                    val backward = if (direction.rightToLeft) total < -far else total > far
                    when {
                        forward && next != null -> session.crossOn()
                        backward -> onBack()
                    }
                },
            )
        },
    ) {
        PhoneEndPage(state, session, actions, images, onFirstPage = onFirstPage, cover = cover)
    }
}

/** A capsule at the top centre: 44 dp tall at least, solid, with an underlined text action. */
@Composable
private fun NoticePlate(words: String, action: String?, modifier: Modifier, atFoot: Boolean = false, onAction: () -> Unit) {
    Row(
        modifier
            .windowInsetsPadding(WindowInsets.safeDrawing)
            // At the foot it is centred where the pages left line sits.
            .padding(top = if (atFoot) 0.dp else 12.dp, bottom = if (atFoot) 15.dp else 0.dp, start = 16.dp, end = 16.dp)
            .heightIn(min = 44.dp)
            .background(Chrome.plate, RoundedCornerShape(22.dp))
            .border(1.dp, Chrome.rule, RoundedCornerShape(22.dp))
            .padding(start = 18.dp, end = if (action != null) 6.dp else 18.dp, top = 4.dp, bottom = 4.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(words, color = Chrome.text, fontSize = 14.sp, modifier = Modifier.weight(1f, fill = false))
        if (action != null) {
            TextButton(onClick = onAction) {
                Text(action, color = Chrome.text, fontSize = 14.sp, fontWeight = FontWeight.Medium, textDecoration = TextDecoration.Underline)
            }
        }
    }
}

/**
 * Back to page, at the leading end above the scrubber: 32 dp tall on the selected fill inside a
 * 48 dp target, the back glyph and the words in silk.
 */
@Composable
private fun BackToPage(words: String, rtl: Boolean, onClick: () -> Unit) {
    Row(
        Modifier
            .padding(start = 4.dp, top = 2.dp)
            .heightIn(min = 48.dp)
            .clip(RoundedCornerShape(50))
            .clickable(role = Role.Button, onClick = onClick)
            .padding(vertical = 8.dp)
            .heightIn(min = 32.dp)
            .background(Chrome.selected, RoundedCornerShape(50))
            .padding(start = 10.dp, end = 14.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        GlyphIcon(Glyph.Back, Chrome.text, size = 18.dp, mirror = rtl)
        Text(words, color = Chrome.text, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/**
 * The strip's divider into the next chapter, 168 dp on the ground: Next, its name at 22 on 28,
 * and how many images it has and where they come from.
 */
@Composable
internal fun StripDivider(state: ReaderState, next: ChapterRow, onShowCredits: (ChapterId) -> Unit = {}) {
    val context = LocalContext.current
    // The chapter before the divider: the one kept above when it leads into the open one.
    val before = if (next.id == state.view?.chapter?.id) state.previous?.chapter else state.chapter
    val skipped = before?.let(state::endSkipped).orEmpty()
    val images = next.pages?.let { Strings.readerDividerImages(context, it.toInt()) }
    val source = state.end?.source?.takeIf { state.end.next?.id == next.id }
        ?: state.previous?.end?.source?.takeIf { state.previous.end.next?.id == next.id }
    val facts = when {
        images != null && source != null -> Strings.readerDividerFactsSource(context, images, source)
        else -> images
    }
    Column(
        Modifier.fillMaxWidth().heightIn(min = StripDividerHeight).background(Chrome.ground).padding(horizontal = 24.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Text(Strings.readerEndNext(context), color = Chrome.faint, fontSize = 13.sp)
        Text(
            chapterWords(context, next),
            color = Chrome.text,
            fontFamily = robotoFlex(FontWeight.Bold, 22.sp),
            fontSize = 22.sp,
            lineHeight = 28.sp,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = 2.dp),
        )
        if (facts != null) Text(facts, color = Chrome.faint, fontSize = 14.sp, modifier = Modifier.padding(top = 4.dp))
        // Pages skipped at the end of the chapter before, with Show.
        if (before != null && skipped.isNotEmpty()) SkippedLine(skipped.size, Modifier.padding(top = 8.dp)) { onShowCredits(before) }
    }
}

/** What the open waits on, in words: the stage the engine named, else the chapter. */
fun openingStage(context: android.content.Context, wait: ReaderWait?, chapter: String?): String? = when (wait) {
    is ReaderWait.ChapterList -> Strings.readerWaitChapterList(context, wait.source)
    is ReaderWait.ChapterPages -> Strings.readerWaitChapterPages(context, wait.chapter, wait.source)
    else -> chapter?.let { Strings.readerOpeningChapter(context, it) }
}

/** How long a stage goes on before the opening offers another way, as the player's start does. */
internal const val OPENING_LONG_MS = 15_000

/**
 * While a chapter opens: nothing for 600 ms, then the page plate in the page's place with what
 * the open waits on, from 3 s with how long, and from [OPENING_LONG_MS] another source and Go
 * back. Once the first page is [done] and drawn over it, the plate fades out.
 */
@Composable
private fun Opening(
    state: ReaderState,
    done: Boolean,
    drawn: Boolean,
    fades: Boolean,
    onGone: () -> Unit,
    session: ReaderSession,
    instant: Boolean,
    onClose: () -> Unit,
) {
    val context = LocalContext.current
    // Counted from the ask, so a novel's plate after the chapter opens goes on from the first one.
    val since = remember(state.openingSince) { state.openingSince.takeIf { it > 0 } ?: openingClock() }
    var waited by remember { mutableIntStateOf((openingClock() - since).toInt()) }
    LaunchedEffect(since) {
        while (true) {
            waited = (openingClock() - since).toInt()
            delay(200)
        }
    }
    val shown = remember { Animatable(1f) }
    val gone by rememberUpdatedState(onGone)
    LaunchedEffect(drawn) {
        if (drawn) {
            if (fades) shown.animateTo(0f, Motion.fadeOut()) else shown.snapTo(0f)
            gone()
        }
    }
    if (waited < PageWaits.PLATE_MS) return
    val number = (state.openingChapter ?: state.view?.chapter)?.let(::chapterNumber)
    val stage = openingStage(context, state.openingWait, number)
    val line = when {
        waited < 3000 -> stage
        else -> stage?.let { Strings.readerWaitSeconds(context, it, waited / 1000) }
    }
    val long = waited >= OPENING_LONG_MS && !done
    LaunchedEffect(long) { if (long) session.loadSources() }
    var choosing by remember { mutableStateOf(false) }
    BoxWithConstraints(Modifier.fillMaxSize().graphicsLayer { alpha = shown.value }, contentAlignment = Alignment.Center) {
        // The page's place: a page's shape, as wide as it fits.
        val width = minOf(maxWidth, maxHeight * 0.703f)
        Box(Modifier.width(width).aspectRatio(0.703f).background(Chrome.plate)) {
            if (line != null && !done) {
                Text(
                    line,
                    color = Chrome.faint,
                    fontSize = 15.sp,
                    textAlign = TextAlign.Center,
                    modifier = Modifier
                        .align(Alignment.Center)
                        .padding(horizontal = 24.dp)
                        .semantics { liveRegion = LiveRegionMode.Polite },
                )
            }
        }
        if (long) {
            // A stage that goes on offers the way on, as a failed open does.
            Column(
                Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(horizontal = 28.dp).padding(bottom = 30.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(Strings.readerWaitLong(context), color = Chrome.faint, fontSize = 15.sp, modifier = Modifier.padding(bottom = 12.dp))
                val sources = state.sources.orEmpty()
                if (sources.isNotEmpty()) {
                    OutlinedButton(onClick = { choosing = true }, shape = RoundedCornerShape(25.dp), modifier = Modifier.fillMaxWidth().height(50.dp)) {
                        Text(Strings.readerFailedChoose(context), color = Chrome.text, fontSize = 16.sp)
                    }
                    Spacer(Modifier.height(4.dp))
                }
                TextButton(onClick = onClose, modifier = Modifier.heightIn(min = 38.dp)) {
                    Text(Strings.readerNoticeGoBack(context), color = Chrome.text, fontSize = 15.sp)
                }
            }
        }
    }
    if (choosing) {
        SourcesSheet(state.sources.orEmpty(), instant, onDismiss = { choosing = false }) { session.chooseSource(it) }
    }
}

/**
 * A chapter that would not open: left aligned, the glyph beside the title at 22, the reason in
 * haze. Choose source is the amber pill when another source has the chapter, with the sources in a
 * sheet; otherwise Try again is. Close is text below.
 */
@Composable
private fun ChapterFailed(
    title: String,
    line: String,
    state: ReaderState,
    session: ReaderSession,
    onClose: () -> Unit,
    onRetry: () -> Unit,
    instant: Boolean,
) {
    val context = LocalContext.current
    LaunchedEffect(Unit) { session.loadSources() }
    val sources = state.sources.orEmpty()
    var choosing by remember { mutableStateOf(false) }
    Box(Modifier.fillMaxSize()) {
        Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            Column(Modifier.align(Alignment.CenterStart).padding(horizontal = 28.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    GlyphIcon(Glyph.Warning, Chrome.error)
                    Text(title, color = Chrome.text, fontFamily = robotoFlex(FontWeight.SemiBold, 22.sp), fontSize = 22.sp, lineHeight = 28.sp)
                }
                Spacer(Modifier.height(8.dp))
                Text(line, color = Chrome.faint, fontSize = 15.sp, lineHeight = 21.sp)
            }
            Column(
                Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(horizontal = 28.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                val choose = sources.isNotEmpty()
                Button(
                    onClick = { if (choose) choosing = true else onRetry() },
                    shape = RoundedCornerShape(25.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = Chrome.accent, contentColor = Chrome.onAccent),
                    modifier = Modifier.fillMaxWidth().height(50.dp),
                ) {
                    Text(
                        if (choose) Strings.readerFailedChoose(context) else Strings.readerTryAgain(context),
                        fontSize = 16.sp,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
                Spacer(Modifier.height(4.dp))
                if (choose) {
                    TextButton(onClick = onRetry, modifier = Modifier.heightIn(min = 38.dp)) {
                        Text(Strings.readerTryAgain(context), color = Chrome.text, fontSize = 15.sp)
                    }
                }
                TextButton(onClick = onClose, modifier = Modifier.heightIn(min = 38.dp)) {
                    Text(Strings.readerClose(context), color = Chrome.text, fontSize = 15.sp)
                }
                Spacer(Modifier.height(30.dp))
            }
        }
        if (choosing) {
            SourcesSheet(sources, instant, onDismiss = { choosing = false }) { session.chooseSource(it) }
        }
    }
}

/** The other sources that have the chapter, to open it from one of them instead. */
@Composable
private fun SourcesSheet(sources: List<ChapterVersion>, instant: Boolean, onDismiss: () -> Unit, onPick: (ChapterVersion) -> Unit) {
    val context = LocalContext.current
    ReaderSheet(instant = instant, onDismiss = onDismiss) {
        Column(Modifier.navigationBarsPadding().padding(horizontal = 20.dp).padding(bottom = 16.dp)) {
            Text(Strings.readerSourcesTitle(context), color = Chrome.faint, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(bottom = 6.dp))
            sources.forEach { version ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = 56.dp)
                        .clip(RoundedCornerShape(12.dp))
                        .clickable {
                            onDismiss()
                            onPick(version)
                        }
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(version.edition.sourceName, color = Chrome.text, fontSize = 16.sp, fontWeight = FontWeight.Medium, modifier = Modifier.weight(1f))
                    GlyphIcon(Glyph.Chevron, Chrome.faint, size = 20.dp)
                }
            }
        }
    }
}

/** The setting [key]'s amount, or null. */
internal fun ReaderState.amount(key: ReaderSettingKey): Double? =
    (settings?.rows?.firstOrNull { it.key == key }?.value as? ReaderValue.Amount)?.value

/** How long a demo work's credit stays with the chrome after the reader opens. */
internal const val DEMO_CREDIT_HOLD_MILLIS = 3_000L

/**
 * A demo work's credit: a pill 32 dp tall under the status bar, the info glyph and "series ·
 * author · licence" in 13 sp. The licence travels with the work, as CC BY asks.
 */
@Composable
private fun CreditCapsule(demo: app.kaeri.engine.model.DemoWorkInfo) {
    val context = LocalContext.current
    val words = demo.licence?.let { Strings.readerDemoCredit(context, demo.series, demo.author, it) } ?: demo.credit
    Row(
        Modifier
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .padding(top = 12.dp, start = 16.dp, end = 16.dp)
            .heightIn(min = 32.dp)
            .background(Chrome.plate.copy(alpha = 0.94f), RoundedCornerShape(16.dp))
            .border(1.dp, Chrome.rule, RoundedCornerShape(16.dp))
            .padding(horizontal = 16.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        GlyphIcon(Glyph.Info, Chrome.faint, size = 13.dp)
        Text(words, color = Chrome.text, fontSize = 13.sp, lineHeight = 18.sp, maxLines = 2, overflow = TextOverflow.Ellipsis)
    }
}
