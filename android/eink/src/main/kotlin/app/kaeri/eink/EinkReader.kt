// The e-ink reader, on the :reader surface: pages turn at once by the 40/30/30 zones, the page
// buttons and swipes; the bars are opaque with a 2 dp rule and no dim; the folio sits in the
// page's own margin; the page between chapters is in the flow; nothing animates; and each paint
// goes through the engine's refresh policy to the vendor's full refresh.

package app.kaeri.eink

import androidx.compose.foundation.background
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
import app.kaeri.reader.BrokenPageGlyph
import app.kaeri.reader.PageImages
import app.kaeri.reader.PagedReader
import app.kaeri.reader.PagedReaderState
import app.kaeri.reader.ReaderAction
import app.kaeri.reader.ReaderKeyRules
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.ShownNotice
import app.kaeri.reader.TapZones
import app.kaeri.reader.readerKeys
import app.kaeri.reader.rememberPagedReaderState
import app.kaeri.reader.rightToLeft
import app.kaeri.reader.phone.endReadLine
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
 * The e-ink reader for [session]. [showZones] puts the tap zone guide over the first chapter
 * until the first touch. Every paint goes through [painter].
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
    swapVolume: Boolean = false,
) {
    val view = state.view
    val direction = view?.direction ?: ReadDirection.RIGHT_TO_LEFT
    val host = LocalView.current
    var chrome by remember { mutableStateOf(false) }
    var zones by remember { mutableStateOf(showZones) }
    var layer by remember { mutableStateOf(EinkLayer.None) }
    val paint by rememberUpdatedState { what: EinkPaint -> painter.paint(host, what) }

    fun toggleChrome() {
        chrome = !chrome
        paint(if (chrome) EinkPaint.ChromeOpen else EinkPaint.ChromeClose)
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
        if (layer == EinkLayer.Settings || layer == EinkLayer.More) paint(EinkPaint.SheetChange)
    }

    // Back closes the sheet or Contents first, then the reader, never the app's screens under it.
    BackHandler {
        when (layer) {
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

    fun act(action: ReaderAction) {
        when (action) {
            ReaderAction.FORWARD -> pager.forward(TurnHow.KEY)
            ReaderAction.BACK -> pager.back(TurnHow.KEY)
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
            .readerKeys(ReaderKeyRules(direction, volumeTurns = state.settings?.rows?.firstOrNull { it.key == ReaderSettingKey.VOLUME_KEYS_TURN }?.let { (it.value as? ReaderValue.Toggle)?.on } ?: true, volumeDownForward = swapVolume, holdCleans = painter.controller.available), ::act)
            // The first touch anywhere clears the guide, and still turns or opens what it touched.
            .pointerInput(zones) {
                if (!zones) return@pointerInput
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                    zones = false
                }
            },
    ) {
        val failure = state.failure
        when {
            failure != null && state.leaves.isEmpty() -> EinkChapterFailed(failure, state, session, onRetry = { session.retry(null) }, onBack = actions.onBack)
            state.leaves.isEmpty() -> EinkWaiting(view?.chapter?.let(::chapterNumber))
            else -> PagedReader(
                state = state,
                pager = pager,
                images = images,
                direction = direction,
                zones = TapZones.eink(direction),
                onControls = ::toggleChrome,
                onShown = { leaf, how ->
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
                endPage = { EinkEndPage(state, actions, onContinue = { next -> session.openChapter(next.id) }, onFirstPage = { pager.go(0, TurnHow.JUMP) }) },
                waiting = { page, long -> if (long) EinkPageWaiting(page) },
                failed = { page, failure, retry -> EinkPageFailed(page, failure, retry, skip = { pager.forward(TurnHow.TURN) }) },
                fit = (state.settings?.rows?.firstOrNull { it.key == ReaderSettingKey.FIT }?.value as? ReaderValue.Choice)?.name,
            )
        }
        val leaf = state.leaves.getOrNull(pager.leaf)
        if (leaf != null && !leaf.endPage && !chrome) EinkFolio(leaf, state)
        if (zones && state.leaves.isNotEmpty()) EinkZonesGuide(direction)
        if (chrome && view != null && layer != EinkLayer.Contents) {
            EinkTopBar(
                title = view.title,
                chapter = view.chapter,
                onBack = actions.onBack,
                onContents = { show(EinkLayer.Contents) },
                modifier = Modifier.align(Alignment.TopCenter),
                subtitle = view.demoCredit?.let { demo -> demo.licence?.let { Strings.readerDemoCredit(LocalContext.current, demo.series, demo.author, it) } ?: demo.credit },
            )
            val page = (leaf?.parts?.firstOrNull()?.page?.toInt() ?: 0) + 1
            EinkBottomBar(
                page = page,
                pages = state.pages,
                rtl = direction.rightToLeft,
                backTo = (state.notice?.notice?.action == NoticeAction.BACK_TO).takeIf { it }?.let { state.notice },
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
        if (notice != null && !chrome && notice.notice.action != NoticeAction.BACK_TO) {
            EinkNotice(notice, onAction = { action ->
                when (action) {
                    NoticeAction.START_OVER -> session.startOver()
                    NoticeAction.BACK_TO -> session.backTo()
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
            EinkLayer.More -> EinkMoreSheet(session, state, onBack = { show(EinkLayer.Settings) }, onDone = { show(EinkLayer.None) })
            EinkLayer.Contents -> EinkContents(
                session = session,
                state = state,
                onBack = { show(EinkLayer.None) },
                onOpen = { chapter ->
                    if (chapter != view?.chapter?.id) session.openChapter(chapter)
                    show(EinkLayer.None)
                },
                onPage = { paint(EinkPaint.ListPage(false)) },
            )
            EinkLayer.None -> Unit
        }
    }
}

/** The chapter's number as the strings name it: its canonical number, or the source's label. */
fun chapterNumber(row: ChapterRow): String = row.number ?: row.label

/** The tap zone guide: three bands with their names, forward at the reading side. */
@Composable
fun BoxScope.EinkZonesGuide(direction: ReadDirection) {
    val context = LocalContext.current
    val rtl = direction.rightToLeft
    val forward = 0.4f
    val controls = 0.3f
    @Composable
    fun RowScope.Band(weight: Float, title: String, line: String?) {
        Box(
            Modifier.fillMaxHeight().weight(weight).border(EinkSizes.Rule, EinkColours.Ink).background(EinkColours.Paper.copy(alpha = 0.92f)),
            contentAlignment = Alignment.Center,
        ) {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Text(title, style = EinkType.Label, textAlign = TextAlign.Center)
                if (line != null) Text(line, style = EinkType.Secondary, textAlign = TextAlign.Center)
            }
        }
    }
    Row(Modifier.matchParentSize()) {
        val next = Strings.readerActionNext(context)
        val how = if (rtl) Strings.einkZoneNextRtl(context) else Strings.einkZoneNextLtr(context)
        val middle = Strings.readerActionControls(context)
        val tap = Strings.einkZoneTap(context)
        val back = Strings.readerActionPrevious(context)
        if (rtl) {
            Band(forward, next, how)
            Band(controls, middle, tap)
            Band(1f - forward - controls, back, null)
        } else {
            Band(1f - forward - controls, back, null)
            Band(controls, middle, tap)
            Band(forward, next, how)
        }
    }
}


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
        val pages = leaf.parts.map { it.page.toInt() + 1 }.distinct().sorted()
        val words = when (pages.size) {
            1 -> Strings.einkFolioOne(context, pages[0], state.pages)
            else -> Strings.einkFolioTwo(context, pages.first(), pages.last(), state.pages)
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
    backTo: ShownNotice?,
    hasNext: Boolean,
    hasPrevious: Boolean,
    canClean: Boolean,
    onBackTo: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onSettings: () -> Unit,
    onClean: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    Column(
        modifier.fillMaxWidth().background(EinkColours.Paper).topRule().navigationBarsPadding().heightIn(min = EinkSizes.BottomBar).padding(horizontal = 20.dp, vertical = 14.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Row(verticalAlignment = Alignment.Bottom) {
            Text(Strings.readerPageOf(context, page, pages.coerceAtLeast(1)), style = EinkType.Label)
            Spacer(Modifier.width(12.dp))
            Text(Strings.einkPagesLeft(context, (pages - page).coerceAtLeast(0)), style = EinkType.Secondary)
        }
        EinkProgress(page.toFloat() / pages.coerceAtLeast(1), fromRight = rtl)
        if (backTo != null) {
            EinkOutlinedButton(backTo.notice.msg.text(context), onBackTo, height = 46.dp)
        }
        // Mirrored for left to right: the next chapter is on the reading side.
        val next = @Composable { if (hasNext) EinkTextButton(if (rtl) "‹ " + Strings.einkNextChapter(context) else Strings.einkNextChapter(context) + " ›", onNext) }
        val previous = @Composable { if (hasPrevious) EinkTextButton(if (rtl) Strings.einkPreviousChapter(context) + " ›" else "‹ " + Strings.einkPreviousChapter(context), onPrevious) }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            if (rtl) next() else previous()
            Row {
                EinkTextButton(Strings.einkSettings(context), onSettings)
                if (canClean) EinkTextButton(Strings.einkCleanScreen(context), onClean)
            }
            if (rtl) previous() else next()
        }
    }
}

/** A 6 dp ink bar with a 1 dp edge, filled from the right when reading right to left. */
@Composable
fun EinkProgress(fraction: Float, fromRight: Boolean, modifier: Modifier = Modifier) {
    Box(
        modifier.fillMaxWidth().height(8.dp).border(EinkSizes.Hairline, EinkColours.Tertiary).drawBehind {
            val w = size.width * fraction.coerceIn(0f, 1f)
            drawRect(EinkColours.Ink, topLeft = Offset(if (fromRight) size.width - w else 0f, 0f), size = androidx.compose.ui.geometry.Size(w, size.height))
        },
    )
}

/** Nothing for 600 ms, then one static line, painted once: "Loading chapter 13". */
@Composable
fun BoxScope.EinkWaiting(chapter: String?) {
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(600)
        shown = true
    }
    if (!shown || chapter == null) return
    Text(
        Strings.einkLoadingChapter(LocalContext.current, chapter),
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
                NoticeAction.BACK_TO -> notice.notice.msg.text(context)
                NoticeAction.GO_BACK -> Strings.readerNoticeGoBack(context)
                NoticeAction.CHANGE_MODE -> Strings.readerNoticeChange(context)
            }
            EinkTextButton(words, { onAction(action) }, underline = true)
        }
    }
}
