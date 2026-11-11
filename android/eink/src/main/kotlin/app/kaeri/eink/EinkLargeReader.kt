// What the e-ink reader adds on a large panel (10.3 inch and up), as the large panel board draws it:
// Download beside Contents in the top bar, the series' own settings as segmented controls in the
// bottom bar, and the next chapter's first page and the chapters after it beside the end page.
// And on every panel, a chapter still opening after 10 seconds: which source is silent, with Back
// to series, Try again and Another source.

package app.kaeri.eink

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.ReaderWait
import app.kaeri.engine.model.SettingScope
import app.kaeri.i18n.Strings
import app.kaeri.reader.PageFileView
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.openingClock
import app.kaeri.reader.rightToLeft
import app.kaeri.reader.phone.openingStage
import kotlinx.coroutines.delay

/** The panel width from which the reader uses the room a 10.3 inch panel has, as the other e-ink screens do. */
private const val LARGE_PANEL_DP = 900

/** Whether the panel is a large one, 10.3 inch and up. */
internal val einkLargePanel: Boolean
    @Composable @ReadOnlyComposable
    get() = LocalConfiguration.current.screenWidthDp >= LARGE_PANEL_DP

/** How long a chapter opens with only its line before the page names the silent source. */
internal const val SLOW_OPEN_MS = 10_000L

/** A group's name on paper, as the board's headings: 15 sp bold in #333. */
private val GroupName = EinkType.Secondary.copy(fontWeight = FontWeight.Bold)

// The top bar

/**
 * Download in the top bar: the open chapter to this device. While it downloads it says so, and
 * once the chapter is here it says that; neither is a button.
 */
@Composable
internal fun EinkTopDownload(chapter: ChapterRow, onDownload: () -> Unit) {
    val context = LocalContext.current
    var asked by remember(chapter.id) { mutableStateOf(false) }
    when (chapter.download) {
        ChapterDownloadState.Done -> TopWords(Strings.einkDownloadDone(context))
        null, is ChapterDownloadState.Failed -> if (asked) {
            TopWords(Strings.einkDownloadRunning(context))
        } else {
            EinkTextButton(Strings.einkDownload(context), {
                asked = true
                onDownload()
            }, underline = true)
        }
        else -> TopWords(Strings.einkDownloadRunning(context))
    }
}

@Composable
private fun TopWords(text: String) {
    Text(
        text,
        style = EinkType.Secondary,
        modifier = Modifier.padding(horizontal = 12.dp).semantics { liveRegion = LiveRegionMode.Polite },
    )
}

// The bottom bar

/**
 * The series' own settings in the bottom bar: reading direction (or strip), one or two pages, and
 * cropping, each a segmented control after "This series". A change is the series' own, as in the
 * settings sheet; pages side by side stay the device's, as the engine keeps that setting. A
 * novel's settings stay in the sheet.
 */
@Composable
internal fun EinkSeriesBar(session: ReaderSession, state: ReaderState, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val view = state.view ?: return
    if (view.medium == Medium.NOVEL) return
    val rows = state.settings?.rows.orEmpty()
    fun row(key: ReaderSettingKey) = rows.firstOrNull { it.key == key }
    fun word(key: ReaderSettingKey) = (row(key)?.value as? ReaderValue.Choice)?.name
    val strip = view.mode == ReaderMode.STRIP
    val way = when {
        strip -> "strip"
        view.direction.rightToLeft -> "rtl"
        else -> "ltr"
    }
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(Strings.readerSheetSeries(context), style = GroupName, maxLines = 1)
        EinkSegmented(
            choices = listOf("rtl", "ltr", "strip"),
            chosen = way,
            label = { choice ->
                when (choice) {
                    "rtl" -> Strings.readerModeRtl(context)
                    "ltr" -> Strings.readerModeLtr(context)
                    else -> Strings.readerModeStrip(context)
                }
            },
            onChoose = { choice ->
                if (choice == way) {
                    // Already so: nothing to change.
                } else if (choice == "strip") {
                    session.setSetting(ReaderSettingKey.MODE, SettingScope.SERIES, ReaderValue.Choice("strip"))
                } else {
                    session.setSetting(ReaderSettingKey.MODE, SettingScope.SERIES, ReaderValue.Choice("paged"))
                    session.setSetting(ReaderSettingKey.DIRECTION, SettingScope.SERIES, ReaderValue.Choice(choice))
                }
            },
            modifier = Modifier.weight(390f),
        )
        val layout = row(ReaderSettingKey.PAGE_LAYOUT)
        if (!strip && layout != null) {
            val two = word(ReaderSettingKey.PAGE_LAYOUT) == "double"
            EinkSegmented(
                choices = listOf(false, true),
                chosen = two,
                label = { if (it) Strings.einkLayoutTwo(context) else Strings.einkLayoutOne(context) },
                onChoose = { if (it != two) session.setSetting(ReaderSettingKey.PAGE_LAYOUT, layout.scope, ReaderValue.Choice(if (it) "double" else "single")) },
                modifier = Modifier.weight(250f),
            )
        }
        val crop = row(ReaderSettingKey.CROP_BORDERS)
        if (crop != null) {
            val on = (crop.value as? ReaderValue.Toggle)?.on == true
            EinkSegmented(
                choices = listOf(false, true),
                chosen = on,
                label = { if (it) Strings.einkCropOn(context) else Strings.einkCropOff(context) },
                onChoose = { if (it != on) session.setSetting(ReaderSettingKey.CROP_BORDERS, SettingScope.SERIES_HERE, ReaderValue.Toggle(it)) },
                modifier = Modifier.weight(210f),
            )
        }
    }
}

// The end of a chapter

/**
 * The end of a chapter on a large panel: the end page's column on the left; beside it the next
 * chapter's first page when that chapter is on this device; under both, the chapters coming up
 * with their state and Contents, as many as fit whole; and the line about turning at the foot.
 */
@Composable
internal fun EinkLargeEnd(
    state: ReaderState,
    next: ChapterRow,
    images: PageImages?,
    onOpen: (ChapterRow) -> Unit,
    onContents: (() -> Unit)?,
    loadChapters: () -> Unit,
    modifier: Modifier = Modifier,
    column: @Composable (Modifier) -> Unit,
) {
    val context = LocalContext.current
    LaunchedEffect(state.view?.work) { if (state.volumes.isEmpty()) loadChapters() }
    val here = next.download is ChapterDownloadState.Done
    val chapters = remember(state.volumes) { state.volumes.flatMap { it.chapters } }
    val from = chapters.indexOfFirst { it.id == next.id }
    val coming = if (from >= 0) chapters.drop(from) else listOf(next)
    Box(modifier.fillMaxSize().background(EinkColours.Paper)) {
        Column(Modifier.fillMaxSize().padding(start = 64.dp, end = 64.dp, top = 96.dp, bottom = 96.dp)) {
            Row {
                column(Modifier.width(520.dp))
                if (here) {
                    Spacer(Modifier.width(56.dp))
                    Column(Modifier.weight(1f)) {
                        val file = state.preview?.takeIf { it.chapter == next.id }?.file
                        val number = app.kaeri.eink.chapterNumber(next)
                        if (images != null) {
                            PageFileView(file, images, Modifier.fillMaxWidth().aspectRatio(1f / 1.42f).border(EinkSizes.Hairline, EinkColours.Ink))
                        } else {
                            Box(Modifier.fillMaxWidth().aspectRatio(1f / 1.42f).border(EinkSizes.Hairline, EinkColours.Ink))
                        }
                        Text(Strings.einkEndPreview(context, number), style = EinkType.Secondary, modifier = Modifier.padding(top = 8.dp))
                    }
                }
            }
            Spacer(Modifier.height(40.dp))
            Row(
                Modifier.fillMaxWidth().heightIn(min = 48.dp).semantics { heading() },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(Strings.einkComingUp(context), style = EinkType.Label.copy(fontSize = 18.sp, fontWeight = FontWeight.Bold), modifier = Modifier.weight(1f))
                if (onContents != null) {
                    Text(
                        Strings.einkContents(context),
                        style = EinkType.Secondary.copy(color = EinkColours.Ink, fontWeight = FontWeight.Bold, textDecoration = TextDecoration.Underline),
                        modifier = Modifier.heightIn(min = 48.dp).einkClick(onClick = onContents).wrapContentHeight(Alignment.CenterVertically),
                    )
                }
            }
            WholeRows(Modifier.weight(1f).fillMaxWidth()) {
                coming.forEachIndexed { index, row -> ComingRow(row, first = index == 0) { onOpen(row) } }
            }
        }
        Text(
            Strings.einkTurnToContinue(context),
            style = EinkType.Tertiary,
            textAlign = TextAlign.Center,
            modifier = Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(bottom = 36.dp),
        )
    }
}

/** A chapter coming up: its number in a 64 dp column, its title, and whether it is here. */
@Composable
private fun ComingRow(row: ChapterRow, first: Boolean, onClick: () -> Unit) {
    val context = LocalContext.current
    val state = when (row.download) {
        ChapterDownloadState.Done -> Strings.einkDownloadsHere(context)
        null, is ChapterDownloadState.Failed -> null
        else -> Strings.einkDownloadsDownloading(context)
    }
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 44.dp)
            .then(if (first) Modifier else Modifier.hairlineOver())
            .einkClick(onClick = onClick)
            .padding(vertical = 11.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(app.kaeri.eink.chapterNumber(row), style = EinkType.Body.copy(fontSize = 17.sp), modifier = Modifier.width(64.dp), maxLines = 1)
        Text(row.title ?: Strings.einkChapter(context, app.kaeri.eink.chapterNumber(row)), style = EinkType.Body.copy(fontSize = 17.sp), modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        state?.let { Text(it, style = EinkType.Secondary, maxLines = 1) }
    }
}

/** A 1 dp grey hairline over what it is on, between rows of a list. */
private fun Modifier.hairlineOver(): Modifier = drawBehind {
    val y = EinkSizes.Hairline.toPx() / 2
    drawLine(EinkColours.Tertiary, Offset(0f, y), Offset(size.width, y), EinkSizes.Hairline.toPx())
}

/** Its children one under another, as many as fit whole; the rest are left out, never cut. */
@Composable
private fun WholeRows(modifier: Modifier, content: @Composable () -> Unit) {
    Layout(content, modifier) { measurables, constraints ->
        val loose = constraints.copy(minHeight = 0)
        val placeables = mutableListOf<androidx.compose.ui.layout.Placeable>()
        var used = 0
        for (m in measurables) {
            val p = m.measure(loose)
            if (used + p.height > constraints.maxHeight) break
            placeables += p
            used += p.height
        }
        layout(constraints.maxWidth, if (constraints.hasBoundedHeight) constraints.maxHeight else used) {
            var y = 0
            placeables.forEach {
                it.place(0, y)
                y += it.height
            }
        }
    }
}

// A chapter slow to open

/**
 * A chapter still opening after [SLOW_OPEN_MS]: over the waiting line, the series, what the open
 * waits on, which source hasn't answered, and Back to series, Try again and Another source when
 * the series has another. Another source lists them in place of the page, closed by Back. Drawn
 * once; a later stage does not repaint it.
 */
@Composable
internal fun BoxScope.EinkSlowOpen(state: ReaderState, session: ReaderSession, onSeries: () -> Unit) {
    val context = LocalContext.current
    var slow by remember(state.openingSince) { mutableStateOf(false) }
    LaunchedEffect(state.openingSince) {
        val since = state.openingSince.takeIf { it > 0 } ?: openingClock()
        delay((SLOW_OPEN_MS - (openingClock() - since)).coerceAtLeast(0))
        slow = true
        session.loadSources()
    }
    if (!slow) return
    val sources = state.sources.orEmpty()
    var choosing by remember { mutableStateOf(false) }
    BackHandler(enabled = choosing) { choosing = false }
    val side = if (einkLargePanel) 64.dp else 44.dp
    if (choosing && sources.isNotEmpty()) {
        Column(Modifier.fillMaxSize().background(EinkColours.Paper).padding(horizontal = side, vertical = 92.dp)) {
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
                        .einkClick(role = Role.Button) {
                            choosing = false
                            session.chooseSource(version)
                        }
                        .wrapContentHeight(Alignment.CenterVertically),
                )
            }
        }
        return
    }
    val wait = state.openingWait
    val chapter = (state.openingChapter ?: state.view?.chapter)?.let(::chapterNumber)
    val source = when (wait) {
        is ReaderWait.ChapterList -> wait.source
        is ReaderWait.ChapterPages -> wait.source
        else -> null
    }
    val title = state.view?.title ?: state.openingTitle
    val volume = (state.openingChapter ?: state.view?.chapter)?.volume
    val kicker = when {
        title != null && volume != null -> Strings.einkSlowKicker(context, title, volume)
        volume != null -> Strings.readerVolumeLine(context, volume)
        else -> title
    }
    val line = openingStage(context, wait, chapter) ?: chapter?.let { Strings.einkLoadingChapter(context, it) }
    val body = when {
        source == null -> Strings.einkSlowLineUnnamed(context)
        sources.isNotEmpty() -> Strings.einkSlowLine(context, source)
        else -> Strings.einkSlowLineAlone(context, source)
    }
    Box(Modifier.fillMaxSize().background(EinkColours.Paper)) {
        Column(Modifier.padding(start = side, end = side, top = 300.dp).semantics { liveRegion = LiveRegionMode.Polite }) {
            kicker?.let { Text(it, style = EinkType.Secondary) }
            line?.let { Text(it, style = EinkType.PageTitle.copy(lineHeight = EinkType.PageTitle.fontSize * 1.25f), modifier = Modifier.padding(top = 4.dp)) }
            Spacer(Modifier.height(32.dp))
            Box(Modifier.width(72.dp).height(3.dp).background(EinkColours.Ink))
            Spacer(Modifier.height(28.dp))
            Text(body, style = EinkType.Body.copy(lineHeight = EinkType.Body.fontSize * 1.5f), modifier = Modifier.widthIn(max = 640.dp))
            Spacer(Modifier.height(30.dp))
            val buttons = @Composable {
                EinkSolidButton(Strings.readerEndSeries(context), onSeries)
                EinkOutlinedButton(Strings.readerTryAgain(context), session::retryOpen)
                if (sources.isNotEmpty()) EinkTextButton(Strings.einkSlowAnother(context), { choosing = true }, underline = true)
            }
            if (einkLargePanel) {
                Row(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) { buttons() }
            } else {
                // A 6 inch panel has no room for three across: one under another, as wide as their words.
                Column(verticalArrangement = Arrangement.spacedBy(14.dp)) { buttons() }
            }
        }
    }
}
