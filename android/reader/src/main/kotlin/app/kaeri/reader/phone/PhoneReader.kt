// The phone's reader: a chapter's pages or strip over the engine's reader session, with the one
// chrome group the board draws at the thumb (scrubber, Close, the series and chapter, Chapters and
// Aa), the notices, the end page and the sheets. The engine decides what is shown and when; this
// draws it. The status bar goes with the chrome.

package app.kaeri.reader.phone

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.view.accessibility.AccessibilityManager
import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
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
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.IconButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
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
import app.kaeri.reader.PagedReader
import app.kaeri.reader.ReaderAction
import app.kaeri.reader.ReaderKeyRules
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.ShownNotice
import app.kaeri.reader.StripReader
import app.kaeri.reader.TapZones
import app.kaeri.reader.readerKeys
import app.kaeri.reader.readerSurface
import app.kaeri.reader.rememberPagedReaderState
import app.kaeri.reader.rightToLeft
import app.kaeri.reader.text
import app.kaeri.store.text
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay

/** What the phone reader leaves to the app around it. */
class PhoneReaderActions(
    val onClose: () -> Unit,
    val onSeries: () -> Unit = onClose,
    val onDownloads: () -> Unit = onClose,
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

/** The tap bands for the Tap zones setting: edges are thirds, halves keep a band in the middle, off has none. */
internal fun zonesFor(choice: String?, direction: ReadDirection): TapZones = when (choice) {
    "sides" -> if (direction.rightToLeft) TapZones(0f..0.45f, 0.45f..0.55f) else TapZones(0.55f..1f, 0.45f..0.55f)
    "off" -> TapZones(2f..2f, 0f..1f)
    else -> TapZones.phone(direction)
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
    novel: @Composable (onControls: () -> Unit) -> Unit,
    modifier: Modifier = Modifier,
    quiet: QuietPhone? = null,
) {
    val context = LocalContext.current
    val view = state.view
    val density = LocalDensity.current
    val direction = view?.direction ?: ReadDirection.RIGHT_TO_LEFT
    val paper = state.toggle(ReaderSettingKey.PAPER_MODE)
    val novelOpen = state.novel != null || view?.medium == Medium.NOVEL
    val systemDark = isSystemInDarkTheme()
    // The chrome turns light with the Paper, Light and Sepia looks; Paper mode follows the system.
    val light = when {
        paper -> !systemDark
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
    val pager = rememberPagedReaderState()
    val volume = state.toggle(ReaderSettingKey.VOLUME_KEYS_TURN) || paper

    // While TalkBack runs the chrome stays shown.
    val accessibility = remember { context.getSystemService(AccessibilityManager::class.java) }
    val exploring = accessibility?.isTouchExplorationEnabled == true
    val shown = chrome || exploring

    SystemBars(context, shown, light)

    val quietOn = state.toggle(ReaderSettingKey.QUIET_PHONE)
    val owner = remember { context.activity() as? LifecycleOwner }
    var started by remember { mutableStateOf(true) }
    var askedAccess by remember { mutableStateOf(false) }
    DisposableEffect(owner) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START -> started = true
                Lifecycle.Event.ON_STOP -> started = false
                Lifecycle.Event.ON_RESUME -> if (askedAccess) {
                    askedAccess = false
                    // Back from the access page: on when it was granted, otherwise it stays off.
                    if (quiet?.allowed == true) session.setSetting(ReaderSettingKey.QUIET_PHONE, SettingScope.DEVICE, ReaderValue.Toggle(true))
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
        when (action) {
            ReaderAction.FORWARD -> pager.forward(TurnHow.KEY)
            ReaderAction.BACK -> pager.back(TurnHow.KEY)
            ReaderAction.CONTROLS -> chrome = !chrome
            ReaderAction.CLEAN -> {}
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
        paper -> Chrome.paper
        else -> when (state.choice(ReaderSettingKey.BACKGROUND)) {
            "black" -> Color.Black
            "white" -> Color.White
            else -> colours.ground
        }
    }
    // Paper mode dims the page's white instead of tinting the page.
    val tone = remember(paper) {
        if (paper) ColorFilter.colorMatrix(ColorMatrix().apply { setToScale(0.9f, 0.9f, 0.88f, 1f) }) else null
    }
    val fades = !paper

    CompositionLocalProvider(LocalChrome provides colours, LocalPageTone provides tone) {
        Box(
            modifier
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
                .readerKeys(ReaderKeyRules(direction, volumeTurns = volume), ::act),
        ) {
            val failure = state.failure
            val tapZones = zonesFor(state.choice(ReaderSettingKey.TAP_LAYOUT), direction)
            when {
                failure != null && state.leaves.isEmpty() && state.novel == null ->
                    Failed(
                        title = Strings.readerFailedChapter(context, view?.chapter?.let(::chapterNumber) ?: ""),
                        line = failure.text(context),
                        onClose = actions.onClose,
                        onChoose = actions.onSeries,
                    ) { session.retry(null) }
                novelOpen && view != null -> novel { chrome = !chrome }
                state.leaves.isEmpty() -> Opening(view?.chapter?.let(::chapterNumber))
                view?.mode == ReaderMode.STRIP || view?.mode == ReaderMode.VERTICAL -> {
                    var opened by remember(view.chapter.id) { mutableStateOf(false) }
                    StripReader(
                        state = state,
                        images = images,
                        onAt = { at, foot -> session.stripAt(at, foot) },
                        onTap = { chrome = !chrome },
                        onScrolled = { chrome = false },
                        onMissing = session::needPage,
                        onRetry = { session.retry(it.toUInt()) },
                        onJumped = session::jumped,
                        footer = { PhoneEndPage(state, session, actions) },
                        flow = view.next?.let { next -> { StripDivider(next) } },
                        onFlowed = {
                            val next = view.next
                            if (next != null && !opened) {
                                opened = true
                                session.openChapter(next.id)
                            }
                        },
                    )
                }
                else -> PagedReader(
                    state = state,
                    pager = pager,
                    images = images,
                    direction = direction,
                    zones = tapZones,
                    onControls = { chrome = !chrome },
                    onShown = { leaf, how ->
                        session.shown(leaf, how)
                        if (how != TurnHow.JUMP) chrome = false
                    },
                    onMissing = session::needPage,
                    onRetry = { session.retry(it.toUInt()) },
                    onJumped = session::jumped,
                    endPage = { PhoneEndPage(state, session, actions) },
                    motion = fades,
                )
            }

            if (view != null && view.mode != ReaderMode.STRIP && view.mode != ReaderMode.VERTICAL && !novelOpen && state.leaves.isNotEmpty()) {
                TapZonesHint(context, direction, tapZones, pager.turns, shown)
            }

            if (view != null) {
                AnimatedVisibility(
                    visible = shown,
                    enter = if (fades) fadeIn(Motion.fadeIn()) else EnterTransition.None,
                    exit = if (fades) fadeOut(Motion.fadeOut()) else ExitTransition.None,
                    modifier = Modifier.align(Alignment.BottomCenter),
                ) {
                    ChromeGroup(
                        state = state,
                        title = view.title,
                        chapter = view.chapter,
                        rtl = direction.rightToLeft && view.mode == ReaderMode.PAGED,
                        scrubber = !novelOpen,
                        shownLeaf = pager.leaf,
                        chaptersOpen = sheet == Sheet.Chapters,
                        settingsOpen = sheet == Sheet.Settings,
                        onGo = { pager.go(it, TurnHow.JUMP) },
                        onClose = actions.onClose,
                        onChapters = { sheet = Sheet.Chapters },
                        onSettings = { sheet = Sheet.Settings },
                    )
                }
            }

            // Notices sit at the top centre while the chrome is away, and Back to page in the bottom
            // leading corner, as the board has them.
            val notice = state.notice
            val clear = !shown && sheet == Sheet.None
            AnimatedVisibility(
                visible = notice != null && clear && notice.notice.action != NoticeAction.BACK_TO,
                enter = if (fades) fadeIn(Motion.fadeIn()) else EnterTransition.None,
                exit = if (fades) fadeOut(Motion.fadeOut()) else ExitTransition.None,
                modifier = Modifier.align(Alignment.TopCenter),
            ) {
                val held = remember { mutableStateOf<ShownNotice?>(null) }
                if (notice != null) held.value = notice
                held.value?.let { n ->
                    NoticePlate(n.notice.msg.text(context), actionWords(context, n), Modifier) {
                        n.notice.action?.let { action ->
                            when (action) {
                                NoticeAction.START_OVER -> session.startOver()
                                NoticeAction.BACK_TO -> session.backTo()
                                NoticeAction.GO_BACK -> session.goBack()
                                NoticeAction.CHANGE_MODE -> sheet = Sheet.Settings
                            }
                        }
                        session.dismissNotice(n.id)
                    }
                }
            }
            AnimatedVisibility(
                visible = notice != null && clear && notice.notice.action == NoticeAction.BACK_TO,
                enter = if (fades) fadeIn(Motion.fadeIn()) else EnterTransition.None,
                exit = if (fades) fadeOut(Motion.fadeOut()) else ExitTransition.None,
                modifier = Modifier.align(Alignment.BottomStart),
            ) {
                val held = remember { mutableStateOf<ShownNotice?>(null) }
                if (notice != null) held.value = notice
                held.value?.let { n ->
                    BackToPage(n.notice.msg.text(context), rtl = direction.rightToLeft) {
                        session.backTo()
                        session.dismissNotice(n.id)
                    }
                }
            }
            // A notice without a button times out, counting only while it is on screen with nothing over it.
            if (notice != null && notice.notice.action == null) {
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
                )
                Sheet.Settings -> ReaderSettingsSheet(session, state, onDismiss = { sheet = Sheet.None }, toggle = ::toggleSetting)
                Sheet.None -> {}
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
    NoticeAction.BACK_TO -> notice.notice.msg.text(context)
    NoticeAction.GO_BACK -> Strings.readerNoticeGoBack(context)
    NoticeAction.CHANGE_MODE -> Strings.readerNoticeChange(context)
}

/**
 * The one group at the thumb: the scrubber (page number at its start, the length at its end, in
 * mono 13), then Close, the series and chapter, Chapters and Aa. Solid Raised, radius 28.
 */
@Composable
private fun ChromeGroup(
    state: ReaderState,
    title: String,
    chapter: ChapterRow,
    rtl: Boolean,
    scrubber: Boolean,
    shownLeaf: Int,
    chaptersOpen: Boolean,
    settingsOpen: Boolean,
    onGo: (Int) -> Unit,
    onClose: () -> Unit,
    onChapters: () -> Unit,
    onSettings: () -> Unit,
) {
    val context = LocalContext.current
    val density = LocalDensity.current
    val shape = RoundedCornerShape(28.dp)
    // Larger text scales the group up to 1.3 times.
    CompositionLocalProvider(LocalDensity provides Density(density.density, density.fontScale.coerceAtMost(1.3f))) {
        Column(
            Modifier
                .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal + WindowInsetsSides.Bottom))
                .padding(start = 12.dp, end = 12.dp, bottom = 26.dp)
                .fillMaxWidth()
                .clip(shape)
                .background(Chrome.plate, shape)
                .padding(horizontal = 8.dp, vertical = 8.dp)
                .semantics { isTraversalGroup = true },
        ) {
            if (scrubber) {
                Box(Modifier.semantics { isTraversalGroup = true; traversalIndex = 1f }) {
                    Scrubber(state, rtl, shownLeaf, onGo)
                }
            }
            Row(
                Modifier.fillMaxWidth().semantics { isTraversalGroup = true; traversalIndex = 0f },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onClose, modifier = Modifier.size(48.dp)) {
                    GlyphIcon(Glyph.Close, Chrome.text, modifier = Modifier.semantics { contentDescription = Strings.readerClose(context) })
                }
                Column(Modifier.weight(1f).padding(horizontal = 6.dp)) {
                    Text(title, color = Chrome.text, fontFamily = robotoFlex(FontWeight.Bold, 14.sp), fontSize = 14.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(chapterWords(context, chapter), color = Chrome.faint, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                IconButton(
                    onClick = onChapters,
                    modifier = Modifier.size(48.dp).background(if (chaptersOpen) Chrome.selected else Color.Transparent, RoundedCornerShape(24.dp)),
                ) {
                    GlyphIcon(Glyph.List, Chrome.text, modifier = Modifier.semantics { contentDescription = Strings.readerChapters(context) })
                }
                IconButton(
                    onClick = onSettings,
                    modifier = Modifier.size(48.dp).background(if (settingsOpen) Chrome.selected else Color.Transparent, RoundedCornerShape(24.dp)),
                ) {
                    Text("Aa", color = Chrome.text, fontSize = 17.sp, fontWeight = FontWeight.Medium, modifier = Modifier.semantics { contentDescription = Strings.readerSettings(context) })
                }
            }
        }
    }
}

@Composable
private fun Scrubber(state: ReaderState, rtl: Boolean, shownLeaf: Int, onGo: (Int) -> Unit) {
    val context = LocalContext.current
    val count = state.pages.coerceAtLeast(1)
    val leaf = shownLeaf.coerceIn(0, (state.leaves.size - 1).coerceAtLeast(0))
    val page = (state.leaves.getOrNull(leaf)?.parts?.firstOrNull()?.page?.toInt() ?: 0).coerceIn(0, count - 1)
    var dragging by remember { mutableStateOf<Float?>(null) }
    val at = dragging?.toInt() ?: page
    // A right-to-left chapter's scrubber runs from the right, as its pages do.
    val layout = if (rtl) LayoutDirection.Rtl else LayoutDirection.Ltr
    CompositionLocalProvider(LocalLayoutDirection provides layout) {
        Row(Modifier.fillMaxWidth().heightIn(min = 28.dp).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Text((at + 1).toString(), color = Chrome.text, fontFamily = FontFamily.Monospace, fontSize = 13.sp, modifier = Modifier.width(32.dp))
            Slider(
                value = dragging ?: page.toFloat(),
                onValueChange = { dragging = it.toInt().toFloat() },
                onValueChangeFinished = {
                    dragging?.let { target -> state.leafOf(target.toInt().toUInt())?.let(onGo) }
                    dragging = null
                },
                enabled = count > 1,
                valueRange = 0f..(count - 1).coerceAtLeast(1).toFloat(),
                steps = (count - 2).coerceAtLeast(0),
                colors = SliderDefaults.colors(
                    thumbColor = Chrome.text,
                    activeTrackColor = Chrome.text,
                    inactiveTrackColor = Chrome.rule,
                    activeTickColor = Color.Transparent,
                    inactiveTickColor = Color.Transparent,
                    disabledThumbColor = Chrome.faint,
                    disabledActiveTrackColor = Chrome.faint,
                    disabledInactiveTrackColor = Chrome.rule,
                ),
                modifier = Modifier.weight(1f).semantics {
                    contentDescription = Strings.readerScrubber(context)
                    stateDescription = Strings.readerPageOf(context, at + 1, count)
                },
            )
            Text(count.toString(), color = Chrome.faint, fontFamily = FontFamily.Monospace, fontSize = 13.sp, modifier = Modifier.width(32.dp))
        }
    }
}

/** A capsule at the top centre: 44 dp tall at least, solid, with an underlined text action. */
@Composable
private fun NoticePlate(words: String, action: String?, modifier: Modifier, onAction: () -> Unit) {
    Row(
        modifier
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .padding(top = 12.dp, start = 16.dp, end = 16.dp)
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

/** Back to page: 40 dp tall, 16 dp from the leading edge, the back glyph and the words in silk. */
@Composable
private fun BackToPage(words: String, rtl: Boolean, onClick: () -> Unit) {
    Row(
        Modifier
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .padding(start = 16.dp, bottom = 40.dp)
            .heightIn(min = 40.dp)
            .clip(RoundedCornerShape(50))
            .background(Chrome.plate)
            .clickable(onClick = onClick)
            .padding(start = 12.dp, end = 16.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        GlyphIcon(Glyph.Back, Chrome.text, size = 20.dp, mirror = rtl)
        Text(words, color = Chrome.text, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
    }
}

/** The three tap zones, shown once on the first chapter and when the direction changes. */
@Composable
private fun TapZonesHint(context: Context, direction: ReadDirection, zones: TapZones, turns: Int, chromeShown: Boolean) {
    val prefs = remember { context.getSharedPreferences("kaeri_reader_ui", Context.MODE_PRIVATE) }
    var visible by remember { mutableStateOf(prefs.getString("zones_direction", null) != direction.name) }
    LaunchedEffect(direction) {
        if (visible) {
            prefs.edit().putString("zones_direction", direction.name).apply()
            delay(5000)
            visible = false
        }
    }
    // The first touch ends it.
    LaunchedEffect(turns, chromeShown) { if (turns > 1 || chromeShown) visible = false }
    AnimatedVisibility(visible = visible, enter = fadeIn(Motion.fadeIn()), exit = fadeOut(tween300())) {
        Row(Modifier.fillMaxSize()) {
            val forward = zones.forward.endInclusive - zones.forward.start
            val middle = zones.controls.endInclusive - zones.controls.start
            val back = (1f - forward - middle).coerceAtLeast(0.01f)
            val first = if (direction.rightToLeft) forward to Strings.readerZoneNext(context) else back to Strings.readerZonePrevious(context)
            val last = if (direction.rightToLeft) back to Strings.readerZonePrevious(context) else forward to Strings.readerZoneNext(context)
            for ((weight, label) in listOf(first, middle.coerceAtLeast(0.01f) to Strings.readerZoneControls(context), last)) {
                Box(
                    Modifier.weight(weight).fillMaxSize().background(Color(0x33000000)).border(0.5.dp, Chrome.rule),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(label, color = Color.White, fontSize = 14.sp, fontWeight = FontWeight.Medium)
                }
            }
        }
    }
}

private fun tween300() = androidx.compose.animation.core.tween<Float>(300)

/** The strip's divider: what comes next, already loading; passing it counts the chapter read. */
@Composable
private fun StripDivider(next: ChapterRow) {
    val context = LocalContext.current
    Column(
        Modifier.fillMaxSize().background(Chrome.ground).padding(horizontal = 28.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Text(Strings.readerEndNext(context), color = Chrome.faint, fontSize = 13.sp)
        Text(
            next.title ?: Strings.readerChapterLine(context, chapterNumber(next)),
            color = Chrome.text,
            fontFamily = robotoFlex(FontWeight.Bold, 20.sp),
            fontSize = 20.sp,
            maxLines = 2,
        )
        Text(
            Strings.readerEndFacts(context, chapterNumber(next), next.pages?.let { Strings.readerEndPages(context, it.toInt()) }.orEmpty()),
            color = Chrome.faint,
            fontSize = 13.sp,
        )
    }
}

/** The cover's place while a chapter opens: a silk spinner and the line from 600 ms, the source line from 3 s. */
@Composable
private fun Opening(number: String?) {
    val context = LocalContext.current
    var step by remember { mutableStateOf(0) }
    LaunchedEffect(Unit) {
        delay(600)
        step = 1
        delay(2400)
        step = 2
    }
    if (step == 0) return
    Column(Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        CircularProgressIndicator(color = Chrome.text, trackColor = Chrome.rule, strokeWidth = 2.dp, modifier = Modifier.size(20.dp))
        Spacer(Modifier.height(14.dp))
        if (number != null) Text(Strings.readerOpening(context, number), color = Chrome.faint, fontSize = 14.sp)
        if (step == 2) {
            Spacer(Modifier.height(4.dp))
            Text(Strings.readerOpeningSlow(context), color = Chrome.faint, fontSize = 13.sp)
        }
    }
}

/** A chapter that would not open: left aligned, the glyph beside the title, Choose source in amber. */
@Composable
private fun Failed(title: String, line: String, onClose: () -> Unit, onChoose: () -> Unit, onRetry: () -> Unit) {
    val context = LocalContext.current
    Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
        Column(Modifier.align(Alignment.CenterStart).padding(horizontal = 28.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                GlyphIcon(Glyph.Warning, Chrome.error)
                Text(title, color = Chrome.text, fontFamily = robotoFlex(FontWeight.SemiBold, 22.sp), fontSize = 22.sp)
            }
            Spacer(Modifier.height(8.dp))
            Text(line, color = Chrome.faint, fontSize = 15.sp)
        }
        Column(
            Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(horizontal = 28.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Button(
                onClick = onChoose,
                shape = RoundedCornerShape(25.dp),
                colors = ButtonDefaults.buttonColors(containerColor = Chrome.accent, contentColor = Chrome.onAccent),
                modifier = Modifier.fillMaxWidth().height(50.dp),
            ) { Text(Strings.readerFailedChoose(context), fontWeight = FontWeight.SemiBold) }
            Row {
                TextButton(onClick = onRetry) { Text(Strings.readerTryAgain(context), color = Chrome.text) }
                TextButton(onClick = onClose) { Text(Strings.readerClose(context), color = Chrome.text) }
            }
            Spacer(Modifier.height(48.dp))
        }
    }
}
