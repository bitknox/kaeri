// The page between chapters on the phone: where the chapter ended and which trackers heard of it,
// whether the book server has it, what is next and how it reads, or why nothing is. It is a leaf of
// its own, turned to like any page. Its one amber is Continue, or Back to the series when nothing
// is next. A demo work ends on the demo's card instead.

package app.kaeri.reader.phone

import android.content.Context
import android.net.ConnectivityManager
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.platform.LocalContext
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.ui.graphics.luminance
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.isTraversalGroup
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.traversalIndex
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.Motion
import app.kaeri.design.SyncIcons
import app.kaeri.design.robotoFlex
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.DemoWorkInfo
import app.kaeri.engine.model.NoNext
import app.kaeri.i18n.Strings
import app.kaeri.reader.HeldLine
import app.kaeri.reader.PageFileView
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.TextThenLink
import app.kaeri.reader.rememberServerLine
import app.kaeri.reader.serverLineWords
import app.kaeri.store.ServerLine

@Composable
fun PhoneEndPage(
    state: ReaderState,
    session: ReaderSession,
    actions: PhoneReaderActions,
    images: PageImages,
    modifier: Modifier = Modifier,
    /** Back to page 1, from the demo's card. */
    onFirstPage: () -> Unit = {},
    /** The series' cover, on the demo's card. */
    cover: (@Composable (Modifier) -> Unit)? = null,
    /** The reader's page background: the demo's card follows it, light on a white or paper page. */
    pageGround: Color? = null,
) {
    val view = state.view ?: return
    val demo = view.demoCredit
    if (demo != null) {
        val light = pageGround != null && pageGround.luminance() > 0.5f
        CompositionLocalProvider(LocalChrome provides if (light) LightChrome else LocalChrome.current) {
            DemoEndCard(demo, actions, onFirstPage, cover, pageGround ?: Chrome.ground, modifier)
        }
        return
    }
    val context = LocalContext.current
    val end = state.end
    val next = end?.next ?: view.next
    val number = chapterNumber(view.chapter)
    val server = rememberServerLine(view.work, view.chapter.id)
    val read = view.chapter.id in state.read
    val continueFocus = remember { FocusRequester() }
    val backFocus = remember { FocusRequester() }
    val retryFocus = remember { FocusRequester() }
    val retrying = server.line?.canRetry == true
    Box(modifier.fillMaxSize().background(Chrome.ground).windowInsetsPadding(WindowInsets.safeDrawing).semantics { isTraversalGroup = true }) {
        // Swipe above the amber button, the amber button at 88 dp from the foot, the text button at 40;
        // each grows with larger text. They come first, so Continue is the first stop and Try again on
        // the line follows Back.
        val finished = next == null && end?.whyNone == NoNext.FINISHED
        val second = next != null || finished
        var foot by remember { mutableIntStateOf(0) }
        Column(
            Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .padding(start = 28.dp, end = 28.dp, bottom = 40.dp)
                .onSizeChanged { foot = it.height },
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            if (next != null) {
                Text(
                    Strings.readerEndSwipe(context),
                    color = Chrome.faint,
                    fontSize = 13.sp,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(12.dp))
            }
            Button(
                onClick = { if (next != null) session.openChapter(next.id) else actions.onSeries() },
                shape = RoundedCornerShape(25.dp),
                colors = ButtonDefaults.buttonColors(containerColor = Chrome.accent, contentColor = Chrome.onAccent),
                contentPadding = PaddingValues(horizontal = 24.dp, vertical = 12.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = 50.dp)
                    .focusRequester(continueFocus)
                    .focusProperties {
                        when {
                            second -> this.next = backFocus
                            retrying -> this.next = retryFocus
                        }
                    },
            ) {
                Text(
                    if (next != null) Strings.readerEndContinue(context) else Strings.readerEndSeries(context),
                    fontSize = 16.sp,
                    fontWeight = FontWeight.SemiBold,
                    textAlign = TextAlign.Center,
                )
            }
            Spacer(Modifier.height(10.dp))
            val secondModifier = Modifier
                .heightIn(min = 38.dp)
                .focusRequester(backFocus)
                .focusProperties {
                    previous = continueFocus
                    if (retrying) this.next = retryFocus
                }
            when {
                next != null -> TextButton(onClick = actions.onSeries, modifier = secondModifier) {
                    Text(Strings.readerEndSeries(context), color = Chrome.text, fontSize = 15.sp, textAlign = TextAlign.Center)
                }
                finished -> TextButton(onClick = session::readAgain, modifier = secondModifier) {
                    Text(Strings.readerEndAgain(context), color = Chrome.text, fontSize = 15.sp, textAlign = TextAlign.Center)
                }
                else -> Spacer(Modifier.height(38.dp))
            }
        }
        // Larger text scrolls the page above the buttons.
        Column(
            Modifier
                .fillMaxSize()
                .padding(bottom = with(LocalDensity.current) { foot.toDp() } + 64.dp)
                .verticalScroll(rememberScrollState())
                .padding(start = 28.dp, end = 28.dp, top = 100.dp),
        ) {
            Text(Strings.readerEndOfChapterNumber(context, number), color = Chrome.faint, fontSize = 15.sp, lineHeight = 20.sp)
            // With a server line coming, the tracker line's place is kept too, so nothing under it moves.
            if (read || server.held) {
                Spacer(Modifier.height(4.dp))
                Text(
                    endReadLine(context, state, view.chapter.id),
                    color = Chrome.faint,
                    fontSize = 13.sp,
                    lineHeight = 18.sp,
                    modifier = if (read) Modifier else Modifier.alpha(0f).clearAndSetSemantics {},
                )
            }
            if (server.held) {
                Spacer(Modifier.height(2.dp))
                PhoneServerLine(
                    server,
                    retryFocus,
                    after = if (second) backFocus else continueFocus,
                    Modifier.fillMaxWidth(),
                )
            }
            if (next != null) {
                Spacer(Modifier.height(if (server.held) 18.dp else 34.dp))
                Text(Strings.readerEndNext(context), color = Chrome.faint, fontSize = 13.sp)
                Spacer(Modifier.height(4.dp))
                Text(
                    next.title ?: Strings.readerChapterLine(context, chapterNumber(next)),
                    color = Chrome.text,
                    fontFamily = robotoFlex(FontWeight.Bold, 30.sp),
                    fontSize = 30.sp,
                    lineHeight = 38.sp,
                )
                Spacer(Modifier.height(4.dp))
                val facts = endFacts(context, next, end?.source?.takeIf { end.next?.id == next.id })
                if (facts.isNotEmpty()) Text(facts, color = Chrome.faint, fontSize = 14.sp, lineHeight = 20.sp)
                // The next chapter's first page, once the engine made it; its plate until then. A
                // novel's chapter has no page to show.
                if (view.medium != Medium.NOVEL) {
                    Spacer(Modifier.height(22.dp))
                    PageFileView(
                        file = state.preview?.takeIf { it.chapter == next.id }?.file,
                        images = images,
                        modifier = Modifier.width(170.dp).height(242.dp),
                    ) {
                        // With no preview the plate names the chapter, as the page plate names its page.
                        Box(Modifier.fillMaxSize().background(Chrome.plate)) {
                            Text(
                                Strings.readerChapterLine(context, chapterNumber(next)),
                                color = Chrome.faint,
                                fontSize = 15.sp,
                                textAlign = TextAlign.Center,
                                modifier = Modifier.align(Alignment.Center).padding(horizontal = 12.dp),
                            )
                        }
                    }
                }
            } else {
                val why = end?.whyNone ?: NoNext.CAUGHT_UP
                val following = view.chapter.number?.toDoubleOrNull()?.let { (it.toInt() + 1).toString() } ?: ""
                val (title, line) = when (why) {
                    NoNext.NO_SOURCE_HAS_IT, NoNext.NOT_OUT_YET -> Strings.readerEndNone(context, following) to Strings.readerEndNoneLine(context)
                    NoNext.CAUGHT_UP -> Strings.readerEndCaughtUp(context) to Strings.readerEndNoneLine(context)
                    NoNext.FINISHED -> Strings.readerEndFinishedTitle(context) to Strings.readerEndFinishedLine(context)
                    NoNext.OFFLINE -> Strings.readerEndOfflineTitle(context, following) to Strings.readerEndOfflineLine(context)
                }
                Spacer(Modifier.height(if (server.held) 26.dp else 44.dp))
                Text(title, color = Chrome.text, fontFamily = robotoFlex(FontWeight.SemiBold, 22.sp), fontSize = 22.sp, lineHeight = 28.sp)
                Spacer(Modifier.height(6.dp))
                Text(line, color = Chrome.faint, fontSize = 15.sp, lineHeight = 21.sp)
            }
        }
    }
}

/**
 * The server line in its held two-line space: 13 sp in haze, a 13 dp glyph before it, Try again
 * underlined in silk after a line that can retry. Each change crossfades in place on reveal.
 * It is a polite live region, and comes after the page's buttons in reading order.
 */
@Composable
private fun PhoneServerLine(server: HeldLine, retryFocus: FocusRequester, after: FocusRequester, modifier: Modifier) {
    val context = LocalContext.current
    // Two lines' space, laid out as the line's own text is, so a second line moves nothing.
    Box(modifier) {
        Text("\n", fontSize = 13.sp, lineHeight = 18.sp, modifier = Modifier.alpha(0f).clearAndSetSemantics {})
        PhoneServerLineText(server, retryFocus, after, context)
    }
}

@Composable
private fun PhoneServerLineText(server: HeldLine, retryFocus: FocusRequester, after: FocusRequester, context: Context) {
    AnimatedContent(
        targetState = server.line,
        transitionSpec = { fadeIn(Motion.reveal()) togetherWith fadeOut(Motion.reveal()) },
        contentAlignment = Alignment.TopStart,
        label = "server line", // i18n-skip
        modifier = Modifier.fillMaxWidth().semantics {
            liveRegion = LiveRegionMode.Polite
            traversalIndex = 1f
        },
    ) { line ->
        if (line == null) return@AnimatedContent
        val failed = line is ServerLine.Failed
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            LineGlyph(line, Modifier.padding(top = if (line is ServerLine.Sending) 3.dp else 2.5.dp))
            TextThenLink(
                text = { onLayout ->
                    Text(serverLineWords(context, line), color = if (failed) Chrome.text else Chrome.faint, fontSize = 13.sp, lineHeight = 18.sp, onTextLayout = onLayout)
                },
                link = {
                    if (line.canRetry) Text(
                        Strings.readerTryAgain(context),
                        color = Chrome.text,
                        fontSize = 13.sp,
                        lineHeight = 18.sp,
                        textDecoration = TextDecoration.Underline,
                        softWrap = false,
                        modifier = Modifier
                            .focusRequester(retryFocus)
                            .focusProperties { previous = after }
                            .clickable(role = Role.Button, onClick = server::retry),
                    )
                },
                gap = 4.dp,
                modifier = Modifier.weight(1f, fill = false),
            )
        }
    }
}

/** The glyph before the line: none for Saved, a 12 dp spinner, wifi off, sync, or the vermilion warn. */
@Composable
private fun LineGlyph(line: ServerLine, modifier: Modifier) {
    when (line) {
        is ServerLine.Saved -> Unit
        is ServerLine.Sending -> CircularProgressIndicator(modifier.size(12.dp), color = Chrome.faint, strokeWidth = 1.5.dp, trackColor = Color.Transparent)
        is ServerLine.Offline -> Icon(WifiOff, contentDescription = null, tint = Chrome.faint, modifier = modifier.size(13.dp))
        is ServerLine.Retrying -> Icon(SyncIcons.Sync, contentDescription = null, tint = Chrome.faint, modifier = modifier.size(13.dp))
        is ServerLine.Failed -> WarnMark(Chrome.error, Chrome.ground, 13.dp, modifier)
    }
}

/** The board's warn glyph: a filled triangle with the mark cut in [mark]. */
@Composable
internal fun WarnMark(colour: Color, mark: Color, size: Dp, modifier: Modifier = Modifier) {
    Canvas(modifier.size(size)) {
        val s = this.size.minDimension / 24f
        val triangle = Path().apply {
            moveTo(12f * s, 3.2f * s)
            lineTo(1.8f * s, 20.5f * s)
            lineTo(22.2f * s, 20.5f * s)
            close()
        }
        drawPath(triangle, colour)
        drawPath(triangle, colour, style = Stroke(width = 2f * s, join = StrokeJoin.Round))
        drawLine(mark, Offset(12f * s, 9.5f * s), Offset(12f * s, 14.5f * s), strokeWidth = 2f * s, cap = StrokeCap.Round)
        drawCircle(mark, radius = 1.2f * s, center = Offset(12f * s, 17.4f * s))
    }
}

/** No network: the signal's arcs with a stroke through them, as the app draws it. */
private val WifiOff: ImageVector = ImageVector.Builder("WifiOff", 24.dp, 24.dp, 24f, 24f).apply {
    listOf(
        "M2.5 8.5 C5 6.3 8 5 11.2 4.9 M15.6 5.6 C17.9 6.2 19.8 7.2 21.5 8.5",
        "M5.5 12 C6.8 10.9 8.3 10.2 9.9 9.9 M15.8 10.9 C16.8 11.2 17.7 11.6 18.5 12.2",
        "M8.8 15.5 C10.6 14.2 13.4 14.2 15.2 15.5",
        "M12 19 V19.01",
        "M4 3 L20 21",
    ).forEach { path ->
        addPath(
            pathData = addPathNodes(path),
            stroke = SolidColor(Color.Black),
            strokeLineWidth = 1.8f,
            strokeLineCap = StrokeCap.Round,
            strokeLineJoin = StrokeJoin.Round,
        )
    }
}.build()

/**
 * The end of the demo, after its last page instead of a next chapter, on the page's own ground so
 * the last turn does not flash: a card a third of the way down, its cover, the line on adding your own books, Add a source in amber and Back to page 1.
 * It comes up on reveal.
 */
@Composable
private fun DemoEndCard(
    demo: DemoWorkInfo,
    actions: PhoneReaderActions,
    onFirstPage: () -> Unit,
    cover: (@Composable (Modifier) -> Unit)?,
    ground: Color,
    modifier: Modifier,
) {
    val context = LocalContext.current
    val shown = remember { MutableTransitionState(false).apply { targetState = true } }
    BoxWithConstraints(modifier.fillMaxSize().background(ground)) {
        val top = maxHeight * 0.3f
        AnimatedVisibility(
            visibleState = shown,
            enter = fadeIn(Motion.reveal()),
            modifier = Modifier.align(Alignment.TopCenter).padding(top = top),
        ) {
            Column(
                Modifier
                    .padding(horizontal = 24.dp)
                    .widthIn(max = 480.dp)
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(20.dp))
                    .background(Chrome.plate)
                    .border(1.dp, Chrome.rule, RoundedCornerShape(20.dp))
                    .padding(start = 24.dp, end = 24.dp, top = 28.dp, bottom = 22.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                val art = Modifier.width(72.dp).height(108.dp).clip(RoundedCornerShape(6.dp))
                if (cover != null) cover(art) else Box(art.background(Chrome.selected))
                Spacer(Modifier.height(18.dp))
                Text(
                    Strings.readerDemoEnd(context),
                    color = Chrome.text,
                    fontFamily = robotoFlex(FontWeight.Bold, 20.sp),
                    fontSize = 20.sp,
                    lineHeight = 26.sp,
                    textAlign = TextAlign.Center,
                )
                Spacer(Modifier.height(8.dp))
                Text(Strings.readerDemoEndLine(context), color = Chrome.faint, fontSize = 15.sp, lineHeight = 24.sp, textAlign = TextAlign.Center)
                Spacer(Modifier.height(22.dp))
                Button(
                    onClick = actions.onAddSource,
                    shape = RoundedCornerShape(20.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = Chrome.accent, contentColor = Chrome.onAccent),
                    modifier = Modifier.fillMaxWidth().height(40.dp),
                ) { Text(Strings.readerDemoAddSource(context), fontSize = 14.sp, fontWeight = FontWeight.Medium) }
                Spacer(Modifier.height(8.dp))
                Button(
                    onClick = onFirstPage,
                    shape = RoundedCornerShape(20.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = Chrome.selected, contentColor = Chrome.text),
                    modifier = Modifier.fillMaxWidth().height(40.dp),
                ) { Text(Strings.readerDemoAgain(context), fontSize = 14.sp, fontWeight = FontWeight.Medium) }
            }
        }
    }
}

/** "Chapter 13 · 34 pages", with "· from" its source when it comes from one. */
internal fun endFacts(context: Context, next: ChapterRow, source: String?): String {
    // The heading already names an untitled chapter, and a novel's chapter has no page count.
    val chapter = next.title?.let { chapterNumber(next) }
    val pages = next.pages?.takeIf { it > 0u }?.let { Strings.readerEndPages(context, it.toInt()) }
    val facts = when {
        chapter != null && pages != null -> Strings.readerEndFacts(context, chapter, pages)
        chapter != null -> Strings.readerChapterLine(context, chapter)
        else -> pages
    }
    return when {
        source == null -> facts.orEmpty()
        facts == null -> Strings.readerEndFrom(context, source)
        else -> Strings.readerDividerFactsSource(context, facts, source)
    }
}

/**
 * The line once a chapter counts as read, on the phone's and the e-ink end pages alike: "Counted as
 * read", with the trackers it went to, or the offline form when there is no network.
 */
fun endReadLine(context: Context, state: ReaderState, chapter: app.kaeri.engine.core.ChapterId): String {
    val names = state.readTrackers[chapter].orEmpty()
    if (names.isEmpty()) return Strings.readerEndRead(context)
    val manager = context.getSystemService(ConnectivityManager::class.java)
    val online = manager?.activeNetwork != null
    return if (online) {
        Strings.readerEndTracker(context, android.icu.text.ListFormatter.getInstance().format(names))
    } else {
        Strings.readerEndTrackerOffline(context, android.icu.text.ListFormatter.getInstance().format(names))
    }
}
