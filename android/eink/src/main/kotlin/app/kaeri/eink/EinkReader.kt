// The e-ink reader, on the :reader surface: pages turn at once by the engine's tap zones (Sides
// at Standard is 40/30/30), the page buttons and swipes, which a switch can stop; the zone guide is
// opaque, drawn in one refresh and cleared in one full refresh; the bars are opaque with a 2 dp rule and no dim; the folio sits in the
// page's own margin; the page between chapters is in the flow; nothing animates; and each paint
// goes through the engine's refresh policy to the vendor's full refresh.

package app.kaeri.eink

import androidx.compose.foundation.background
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.semantics.Role
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.clickable
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.EinkPaint
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.NoNext
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.TurnHow
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ReaderAction as NoticeAction
import app.kaeri.i18n.Strings
import app.kaeri.reader.phone.ReaderLook
import app.kaeri.reader.BrokenPageGlyph
import app.kaeri.reader.PageImages
import app.kaeri.reader.PagedReader
import app.kaeri.reader.PagedReaderState
import app.kaeri.reader.ReaderAction
import app.kaeri.reader.ReaderKeyRules
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.ShownNotice
import app.kaeri.reader.Arrow
import app.kaeri.reader.CellLabel
import app.kaeri.reader.ShowTapGuideWhenDue
import app.kaeri.reader.TapGuide
import app.kaeri.reader.clearsTapGuide
import app.kaeri.reader.drawCells
import app.kaeri.reader.rememberScreenReader
import app.kaeri.reader.tapReading
import app.kaeri.reader.tapZones
import app.kaeri.engine.model.TapAction
import app.kaeri.engine.model.TapFlow
import app.kaeri.engine.model.TapMap
import app.kaeri.engine.model.TapReading
import app.kaeri.engine.model.tapMap
import androidx.compose.foundation.Canvas
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import app.kaeri.reader.readerKeys
import app.kaeri.reader.rememberPagedReaderState
import app.kaeri.reader.rightToLeft
import app.kaeri.reader.phone.endReadLine
import app.kaeri.reader.phone.openingStage
import app.kaeri.reader.text
import app.kaeri.store.text
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.semantics.isTraversalGroup
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.traversalIndex
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.sp
import app.kaeri.reader.HeldLine
import app.kaeri.reader.TextThenLink
import app.kaeri.reader.CreditNotice
import app.kaeri.reader.CreditShot
import app.kaeri.reader.partTicks
import app.kaeri.reader.openCredits
import app.kaeri.reader.creditNotice
import app.kaeri.reader.endSkipped
import app.kaeri.reader.onUnkeptCredit
import app.kaeri.reader.paged
import app.kaeri.reader.shownCredit
import app.kaeri.reader.shownPage
import app.kaeri.reader.shownPages
import app.kaeri.reader.startSkipped
import app.kaeri.reader.rememberServerLine
import app.kaeri.reader.serverLineWords
import app.kaeri.store.ServerLine
import kotlinx.coroutines.delay

/** What the e-ink reader leaves to the app around it; its settings and Contents are its own. */
class EinkReaderActions(
    val onBack: () -> Unit,
    val onSeries: () -> Unit,
    val onDownloads: () -> Unit,
    /** Add a source, from the end of the demo. */
    val onAddSource: () -> Unit = onBack,
)

/**
 * The e-ink reader for [session]. The tap zone guide shows when the engine says it is due, and
 * [showZones] puts it over the first chapter whatever the engine says. Either way it stays until
 * the first touch. Every paint goes through [painter].
 */
@Composable
fun EinkReader(
    session: ReaderSession,
    state: ReaderState,
    images: PageImages,
    painter: EinkPainter,
    actions: EinkReaderActions,
    modifier: Modifier = Modifier,
    pager: PagedReaderState = rememberPagedReaderState(),
    showZones: Boolean = false,
) {
    val view = state.view
    val direction = view?.direction ?: ReadDirection.RIGHT_TO_LEFT
    val host = LocalView.current
    var chrome by remember { mutableStateOf(false) }
    // The engine's tap zones and their guide. A screen reader turns the zones off.
    val screenReader = rememberScreenReader()
    val taps = state.tapZones(eink = true)
    val reading = state.tapReading(zoomed = pager.zoomed, screenReader = screenReader)
    val guide = remember { TapGuide() }
    // Asked for at open: once, as soon as a chapter is open.
    var forced by remember { mutableStateOf(showZones) }
    LaunchedEffect(forced, reading != null) {
        if (forced && reading != null) {
            forced = false
            if (!screenReader) guide.show(tapMap(taps, reading, null))
        }
    }
    var layer by remember { mutableStateOf(EinkLayer.None) }
    val paint by rememberUpdatedState { what: EinkPaint -> painter.paint(host, what) }

    fun toggleChrome() {
        chrome = !chrome
        paint(if (chrome) EinkPaint.ChromeOpen else EinkPaint.ChromeClose)
    }

    // Drawn in one refresh; the first touch clears it with a full refresh, and still does what it touched.
    LaunchedEffect(guide.up) { if (guide.up) paint(EinkPaint.SheetChange) }
    fun guideCleared(map: TapMap) {
        guide.gone()
        paint(EinkPaint.Clean)
        if (!screenReader) session.tapGuideSeen(map)
    }

    fun show(next: EinkLayer) {
        layer = next
        when (next) {
            EinkLayer.None -> {
                chrome = false
                paint(EinkPaint.ChromeClose)
            }
            EinkLayer.Contents -> Unit // Contents paints each of its pages.
            else -> paint(EinkPaint.SheetChange)
        }
    }

    // A setting changed under the sheet: the page above it is painted again once it has changed.
    LaunchedEffect(state.settings, state.leaves) {
        if (layer == EinkLayer.Settings || layer == EinkLayer.More || layer == EinkLayer.TapZones) paint(EinkPaint.SheetChange)
    }

    // Back closes the sheet or Contents first, then the reader, never the app's screens under it.
    BackHandler {
        when (layer) {
            EinkLayer.TapZones -> show(EinkLayer.More)
            EinkLayer.More -> show(EinkLayer.Settings)
            EinkLayer.Settings, EinkLayer.Contents -> show(EinkLayer.None)
            EinkLayer.None -> actions.onBack()
        }
    }

    // A demo work opens with the chrome, its credit in the top bar, for 3 s.
    LaunchedEffect(view?.demoCredit != null, view?.chapter?.id) {
        if (view?.demoCredit == null) return@LaunchedEffect
        if (!chrome) toggleChrome()
        delay(DEMO_CREDIT_MILLIS)
        if (chrome) toggleChrome()
    }

    // A kept credit page reaches the engine before the app can be killed in the background.
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    DisposableEffect(lifecycle, session) {
        val observer = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_STOP) session.commitKeep() }
        lifecycle.addObserver(observer)
        onDispose { lifecycle.removeObserver(observer) }
    }

    // A Fixtures shot's step, once the credit pages it staged have come.
    val creditShot = CreditShot.step
    val staged = state.openCredits.isNotEmpty() || state.parts != null
    LaunchedEffect(creditShot, staged) {
        if (creditShot == null || !staged) return@LaunchedEffect
        delay(600)
        // Without the fixture's resume notice, as a chapter opened fresh.
        session.state.value.notice?.let { session.dismissNotice(it.id) }
        when (creditShot) {
            "line" -> pager.go(0, TurnHow.JUMP)
            "chrome", "parts" -> if (!chrome) toggleChrome()
            "shown", "shownchrome", "kept" -> {
                session.showStartCredits()?.let { pager.show(it, TurnHow.TURN, session::showCredit, null) }
                delay(400)
                if (creditShot == "kept") session.keepCredit()
                if (creditShot != "shown" && creditShot != "kept" && !chrome) toggleChrome()
            }
            "end" -> pager.go(session.state.value.leaves.lastIndex, TurnHow.JUMP)
        }
    }

    val contentsKeys = remember { ContentsKeys() }
    fun act(action: ReaderAction) {
        // A page button clears the guide as a touch does, and still turns.
        guide.clear()?.let(::guideCleared)
        when (action) {
            // Contents turns its own pages, not the reader's under it.
            ReaderAction.FORWARD -> if (layer == EinkLayer.Contents) contentsKeys.forward() else pager.forward(TurnHow.KEY)
            ReaderAction.BACK -> if (layer == EinkLayer.Contents) contentsKeys.back() else pager.back(TurnHow.KEY)
            ReaderAction.CONTROLS -> toggleChrome()
            ReaderAction.CLEAN -> paint(EinkPaint.Clean)
            ReaderAction.FIRST -> pager.go(0, TurnHow.JUMP)
            ReaderAction.LAST -> pager.go(state.leaves.indexOfLast { !it.endPage }.coerceAtLeast(0), TurnHow.JUMP)
            ReaderAction.ESCAPE -> when {
                layer != EinkLayer.None -> show(EinkLayer.None)
                chrome -> toggleChrome()
                else -> actions.onBack()
            }
            ReaderAction.GO_TO -> if (!chrome) toggleChrome()
        }
    }

    Box(
        modifier
            .fillMaxSize()
            .background(EinkColours.Paper)
            .readerKeys(ReaderKeyRules(direction, volumeTurns = state.settings?.rows?.firstOrNull { it.key == ReaderSettingKey.VOLUME_KEYS_TURN }?.let { (it.value as? ReaderValue.Toggle)?.on } ?: true, volumeDownForward = !ReaderLook.volumeSwapped(LocalContext.current), holdCleans = painter.controller.available), ::act)
            // The first touch anywhere clears the guide, and still turns or opens what it touched.
            .clearsTapGuide(guide, ::guideCleared),
    ) {
        val failure = state.failure
        when {
            failure != null && state.leaves.isEmpty() -> EinkChapterFailed(failure, state, session, onRetry = { session.retry(null) }, onBack = actions.onBack)
            state.leaves.isEmpty() -> EinkWaiting(state)
            else -> PagedReader(
                state = state,
                pager = pager,
                images = images,
                direction = direction,
                taps = taps,
                reading = reading ?: TapReading(ReaderMode.PAGED, direction, false, screenReader),
                onControls = ::toggleChrome,
                swipes = taps.swipesTurn,
                onShown = { leaf, how, _ ->
                    session.shown(leaf, how)
                    if (state.leaves.getOrNull(leaf)?.endPage == true) paint(EinkPaint.EndPage)
                },
                onMissing = session::needPage,
                onRetry = { session.retry(it.toUInt()) },
                onJumped = session::jumped,
                onReady = { leaf ->
                    val colour = state.leaves.getOrNull(leaf)?.parts?.any { state.file(it)?.colour == true } == true
                    paint(EinkPaint.ImagePage(colour))
                },
                motion = false,
                endPage = {
                    EinkEndPage(
                        state,
                        actions,
                        onContinue = { next -> session.openChapter(next.id) },
                        onFirstPage = { pager.go(0, TurnHow.JUMP) },
                        onShowCredits = { session.showEndCredits()?.let { pager.show(it, TurnHow.TURN, session::showCredit, state.shownCredit) } },
                    )
                },
                // A page button, a volume key or a forward tap on the end page goes on, as on the phone.
                onPastEnd = session::crossOn,
                waiting = { page, long -> if (long) EinkPageWaiting(page) },
                failed = { page, failure, retry -> EinkPageFailed(page, failure, retry, skip = { pager.forward(TurnHow.TURN) }) },
                fit = (state.settings?.rows?.firstOrNull { it.key == ReaderSettingKey.FIT }?.value as? ReaderValue.Choice)?.name,
                onCredit = session::showCredit,
                creditGround = EinkColours.Paper,
            )
        }
        // A credit page shown or put away over the same leaf is a new image: one full refresh.
        val credit = state.shownCredit
        var creditWas by remember { mutableStateOf<UInt?>(null) }
        LaunchedEffect(state.onCredit) {
            val was = creditWas
            creditWas = state.onCredit
            if (state.onCredit == was) return@LaunchedEffect
            val colour = credit?.file?.colour ?: (state.leaves.getOrNull(pager.leaf)?.parts?.any { state.file(it)?.colour == true } == true)
            paint(EinkPaint.ImagePage(colour))
        }
        val leaf = state.leaves.getOrNull(pager.leaf)
        if (leaf != null && !leaf.endPage && !chrome && !state.onUnkeptCredit) EinkFolio(leaf, state)
        // Page 1's box names the credit pages skipped; it stays on page 1 and leaves with the turn.
        val skipped = state.startSkipped
        var turnedAway by remember(view?.chapter?.id) { mutableStateOf(false) }
        if (pager.leaf != 0 && state.leaves.isNotEmpty()) turnedAway = true
        if (state.paged && pager.leaf == 0 && credit == null && skipped.isNotEmpty() && (!turnedAway || CreditShot.step == "line") && !chrome && layer == EinkLayer.None && state.notice == null && view?.demoCredit == null) {
            EinkCreditLine(skipped.size, onShow = {
                session.showStartCredits()?.let { pager.show(it, TurnHow.TURN, session::showCredit, state.shownCredit) }
            }, modifier = Modifier.align(Alignment.TopCenter))
        }
        // The box says a page was kept, with Undo, until the next turn, and stays gone after it; no
        // timer, since a timed change costs a refresh. The kept page reaches the engine when the
        // chapter closes, another opens, another page is kept, or the app goes to the background.
        val keeping = state.keeping
        val here = pager.leaf to state.onCredit
        val keptAt = remember(keeping) { here }
        var keptGone by remember(keeping) { mutableStateOf(false) }
        LaunchedEffect(here, keptAt) { if (here != keptAt) keptGone = true }
        val creditNotice = state.creditNotice?.takeIf { it != CreditNotice.KEPT || (!keptGone && keptAt == here) }
        if (creditNotice != null && !chrome && layer == EinkLayer.None) {
            EinkCreditNotice(creditNotice, session, Modifier.align(Alignment.TopCenter))
        }
        ShowTapGuideWhenDue(
            guide,
            zones = state.settings?.tapZones,
            reading = reading,
            ready = state.leaves.isNotEmpty() && failure == null && layer == EinkLayer.None && !chrome,
        )
        val shownGuide = guide.map
        if (guide.up && shownGuide != null && state.leaves.isNotEmpty()) EinkTapGuide(shownGuide, taps.swipesTurn, rtl = direction.rightToLeft)
        if (chrome && view != null && layer != EinkLayer.Contents) {
            EinkTopBar(
                title = view.title,
                chapter = view.chapter,
                onBack = actions.onBack,
                onContents = { show(EinkLayer.Contents) },
                modifier = Modifier.align(Alignment.TopCenter),
                subtitle = view.demoCredit?.let { demo -> demo.licence?.let { Strings.readerDemoCredit(LocalContext.current, demo.series, demo.author, it) } ?: demo.credit },
            )
            val page = state.shownPage(leaf?.parts?.firstOrNull()?.page?.toInt() ?: 0) + 1
            EinkBottomBar(
                page = page,
                pages = state.shownPages,
                onCredit = state.onUnkeptCredit,
                parts = state.partTicks.map { state.shownPage(it.page) to it.missing },
                rtl = direction.rightToLeft,
                backTo = state.backTo,
                hasNext = view.next != null,
                hasPrevious = view.previous != null,
                canClean = painter.controller.available,
                onBackTo = { session.backTo() },
                onNext = { view.next?.let { session.openChapter(it.id) } },
                onPrevious = { view.previous?.let { session.openChapter(it.id) } },
                onSettings = { show(EinkLayer.Settings) },
                onClean = { paint(EinkPaint.Clean) },
                modifier = Modifier.align(Alignment.BottomCenter),
            )
        }
        val notice = state.notice
        if (notice != null && !chrome) {
            EinkNotice(notice, onAction = { action ->
                when (action) {
                    NoticeAction.START_OVER -> session.startOver()
                    NoticeAction.GO_BACK -> session.goBack()
                    NoticeAction.CHANGE_MODE -> {
                        chrome = true
                        show(EinkLayer.Settings)
                    }
                }
                session.dismissNotice(notice.id)
            }, modifier = Modifier.align(Alignment.BottomCenter))
        }
        when (layer) {
            EinkLayer.Settings -> EinkSettingsSheet(session, state, onMore = { show(EinkLayer.More) }, onDone = { show(EinkLayer.None) })
            EinkLayer.More -> EinkMoreSheet(session, state, onBack = { show(EinkLayer.Settings) }, onDone = { show(EinkLayer.None) }, onTapZones = { show(EinkLayer.TapZones) })
            EinkLayer.TapZones -> EinkTapZonesSheet(
                session,
                state,
                onBack = { show(EinkLayer.More) },
                onDone = { show(EinkLayer.None) },
                onShowZones = {
                    show(EinkLayer.None)
                    reading?.let { guide.show(tapMap(taps, it, null)) }
                },
            )
            EinkLayer.Contents -> EinkContents(
                session = session,
                state = state,
                onBack = { show(EinkLayer.None) },
                onOpen = { chapter ->
                    if (chapter != view?.chapter?.id) session.openChapter(chapter)
                    show(EinkLayer.None)
                },
                onPage = { paint(EinkPaint.ListPage(false)) },
                keys = contentsKeys,
            )
            EinkLayer.None -> Unit
        }
    }
}

/** The chapter's number as the strings name it: its canonical number, or the source's label. */
fun chapterNumber(row: ChapterRow): String = row.number ?: row.label

/**
 * The tap zone guide on e-ink: the engine's cells opaque in black, grey and white with a 2 dp rule
 * where two kinds meet, each labelled cell's name and arrow, how Next takes a touch, and the note
 * at the foot. A swipe toward the reading side turns, so Next says which way. Nothing moves.
 */
@Composable
fun BoxScope.EinkTapGuide(map: TapMap, swipes: Boolean, rtl: Boolean) {
    val context = LocalContext.current
    val foot = Strings.einkZoneGuideNote(context)
    val sideways = map.flow == TapFlow.SIDEWAYS
    BoxWithConstraints(Modifier.matchParentSize()) {
        val w = maxWidth
        val h = maxHeight
        Canvas(Modifier.fillMaxSize()) {
            drawCells(
                map.cells,
                colour = {
                    when (it.action) {
                        TapAction.NEXT -> EinkGuideNext
                        TapAction.PREVIOUS -> EinkGuidePrevious
                        TapAction.CONTROLS -> EinkColours.Paper
                    }
                },
                line = EinkColours.Ink,
                lineWidth = EinkSizes.Rule.toPx(),
            )
        }
        map.cells.filter { it.labelled }.forEach { cell ->
            val next = cell.action == TapAction.NEXT
            val ink = if (next) EinkColours.Paper else EinkColours.Ink
            val how = when {
                next && sideways && swipes -> if (rtl) Strings.einkZoneNextRtl(context) else Strings.einkZoneNextLtr(context)
                next && sideways -> Strings.einkZoneTap(context)
                cell.action == TapAction.CONTROLS -> Strings.einkZoneTap(context)
                else -> null
            }
            CellLabel(cell, w, h, foot = EinkGuideFoot) {
                cell.arrow?.let { Arrow(it, 26.dp, ink) }
                Text(cell.name, style = EinkType.Label.copy(color = ink, fontWeight = FontWeight.Bold, fontSize = 19.sp, lineHeight = 24.sp), textAlign = TextAlign.Center)
                if (how != null) Text(how, style = EinkType.Secondary.copy(color = ink, fontWeight = FontWeight.Medium), textAlign = TextAlign.Center)
            }
        }
        Text(
            foot,
            style = EinkType.Secondary.copy(color = EinkColours.Ink),
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .background(EinkColours.Paper)
                .topRule()
                .navigationBarsPadding()
                .padding(horizontal = 24.dp, vertical = 14.dp),
        )
    }
}

/** The guide's Next and Previous on e-ink: dark and light grey, opaque. */
private val EinkGuideNext = Color(0xFF4A4A4A)
private val EinkGuidePrevious = Color(0xFFD4D4D4)

/** Room the note takes at the foot, which the labels keep above. */
private val EinkGuideFoot = 108.dp


/** The folio, centred in the page's bottom margin, drawn only where the page leaves one. */
@Composable
fun BoxScope.EinkFolio(leaf: Leaf, state: ReaderState) {
    val context = LocalContext.current
    BoxWithConstraints(Modifier.matchParentSize()) {
        val density = LocalDensity.current
        val width = constraints.maxWidth.toFloat()
        val height = constraints.maxHeight.toFloat()
        // The lowest edge any page reaches, fitted into its place.
        val bottom = leaf.parts.maxOfOrNull { part ->
            val file = state.file(part)
            val boxW = part.place.width * width
            val boxH = part.place.height * height
            val drawnH = if (file == null) boxH else {
                val w = (file.crop?.width ?: file.width).toFloat().coerceAtLeast(1f)
                val h = (file.crop?.height ?: file.height).toFloat().coerceAtLeast(1f)
                val (iw, ih) = if (part.rotate) h to w else w to h
                minOf(boxW / iw, boxH / ih) * ih
            }
            part.place.y * height + (boxH + drawnH) / 2
        } ?: height
        val margin = with(density) { (height - bottom).toDp() }
        if (margin < 24.dp) return@BoxWithConstraints
        val pages = leaf.parts.map { state.shownPage(it.page.toInt()) + 1 }.distinct().sorted()
        val words = when (pages.size) {
            1 -> Strings.einkFolioOne(context, pages[0], state.shownPages)
            else -> Strings.einkFolioTwo(context, pages.first(), pages.last(), state.shownPages)
        }
        Box(
            Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(margin),
            contentAlignment = Alignment.Center,
        ) {
            Text(words, style = EinkType.Folio)
        }
    }
}

/** The top bar: back, the series and chapter, and Contents. Opaque, a 2 dp rule under it. */
@Composable
fun EinkTopBar(
    title: String,
    chapter: ChapterRow,
    onBack: () -> Unit,
    onContents: () -> Unit,
    modifier: Modifier = Modifier,
    /** In place of the chapter line: a demo work's credit while it is held. */
    subtitle: String? = null,
) {
    val context = LocalContext.current
    Row(
        modifier.fillMaxWidth().background(EinkColours.Paper).bottomRule().statusBarsPadding().height(EinkSizes.TopBar).padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier.size(56.dp).semantics { contentDescription = Strings.einkBack(context) }.einkClick(onClick = onBack),
            contentAlignment = Alignment.Center,
        ) { Text("‹", style = EinkType.PageTitle) }
        Column(Modifier.weight(1f).padding(horizontal = 8.dp)) {
            Text(title, style = EinkType.ReaderTitle, maxLines = 1)
            val number = chapterNumber(chapter)
            Text(
                subtitle ?: chapter.title?.let { Strings.einkChapterTitled(context, number, it) } ?: Strings.einkChapter(context, number),
                style = EinkType.Secondary,
                maxLines = 1,
            )
        }
        EinkTextButton(Strings.einkContents(context), onContents)
    }
}

/** The bottom bar: where the page is, how much is left, the progress, and the reader's actions. */
@Composable
fun EinkBottomBar(
    page: Int,
    pages: Int,
    rtl: Boolean,
    /** Back to page while it is offered: the page counted from 1. */
    backTo: UInt?,
    hasNext: Boolean,
    hasPrevious: Boolean,
    canClean: Boolean,
    onBackTo: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onSettings: () -> Unit,
    onClean: () -> Unit,
    modifier: Modifier = Modifier,
    /** A credit page shows, only brought back: it has no number, and the total stays. */
    onCredit: Boolean = false,
    /** Where each later part starts, as a page from 0, and whether no source has it. */
    parts: List<Pair<Int, Boolean>> = emptyList(),
) {
    val context = LocalContext.current
    Column(
        modifier.fillMaxWidth().background(EinkColours.Paper).topRule().navigationBarsPadding().heightIn(min = EinkSizes.BottomBar).padding(horizontal = 20.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(verticalAlignment = Alignment.Bottom) {
            Text(if (onCredit) Strings.readerCreditPageLabel(context) else Strings.readerPageOf(context, page, pages.coerceAtLeast(1)), style = EinkType.Label)
            Spacer(Modifier.width(12.dp))
            Text(Strings.einkPagesLeft(context, (pages - if (onCredit) 0 else page).coerceAtLeast(0)), style = EinkType.Secondary)
        }
        EinkProgress(
            if (onCredit) 0f else page.toFloat() / pages.coerceAtLeast(1),
            fromRight = rtl,
            ticks = parts.map { (at, missing) -> at.toFloat() / pages.coerceAtLeast(1) to missing },
        )
        if (backTo != null) {
            EinkOutlinedButton(Strings.readerBackToPage(context, backTo.toInt()), onBackTo, height = 46.dp)
        }
        // Mirrored for left to right: the next chapter is on the reading side.
        // The chapter buttons share what the middle leaves, each on one line.
        val next = @Composable { m: Modifier -> if (hasNext) EinkTextButton(if (rtl) "‹ " + Strings.einkNextChapter(context) else Strings.einkNextChapter(context) + " ›", onNext, m, oneLine = true) }
        val previous = @Composable { m: Modifier -> if (hasPrevious) EinkTextButton(if (rtl) Strings.einkPreviousChapter(context) + " ›" else "‹ " + Strings.einkPreviousChapter(context), onPrevious, m, oneLine = true) }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) { if (rtl) next(Modifier) else previous(Modifier) }
            Row {
                EinkTextButton(Strings.einkSettings(context), onSettings)
                if (canClean) EinkTextButton(Strings.einkCleanScreen(context), onClean)
            }
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterEnd) { if (rtl) previous(Modifier) else next(Modifier) }
        }
    }
}

/**
 * A 6 dp ink bar with a 1 dp edge, filled from the right when reading right to left. A tick marks
 * where each later part of the chapter starts: ink through the bar, paper where it is filled, and
 * hollow, an ink outline, where a part no source has would start.
 */
@Composable
fun EinkProgress(fraction: Float, fromRight: Boolean, modifier: Modifier = Modifier, ticks: List<Pair<Float, Boolean>> = emptyList()) {
    Box(
        modifier.fillMaxWidth().height(8.dp).border(EinkSizes.Hairline, EinkColours.Tertiary).drawBehind {
            val filled = fraction.coerceIn(0f, 1f)
            val w = size.width * filled
            drawRect(EinkColours.Ink, topLeft = Offset(if (fromRight) size.width - w else 0f, 0f), size = androidx.compose.ui.geometry.Size(w, size.height))
            val wide = 2.dp.toPx()
            for ((at, missing) in ticks) {
                val f = at.coerceIn(0f, 1f)
                val x = (if (fromRight) 1f - f else f) * size.width
                val top = Offset(x - wide / 2, -2.dp.toPx())
                val tick = androidx.compose.ui.geometry.Size(wide, size.height + 4.dp.toPx())
                if (missing) {
                    // Hollow: paper inside an ink outline, which reads on a 1-bit panel as well.
                    val hollow = androidx.compose.ui.geometry.Size(4.dp.toPx(), tick.height)
                    val corner = Offset(x - hollow.width / 2, top.y)
                    drawRect(EinkColours.Paper, topLeft = corner, size = hollow)
                    drawRect(EinkColours.Ink, topLeft = corner, size = hollow, style = androidx.compose.ui.graphics.drawscope.Stroke(EinkSizes.Hairline.toPx()))
                } else {
                    drawRect(if (f < filled) EinkColours.Paper else EinkColours.Ink, topLeft = top, size = tick)
                }
            }
        },
    )
}

/**
 * Nothing for 600 ms, then one static line, painted once: what the open waits on then, as the
 * engine named it ("Getting chapter 13 from …"), else "Loading chapter 13". A later stage does not
 * repaint it, and there is no counter: each repaint flashes the panel.
 */
@Composable
fun BoxScope.EinkWaiting(state: ReaderState) {
    val context = LocalContext.current
    var line by remember { mutableStateOf<String?>(null) }
    val now by rememberUpdatedState(state)
    LaunchedEffect(Unit) {
        delay(600)
        val chapter = (now.openingChapter ?: now.view?.chapter)?.let(::chapterNumber)
        line = if (now.openingWait != null) openingStage(context, now.openingWait, chapter) else chapter?.let { Strings.einkLoadingChapter(context, it) }
    }
    val shown = line ?: return
    Text(
        shown,
        style = EinkType.Waiting,
        textAlign = TextAlign.Center,
        modifier = Modifier.align(Alignment.TopCenter).offset(y = 300.dp).padding(horizontal = 24.dp),
    )
}

/** A page still on its way after four seconds: the one static line. */
@Composable
fun BoxScope.EinkPageWaiting(page: Int) {
    Text(
        Strings.readerPageLoading(LocalContext.current, page + 1),
        style = EinkType.Waiting,
        modifier = Modifier.align(Alignment.Center),
    )
}

/** A page that did not load: the glyph, why, Try again and Skip page. */
@Composable
fun BoxScope.EinkPageFailed(page: Int, failure: ReadingFailure, retry: () -> Unit, skip: () -> Unit) {
    val context = LocalContext.current
    Column(
        Modifier.align(Alignment.Center).widthIn(max = 480.dp).padding(44.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        BrokenPageGlyph(EinkColours.Ink, Modifier.size(52.dp))
        Text(Strings.readerPageFailed(context, page + 1), style = EinkType.ReaderTitle, textAlign = TextAlign.Center)
        Text(failure.text(context), style = EinkType.Body.copy(color = EinkColours.Secondary), textAlign = TextAlign.Center)
        EinkSolidButton(Strings.readerTryAgain(context), retry, Modifier.fillMaxWidth())
        EinkOutlinedButton(Strings.readerSkipPage(context), skip, Modifier.fillMaxWidth())
        Text(Strings.einkPageFailedDetails(context), style = EinkType.Tertiary, textAlign = TextAlign.Center)
    }
}

/**
 * The chapter itself could not open. When another source has the chapter, Choose source is the
 * solid button and Try again the outlined one, and Back is the system's; otherwise Try again and
 * Back to the series. The sources are a plain list in place of the screen, closed by Back.
 */
@Composable
fun BoxScope.EinkChapterFailed(failure: ReadingFailure, state: ReaderState, session: ReaderSession, onRetry: () -> Unit, onBack: () -> Unit) {
    val context = LocalContext.current
    LaunchedEffect(Unit) { session.loadSources() }
    val sources = state.sources.orEmpty()
    var choosing by remember { mutableStateOf(false) }
    BackHandler(enabled = choosing) { choosing = false }
    if (choosing && sources.isNotEmpty()) {
        Column(Modifier.fillMaxSize().background(EinkColours.Paper).padding(horizontal = 44.dp, vertical = 92.dp)) {
            Text(Strings.readerSourcesTitle(context), style = EinkType.Secondary)
            Spacer(Modifier.height(14.dp))
            sources.forEach { version ->
                Box(Modifier.fillMaxWidth().height(EinkSizes.Rule).background(EinkColours.Ink))
                Text(
                    version.edition.sourceName,
                    style = EinkType.RowTitle,
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(min = 64.dp)
                        .clickable(role = Role.Button) {
                            choosing = false
                            session.chooseSource(version)
                        }
                        .wrapContentHeight(Alignment.CenterVertically),
                )
            }
        }
        return
    }
    Column(
        Modifier.align(Alignment.Center).widthIn(max = 480.dp).padding(44.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        BrokenPageGlyph(EinkColours.Ink, Modifier.size(52.dp))
        Text(failure.text(context), style = EinkType.Body, textAlign = TextAlign.Center)
        if (sources.isNotEmpty()) {
            EinkSolidButton(Strings.readerFailedChoose(context), { choosing = true }, Modifier.fillMaxWidth())
            EinkOutlinedButton(Strings.readerTryAgain(context), onRetry, Modifier.fillMaxWidth())
        } else {
            EinkSolidButton(Strings.readerTryAgain(context), onRetry, Modifier.fillMaxWidth())
            EinkOutlinedButton(Strings.readerEndSeries(context), onBack, Modifier.fillMaxWidth())
        }
    }
}

/** The page between chapters: what was read, and what comes next or why nothing does. */
@Composable
fun EinkEndPage(
    state: ReaderState,
    actions: EinkReaderActions,
    onContinue: (ChapterRow) -> Unit,
    modifier: Modifier = Modifier,
    /** Back to page 1, from the end of the demo. */
    onFirstPage: () -> Unit = {},
    /** Show the credit pages skipped at the chapter's end. */
    onShowCredits: () -> Unit = {},
) {
    val context = LocalContext.current
    val view = state.view ?: return
    if (view.demoCredit != null) {
        EinkDemoEnd(actions.onAddSource, onFirstPage, modifier)
        return
    }
    val server = rememberServerLine(view.work, view.chapter.id)
    val read = view.chapter.id in state.read
    // Continue, Back to the series, then Try again on the line.
    val first = remember { FocusRequester() }
    val second = remember { FocusRequester() }
    val retry = remember { FocusRequester() }
    val end = state.end
    val next = end?.next ?: view.next
    val number = chapterNumber(view.chapter)
    Column(modifier.fillMaxSize().background(EinkColours.Paper).padding(horizontal = 44.dp, vertical = 92.dp).semantics { isTraversalGroup = true }) {
        Text(Strings.einkEndOfChapter(context, number), style = EinkType.Secondary.copy(fontSize = EinkType.Secondary.fontSize * (16f / 15f)))
        Spacer(Modifier.height(8.dp))
        Text(view.chapter.title ?: view.title, style = EinkType.PageTitle)
        if (read || server.held) {
            Spacer(Modifier.height(8.dp))
            // With a server line coming the counted line's place is kept, so nothing under it moves.
            Text(
                endReadLine(context, state, view.chapter.id),
                style = EinkType.Secondary,
                modifier = if (read) Modifier else Modifier.alpha(0f).clearAndSetSemantics {},
            )
        }
        if (server.held) {
            Spacer(Modifier.height(2.dp))
            EinkServerLine(server, retry, after = if (next != null) second else FocusRequester.Default, Modifier.fillMaxWidth())
        }
        val skipped = state.endSkipped(view.chapter.id)
        if (skipped.isNotEmpty()) {
            Spacer(Modifier.height(4.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(Strings.readerCreditPagesSkipped(context, skipped.size), style = EinkType.Secondary)
                EinkTextButton(Strings.readerCreditShow(context), onShowCredits, underline = true)
            }
        }
        Spacer(Modifier.height(if (server.held) 14.dp else 28.dp))
        Box(Modifier.width(72.dp).height(3.dp).background(EinkColours.Ink))
        Spacer(Modifier.height(28.dp))
        if (next != null) {
            val nextNumber = chapterNumber(next)
            Text(Strings.einkEndNext(context), style = EinkType.Secondary)
            Spacer(Modifier.height(6.dp))
            Text(Strings.einkChapter(context, nextNumber), style = EinkType.SeriesTitle)
            next.title?.let { Text(it, style = EinkType.RowTitle.copy(fontSize = EinkType.RowTitle.fontSize * (22f / 18f))) }
            Spacer(Modifier.height(8.dp))
            Text(nextFacts(context, next, view.mode, view.direction), style = EinkType.Secondary)
            Spacer(Modifier.height(36.dp))
            EinkSolidButton(Strings.einkContinueTo(context, nextNumber), { onContinue(next) }, Modifier.fillMaxWidth().focusRequester(first).focusProperties { this.next = second })
            Spacer(Modifier.height(14.dp))
            EinkOutlinedButton(
                Strings.readerEndSeries(context),
                actions.onSeries,
                Modifier.fillMaxWidth().focusRequester(second).focusProperties {
                    previous = first
                    if (server.line?.canRetry == true) this.next = retry
                },
            )
            Spacer(Modifier.weight(1f))
            Text(Strings.einkTurnToContinue(context), style = EinkType.Tertiary, modifier = Modifier.align(Alignment.CenterHorizontally))
        } else {
            val why = end?.whyNone ?: NoNext.CAUGHT_UP
            val nextNumber = view.chapter.number?.toDoubleOrNull()?.let { (it.toInt() + 1).toString() }
            val (title, line) = when (why) {
                NoNext.NO_SOURCE_HAS_IT -> (nextNumber?.let { Strings.einkNextMissing(context, it) } ?: Strings.einkCaughtUp(context)) to Strings.einkChecksAgain(context)
                NoNext.NOT_OUT_YET -> Strings.einkNextNotOut(context) to Strings.einkChecksAgain(context)
                NoNext.CAUGHT_UP -> Strings.einkCaughtUp(context) to Strings.einkChecksAgain(context)
                NoNext.FINISHED -> Strings.einkFinished(context) to Strings.einkFinishedLine(context)
                NoNext.OFFLINE -> Strings.einkNextOffline(context) to Strings.einkNextOfflineLine(context)
            }
            Text(title, style = EinkType.PageTitle.copy(fontSize = EinkType.RowTitle.fontSize * (22f / 18f)))
            Spacer(Modifier.height(10.dp))
            Text(line, style = EinkType.Body.copy(color = EinkColours.Secondary))
            Spacer(Modifier.height(36.dp))
            if (why == NoNext.OFFLINE) {
                EinkSolidButton(Strings.einkOpenDownloads(context), actions.onDownloads, Modifier.fillMaxWidth())
                Spacer(Modifier.height(14.dp))
                EinkOutlinedButton(Strings.readerEndSeries(context), actions.onSeries, Modifier.fillMaxWidth())
            } else {
                EinkSolidButton(Strings.readerEndSeries(context), actions.onSeries, Modifier.fillMaxWidth())
            }
        }
    }
}

/** How long a demo work's credit stays in the top bar after the reader opens. */
private const val DEMO_CREDIT_MILLIS = 3_000L

/**
 * The server line on e-ink: 16 sp in #333, no glyph for offline or trying again, the ink warn
 * before not saved, and Sending as a still ring; nothing turns or fades. Try again is underlined
 * in ink. A polite live region, read after the page's buttons.
 */
@Composable
private fun EinkServerLine(server: HeldLine, retry: FocusRequester, after: FocusRequester, modifier: Modifier) {
    val context = LocalContext.current
    val line = server.line
    val style = EinkType.Secondary.copy(fontSize = 16.sp, lineHeight = 21.sp)
    Box(modifier.semantics { liveRegion = LiveRegionMode.Polite; traversalIndex = 1f }) {
        // Two lines' space, laid out as the line's own text is, so a second line moves nothing.
        Text("\n", style = style, modifier = Modifier.alpha(0f).clearAndSetSemantics {})
        if (line == null) return@Box
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            when (line) {
                is ServerLine.Sending -> Box(
                    Modifier.padding(top = 2.5.dp).size(16.dp).border(2.dp, EinkColours.Ink, androidx.compose.foundation.shape.CircleShape),
                )
                is ServerLine.Failed -> EinkWarn(Modifier.padding(top = 2.5.dp).size(16.dp))
                else -> Unit
            }
            TextThenLink(
                text = { onLayout -> Text(serverLineWords(context, line), style = style, onTextLayout = onLayout) },
                link = {
                    if (line.canRetry) Text(
                        Strings.readerTryAgain(context),
                        style = style.copy(color = EinkColours.Ink, textDecoration = TextDecoration.Underline),
                        softWrap = false,
                        modifier = Modifier.focusRequester(retry).focusProperties { previous = after }.einkClick(onClick = server::retry),
                    )
                },
                gap = 5.dp,
                modifier = Modifier.weight(1f, fill = false),
            )
        }
    }
}

/** The board's e-ink warn: an ink triangle with the mark cut in paper. */
@Composable
private fun EinkWarn(modifier: Modifier) {
    androidx.compose.foundation.Canvas(modifier) {
        val s = size.minDimension / 24f
        val triangle = androidx.compose.ui.graphics.Path().apply {
            moveTo(12f * s, 3.2f * s)
            lineTo(1.8f * s, 20.5f * s)
            lineTo(22.2f * s, 20.5f * s)
            close()
        }
        drawPath(triangle, EinkColours.Ink)
        drawLine(EinkColours.Paper, Offset(12f * s, 9.5f * s), Offset(12f * s, 14.5f * s), strokeWidth = 2f * s, cap = androidx.compose.ui.graphics.StrokeCap.Round)
        drawCircle(EinkColours.Paper, radius = 1.2f * s, center = Offset(12f * s, 17.4f * s))
    }
}

/**
 * The end of the demo on e-ink: no card, the title and its line centred from 300 dp, Add a source
 * in ink and Back to page 1 outlined, each as wide as its words.
 */
@Composable
private fun EinkDemoEnd(onAddSource: () -> Unit, onFirstPage: () -> Unit, modifier: Modifier) {
    val context = LocalContext.current
    Column(
        modifier.fillMaxSize().background(EinkColours.Paper).padding(start = 30.dp, end = 30.dp, top = 300.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(Strings.readerDemoEnd(context), style = EinkType.PageTitle, textAlign = TextAlign.Center)
        Spacer(Modifier.height(12.dp))
        Text(Strings.readerDemoEndLine(context), style = EinkType.Secondary.copy(fontSize = 18.sp, lineHeight = 24.sp), textAlign = TextAlign.Center)
        Spacer(Modifier.height(28.dp))
        EinkSolidButton(Strings.readerDemoAddSource(context), onAddSource)
        Spacer(Modifier.height(14.dp))
        EinkOutlinedButton(Strings.readerDemoAgain(context), onFirstPage)
    }
}

/** "26 pages · right to left · on this device", from what the row knows. */
private fun nextFacts(context: android.content.Context, next: ChapterRow, mode: ReaderMode, direction: ReadDirection): String = listOfNotNull(
    next.pages?.let { Strings.einkEndPages(context, it.toInt()) },
    when {
        mode == ReaderMode.STRIP -> Strings.einkReadsStrip(context)
        direction.rightToLeft -> Strings.einkReadsRtl(context)
        else -> Strings.einkReadsLtr(context)
    },
    Strings.einkOnDevice(context).takeIf { next.download is ChapterDownloadState.Done },
).joinToString(" · ")

/** A notice on e-ink: a bordered plate with its action, kept until the next turn. */
@Composable
fun EinkNotice(notice: ShownNotice, onAction: (NoticeAction) -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val action = notice.notice.action
    Row(
        modifier
            .padding(20.dp)
            .fillMaxWidth()
            .background(EinkColours.Paper, RoundedCornerShape(12.dp))
            .border(EinkSizes.Rule, EinkColours.Ink, RoundedCornerShape(12.dp))
            .padding(horizontal = 18.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(notice.notice.msg.text(context), style = EinkType.Body, modifier = Modifier.weight(1f))
        if (action != null) {
            val words = when (action) {
                NoticeAction.START_OVER -> Strings.readerNoticeStartOver(context)
                NoticeAction.GO_BACK -> Strings.readerNoticeGoBack(context)
                NoticeAction.CHANGE_MODE -> Strings.readerNoticeChange(context)
            }
            EinkTextButton(words, { onAction(action) }, underline = true)
        }
    }
}
