// A reading series' page on a large e-ink panel, as the e-ink large panel board draws it (decision
// 5): the 220 × 330 cover beside the title, the medium line, the facts on one line, the genres,
// the whole synopsis and the buttons; then a 2 dp rule, the chapter heading and the chapters as
// one-line rows in two columns, down the left column and then the right, paged at the foot.
// Turning a page redraws only the chapter block. Smaller panels keep the page in SeriesPage.kt.

package app.kaeri.ui.reading

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.text
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkOutlinedButton
import app.kaeri.eink.EinkPager
import app.kaeri.eink.EinkSizes
import app.kaeri.eink.EinkSolidButton
import app.kaeri.eink.EinkTextButton
import app.kaeri.eink.EinkTheme
import app.kaeri.eink.EinkType
import app.kaeri.eink.PaperWaiting
import app.kaeri.eink.einkClick
import app.kaeri.eink.einkFocus
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.DownloadPick
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.SeriesAction
import app.kaeri.engine.model.SeriesView
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.ReadChapters
import app.kaeri.store.text
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore

/** A panel this many dp across and wider is drawn as the e-ink large panel board draws the 10.3 inch. */
internal const val EINK_LARGE_DP = 900

/** Whether the page is on an e-ink panel at least [EINK_LARGE_DP] across, upright or on its side. */
internal val einkLarge: Boolean
    @Composable @ReadOnlyComposable
    get() = LocalEinkLayout.current != null && LocalConfiguration.current.screenWidthDp >= EINK_LARGE_DP

/** Between list columns, with a 1 dp rule in its middle. */
internal val PaperListGap = 40.dp

/**
 * A list column is about this wide at its narrowest: two columns upright on the 10.3 inch panel
 * and three on its side, as the board lays its lists.
 */
private val PaperListLeast = 380.dp

/** How many list columns a width holds: as many of at least [PaperListLeast] as fit, one at least. */
internal fun paperListColumns(width: Dp): Int =
    ((width + PaperListGap) / (PaperListLeast + PaperListGap)).toInt().coerceAtLeast(1)

/** The board's cover on the large series page. */
private val SeriesCover = 220.dp

/** The chapter heading's height. */
private val ChapterHead = 52.dp

/** A one-line chapter row is its words' height and this, 48 dp at the least, a whole touch target. */
private val ChapterRowPad = 22.dp
private val ChapterRowLeast = 48.dp

/** The number column of a chapter row. */
private val ChapterNumber = 64.dp

/** Under the pager, above the bottom navigation. */
private val PagerFoot = 12.dp

/** A 1 dp grey hairline at the top of what it is on. */
internal fun Modifier.topHairline(on: Boolean = true): Modifier = if (!on) this else drawBehind {
    val y = EinkSizes.Hairline.toPx() / 2
    drawLine(EinkColours.Tertiary, Offset(0f, y), Offset(size.width, y), EinkSizes.Hairline.toPx())
}

/** The 1 dp rule centred in the gap between two list columns. */
@Composable
internal fun ColumnRule(modifier: Modifier = Modifier) {
    Box(modifier.width(PaperListGap).fillMaxHeight(), contentAlignment = Alignment.Center) {
        Box(Modifier.width(EinkSizes.Hairline).fillMaxHeight().background(EinkColours.Tertiary))
    }
}

/**
 * The large e-ink series page under the app bar. The sheets and menus stay the page's own:
 * [onMain] is the main button, [onReadFrom] Change, [onChoose] Select, [onLong] a row's sheet.
 * [statusMenu] and [downloadMenu] draw the menus anchored to their buttons, and [notes] the
 * Send to e-reader rows and the where-did-you-stop note when there are any.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun EinkSeriesLarge(
    series: SeriesView,
    chapters: ReadChapters?,
    late: Boolean,
    newestFirst: Boolean,
    onOrder: (Boolean) -> Unit,
    chosen: Set<ChapterId>?,
    onChosen: (Set<ChapterId>) -> Unit,
    onCover: () -> Unit,
    onMain: (SeriesAction) -> Unit,
    onReadFrom: () -> Unit,
    onGroup: () -> Unit,
    onStatus: () -> Unit,
    onDownload: () -> Unit,
    onChoose: () -> Unit,
    onOpen: (ChapterRow) -> Unit,
    onLong: (ChapterRow) -> Unit,
    onRetry: () -> Unit,
    statusMenu: @Composable () -> Unit,
    downloadMenu: @Composable () -> Unit,
    notes: @Composable () -> Unit,
    modifier: Modifier = Modifier,
) {
    val look = LocalEinkLayout.current ?: app.kaeri.eink.EinkLook()
    EinkTheme(look) {
        Column(modifier.fillMaxSize().background(EinkColours.Paper).padding(horizontal = Side)) {
            SeriesTop(series, chapters, choosing = chosen != null, onCover, onMain, onReadFrom, onGroup, onStatus, onDownload, statusMenu, downloadMenu, notes)
            Spacer(Modifier.height(22.dp))
            Box(Modifier.fillMaxWidth().height(EinkSizes.Rule).background(EinkColours.Ink))
            SeriesChapters(series, chapters, late, newestFirst, onOrder, chosen, onChosen, onChoose, onOpen, onLong, onRetry, onReadFrom, Modifier.weight(1f))
        }
    }
}

/**
 * The large series page's top line: "‹ Library", or wherever Back goes, in words at the page's
 * margin as Browse and Settings have it, and the series' menu at the end.
 */
@Composable
internal fun EinkSeriesBack(back: String, onBack: () -> Unit, menu: @Composable () -> Unit) {
    val look = LocalEinkLayout.current ?: app.kaeri.eink.EinkLook()
    EinkTheme(look) {
        Row(
            Modifier
                .fillMaxWidth()
                .background(EinkColours.Paper)
                .windowInsetsPadding(WindowInsets.statusBars)
                .padding(start = Side, end = Side - 12.dp, top = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                "‹ $back", // i18n-skip
                style = EinkType.Label,
                modifier = Modifier.heightIn(min = 48.dp).einkClick(onClick = onBack).wrapContentHeight(Alignment.CenterVertically),
            )
            Spacer(Modifier.weight(1f))
            menu()
        }
    }
}

/** The cover beside the title, the medium line, the facts, the genres, the synopsis and the buttons. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SeriesTop(
    series: SeriesView,
    chapters: ReadChapters?,
    choosing: Boolean,
    onCover: () -> Unit,
    onMain: (SeriesAction) -> Unit,
    onReadFrom: () -> Unit,
    onGroup: () -> Unit,
    onStatus: () -> Unit,
    onDownload: () -> Unit,
    statusMenu: @Composable () -> Unit,
    downloadMenu: @Composable () -> Unit,
    notes: @Composable () -> Unit,
) {
    val context = LocalContext.current
    val canOpen = series.action.chapter != null
    Row(Modifier.fillMaxWidth().padding(top = 6.dp)) {
        ReadCover(
            series.cover,
            SeriesCover,
            Modifier
                .border(EinkSizes.Hairline, EinkColours.Ink)
                .then(
                    if (canOpen) {
                        Modifier
                            .einkClick(onClick = onCover)
                            .semantics { contentDescription = series.titles.display }
                    } else {
                        Modifier
                    },
                ),
            shape = RoundedCornerShape(0.dp),
            title = series.titles.display,
        )
        Spacer(Modifier.width(32.dp))
        Column(Modifier.weight(1f)) {
            Text(
                series.titles.display,
                style = EinkType.SeriesTitle.copy(lineHeight = 1.2.em),
                modifier = Modifier.semantics { heading() },
            )
            mediumLine(context, series, chapters).takeIf { it.isNotEmpty() }?.let {
                Text(it, style = EinkType.Secondary, modifier = Modifier.padding(top = 6.dp))
            }
            SeriesFacts(series, onReadFrom, onGroup, Modifier.padding(top = 14.dp))
            if (series.genres.isNotEmpty()) {
                FlowRow(
                    Modifier.padding(top = 14.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    series.genres.forEach { GenreTag(it) }
                }
            }
            // The whole synopsis, never cut short: the chapters take what is left.
            val synopsis = series.synopsis?.replace(Regex("\\s*\\n+\\s*"), " ")
            if (!synopsis.isNullOrBlank()) {
                Text(synopsis, style = EinkType.Body.copy(lineHeight = 1.45.em), modifier = Modifier.padding(top = 14.dp))
            }
            SeriesButtons(series, chapters, choosing, onMain, onReadFrom, onStatus, onDownload, statusMenu, downloadMenu, Modifier.padding(top = 18.dp))
            notes()
        }
    }
}

/** "Manga · 41 chapters · 6 volumes": the kind and the counts, the status being a fact of its own. */
private fun mediumLine(context: android.content.Context, series: SeriesView, chapters: ReadChapters?): String {
    val kind = series.facts.firstOrNull()?.text(context)
    // AniList's count when it has one, else what the source lists.
    val total = series.chaptersTotal?.toLong() ?: chapters?.volumes?.chapterRows()?.size?.takeIf { it > 0 }?.toLong()
    val count = total?.let { Msg.Reading(ReadingMsg.ReadingChapters(it)).text(context) }
    val volumes = series.volumesTotal?.let { Msg.Reading(ReadingMsg.ReadingVolumes(it.toLong())).text(context) }
    // A novel counts its volumes; its chapters are parts, which the list counts.
    val counts = if (series.medium == Medium.NOVEL) listOfNotNull(volumes) else listOfNotNull(count, volumes)
    return (listOfNotNull(kind) + counts).joinToString(" · ") // i18n-skip
}

/** The status in words, from the series' key. */
private fun statusWords(context: android.content.Context, key: String?): String? {
    val msg = when (key) {
        "releasing" -> ReadingMsg.ReadingStatusReleasing
        "finished" -> ReadingMsg.ReadingStatusFinished
        "hiatus" -> ReadingMsg.ReadingStatusHiatus
        "cancelled" -> ReadingMsg.ReadingStatusCancelled
        "not-yet" -> ReadingMsg.ReadingStatusNotYet
        else -> return null
    }
    return Msg.Reading(msg).text(context)
}

/**
 * The facts on one line, each a small label over its value: the person's score, the status, how
 * often chapters come out, the source in use with Change, and the group when the source has more
 * than one. A fact the engine does not know is left out.
 */
@Composable
private fun SeriesFacts(series: SeriesView, onReadFrom: () -> Unit, onGroup: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val forWork by store.bookSources.forWork.collectAsStateWithLifecycle()
    val editions = forWork[series.work]?.editions.orEmpty()
    val change = Strings.seriesChange(context)
    Row(modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(20.dp)) {
        series.score?.takeIf { it > 0f }?.let {
            Fact(Strings.seriesFactScore(context), Strings.seriesTrackerScore(context, scoreWords(it)), Modifier.weight(0.8f))
        }
        statusWords(context, series.status)?.let { Fact(Strings.seriesFactStatus(context), it, Modifier.weight(0.9f)) }
        series.rhythm?.let { Fact(Strings.seriesFactReleases(context), it.text(context), Modifier.weight(1.4f)) }
        series.edition?.let { edition ->
            Fact(
                Strings.seriesReadingFrom(context),
                SeriesWords.source(context, edition, (editions.size - 1).coerceAtLeast(0)),
                Modifier.weight(1.4f),
                action = change,
                onClick = onReadFrom,
            )
            if (edition.groups.size > 1) {
                Fact(Strings.seriesGroup(context), GroupWords.inUse(edition), Modifier.weight(1.2f), action = change, onClick = onGroup)
            }
        }
    }
}

/** A fact: its label at 15 over its value at 17 in SemiBold, one line, with an underlined action after it. */
@Composable
private fun Fact(label: String, value: String, modifier: Modifier = Modifier, action: String? = null, onClick: (() -> Unit)? = null) {
    Column(
        modifier
            .then(if (onClick != null) Modifier.einkClick(onClick = onClick) else Modifier)
            .semantics(mergeDescendants = true) {},
    ) {
        Text(label, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(value, style = EinkType.Label, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
            if (action != null) {
                Text(
                    action,
                    style = EinkType.Button.copy(textDecoration = TextDecoration.Underline),
                    maxLines = 1,
                    modifier = Modifier.padding(start = 6.dp),
                )
            }
        }
    }
}

/** A genre as a 34 dp tag with a 1 dp grey border. */
@Composable
private fun GenreTag(text: String) {
    Box(
        Modifier
            .heightIn(min = 34.dp)
            .border(EinkSizes.Hairline, EinkColours.Tertiary, RoundedCornerShape(8.dp))
            .padding(horizontal = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = EinkType.Secondary.copy(color = EinkColours.Ink))
    }
}

/**
 * The solid main action with the outlined list and Download buttons beside it, as the colour
 * page decides them: with no source, or a source that failed, Read from another source is the
 * main action and the other two are left out.
 */
@Composable
private fun SeriesButtons(
    series: SeriesView,
    chapters: ReadChapters?,
    choosing: Boolean,
    onMain: (SeriesAction) -> Unit,
    onReadFrom: () -> Unit,
    onStatus: () -> Unit,
    onDownload: () -> Unit,
    statusMenu: @Composable () -> Unit,
    downloadMenu: @Composable () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val words = SeriesWords.action(context, series.action)
    val failed = chapters?.failure != null
    val asking = series.action is SeriesAction.NoSource && chapters == null
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        when {
            // Still asking the sources: the button holds its place, quiet.
            asking -> EinkSolidButton("", {}, enabled = false, modifier = Modifier.width(220.dp).clearAndSetSemantics {})
            words == null || failed -> EinkSolidButton(Strings.seriesReadFromAnother(context), onReadFrom)
            else -> EinkSolidButton(words, { onMain(series.action) }, enabled = !choosing)
        }
        if (!failed && (words != null || asking)) {
            Box {
                EinkOutlinedButton(series.list?.let { listWords(context, it) } ?: Strings.seriesAddToList(context), { if (!choosing) onStatus() })
                statusMenu()
            }
            Box {
                EinkOutlinedButton(Strings.seriesDownload(context), { if (!choosing && chapters?.volumes?.hasDownloads() == true) onDownload() })
                downloadMenu()
            }
        }
    }
}

/**
 * Under the rule: Chapters with the count, the order and Select, then the chapters in columns a
 * page at a time. A page holds as many rows as fit above the pager.
 */
@Composable
private fun SeriesChapters(
    series: SeriesView,
    chapters: ReadChapters?,
    late: Boolean,
    newestFirst: Boolean,
    onOrder: (Boolean) -> Unit,
    chosen: Set<ChapterId>?,
    onChosen: (Set<ChapterId>) -> Unit,
    onChoose: () -> Unit,
    onOpen: (ChapterRow) -> Unit,
    onLong: (ChapterRow) -> Unit,
    onRetry: () -> Unit,
    onReadFrom: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val failure = chapters?.failure
    // With the source failed, what is downloaded still opens: only those chapters.
    val all = when {
        chapters == null -> emptyList()
        failure != null -> chapters.volumes.downloaded()
        else -> chapters.volumes.chapterRows()
    }
    val rows = if (newestFirst) all.reversed() else all
    var page by rememberSaveable(series.work) { mutableIntStateOf(0) }
    LaunchedEffect(newestFirst) { page = 0 }
    Column(modifier.fillMaxWidth()) {
        ChaptersHead(series, chapters, newestFirst, onOrder, choosing = chosen != null, canChoose = chapters != null && failure == null && all.isNotEmpty(), onChoose)
        when {
            chapters == null -> if (late) {
                PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 8.dp)) }
            }
            failure is ReadingFailure.NoSource -> ChaptersNote(
                Strings.seriesNoSourceTitle(context),
                Strings.seriesNoSourceLine(context),
                Strings.seriesNoSourceSearch(context),
                onReadFrom,
            )
            failure != null -> {
                val line = listOfNotNull(
                    failureWords(context, failure),
                    if (all.isNotEmpty()) Strings.readSeriesDownloadedOpen(context, all.size) else null,
                ).joinToString(" ") // i18n-skip
                PaperFailure(Strings.seriesChaptersFailed(context, series.edition?.sourceName.orEmpty()), line, Strings.actionTryAgain(context), onRetry)
            }
            all.isEmpty() -> ChaptersNote(
                Strings.seriesNoChaptersTitle(context),
                Strings.seriesNoChaptersLine(context),
                Strings.seriesReadFromAnother(context),
                onReadFrom,
            )
        }
        if (rows.isNotEmpty()) {
            ChapterPages(series, rows, page, { page = it }, chosen, onChosen, onOpen, onLong, Modifier.weight(1f))
        }
    }
}

/** "Chapters", how many, the order as an underlined toggle and Select. */
@Composable
private fun ChaptersHead(
    series: SeriesView,
    chapters: ReadChapters?,
    newestFirst: Boolean,
    onOrder: (Boolean) -> Unit,
    choosing: Boolean,
    canChoose: Boolean,
    onChoose: () -> Unit,
) {
    val context = LocalContext.current
    val count = chapters?.volumes?.chapterRows()?.size ?: 0
    Row(Modifier.fillMaxWidth().height(ChapterHead), verticalAlignment = Alignment.CenterVertically) {
        Text(Strings.seriesChapters(context), style = EinkType.RowTitle, modifier = Modifier.semantics { heading() })
        if (count > 0) {
            Text(Msg.Reading(ReadingMsg.ReadingChapters(count.toLong())).text(context), style = EinkType.Secondary, modifier = Modifier.padding(start = 12.dp))
        }
        Spacer(Modifier.weight(1f))
        if (chapters != null && count > 1) {
            EinkTextButton(
                if (newestFirst) Strings.seriesNewestFirst(context) else Strings.seriesOldestFirst(context),
                { onOrder(!newestFirst) },
                underline = true,
            )
        }
        if (canChoose && !choosing) {
            EinkTextButton(Strings.seriesSelect(context), onChoose, underline = true, modifier = Modifier.padding(start = 4.dp))
        }
    }
}

/** Why there are no chapters to list, and the way on. */
@Composable
private fun ChaptersNote(title: String, line: String, action: String, onAction: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f).semantics(mergeDescendants = true) {}) {
            Text(title, style = EinkType.Body)
            Text(line, style = EinkType.Secondary)
        }
        EinkOutlinedButton(action, onAction, Modifier.padding(start = 12.dp), height = 48.dp)
    }
}

/** A chapter row's height at the text size in use: one 17 sp line and 22 dp, 48 at the least. */
@Composable
private fun chapterRowHeight(): Dp {
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val line = with(density) { measurer.measure("Ag", EinkType.Body).size.height.toDp() } // i18n-skip
    return maxOf(ChapterRowLeast, line + ChapterRowPad)
}

/**
 * The rows a page at a time in as many columns as the width holds, down the first column and
 * then the next, with the pager under them. Only this block redraws when a page turns.
 */
@Composable
private fun ChapterPages(
    series: SeriesView,
    rows: List<ChapterRow>,
    page: Int,
    onPage: (Int) -> Unit,
    chosen: Set<ChapterId>?,
    onChosen: (Set<ChapterId>) -> Unit,
    onOpen: (ChapterRow) -> Unit,
    onLong: (ChapterRow) -> Unit,
    modifier: Modifier = Modifier,
) {
    val rowHeight = chapterRowHeight()
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val columns = paperListColumns(maxWidth)
        // The pager has its row and 8 dp above it, and 12 dp under it above the navigation.
        val perColumn = ((maxHeight - EinkSizes.PagerRow - 8.dp - PagerFoot) / rowHeight).toInt().coerceAtLeast(1)
        val pages = rows.chunked(perColumn * columns)
        val shown = page.coerceIn(0, pages.size - 1)
        Column(Modifier.fillMaxSize()) {
            Row(Modifier.fillMaxWidth().height(rowHeight * perColumn)) {
                val onPage = pages[shown].chunked(perColumn)
                for (column in 0 until columns) {
                    // No rule beside an empty column: the gap stays, so the columns keep their width.
                    if (column > 0) {
                        if (onPage.getOrNull(column).isNullOrEmpty()) Spacer(Modifier.width(PaperListGap))
                        else ColumnRule(Modifier.height(rowHeight * (onPage.getOrNull(0)?.size ?: 0)))
                    }
                    Column(Modifier.weight(1f)) {
                        onPage.getOrNull(column).orEmpty().forEachIndexed { index, row ->
                            ChapterLine(series, row, index == 0, rowHeight, chosen, onChosen, onOpen, onLong)
                        }
                    }
                }
            }
            Spacer(Modifier.weight(1f))
            if (pages.size > 1) EinkPager(shown, pages.size, onPage)
            Spacer(Modifier.height(PagerFoot))
        }
    }
}

/** Whether a row's first words are a light novel's place: a percent, or Opened. */
private fun novelPlace(msg: Msg): Boolean = (msg as? Msg.Reading)?.v1.let { it is ReadingMsg.ReadingRowPercent || it is ReadingMsg.ReadingRowOpened }

/**
 * What a row says at its end: Read, the page, the download's state, New, or nothing at all. A
 * light novel's part says how far in as the engine words it, a percent or Opened, since its kept
 * place is no page the reader counts.
 */
private fun chapterState(context: android.content.Context, row: ChapterRow): String? = when (val state = row.state) {
    is ChapterReadState.Read -> context.getString(R.string.chapter_state_read)
    is ChapterReadState.InProgress -> row.line.firstOrNull()?.takeIf { novelPlace(it) }?.text(context) ?: state.of?.let { Strings.chapterStatePageOf(context, state.page.toInt(), it.toInt()) }
        ?: Strings.chapterStatePage(context, state.page.toInt())
    is ChapterReadState.Unread -> when (val download = row.download) {
        ChapterDownloadState.Done -> Strings.seriesRowOnDevice(context)
        ChapterDownloadState.Queued -> context.getString(R.string.chapter_state_queued)
        ChapterDownloadState.WaitsForWifi -> context.getString(R.string.chapter_state_wifi)
        is ChapterDownloadState.Downloading -> Strings.chapterStateDownloading(context, download.done.toInt(), download.total.toInt())
        is ChapterDownloadState.Failed -> Strings.chapterStateFailed(context, failureWords(context, download.failure))
        null -> if (row.newChapter) context.getString(R.string.chapter_state_new) else null
    }
}

/**
 * One chapter on one line: its number in a 64 dp column, its title, its state at the end. Read
 * rows are grey, the one in progress bold with its page; an unread chapter not on the device says
 * nothing. While choosing, a tap ticks it and a box takes the state's place.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ChapterLine(
    series: SeriesView,
    row: ChapterRow,
    first: Boolean,
    height: Dp,
    chosen: Set<ChapterId>?,
    onChosen: (Set<ChapterId>) -> Unit,
    onOpen: (ChapterRow) -> Unit,
    onLong: (ChapterRow) -> Unit,
) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val read = row.state is ChapterReadState.Read
    val current = row.state is ChapterReadState.InProgress
    val ink = if (read) EinkColours.Tertiary else EinkColours.Ink
    val weight = if (current) FontWeight.Bold else FontWeight.Medium
    val name = SeriesWords.name(context, row)
    val number = row.number?.takeIf { it.isNotBlank() }
    // A chapter with no title of its own leaves the cell empty under its number, rather than say
    // the number twice; a row without a number names the chapter there instead.
    val title = SeriesWords.title(row, name) ?: name.takeIf { number == null }
    val state = chapterState(context, row)
    val ticked = chosen?.contains(row.id)
    val actions = listOf(
        CustomAccessibilityAction(if (read) Strings.seriesMarkUnread(context) else Strings.seriesMarkRead(context)) {
            store.reading.markRead(series.work, listOf(row.id), !read)
            true
        },
        CustomAccessibilityAction(Strings.seriesMarkReadUpTo(context)) {
            store.reading.markReadUpTo(series.work, row.id)
            true
        },
        CustomAccessibilityAction(stringResource(R.string.chapter_retry)) {
            store.reading.download(series.work, DownloadPick.Chapters(listOf(row.id)))
            true
        }.takeIf { row.download is ChapterDownloadState.Failed },
    ).filterNotNull()
    Row(
        Modifier
            .fillMaxWidth()
            .height(height)
            .topHairline(!first)
            .combinedClickable(
                interactionSource = remember { MutableInteractionSource() },
                // The number starts at the row's edge: the mark goes round the row, not over it.
                indication = einkFocus(inset = EinkSizes.FocusOutside),
                role = Role.Button,
                onLongClickLabel = stringResource(R.string.action_more),
                onLongClick = if (chosen == null) ({ onLong(row) }) else null,
                onClick = {
                    if (chosen != null) onChosen(if (row.id in chosen) chosen - row.id else chosen + row.id) else onOpen(row)
                },
            )
            .semantics(mergeDescendants = true) {
                customActions = actions
                if (ticked != null) selected = ticked
            },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val text = EinkType.Body.copy(color = ink, fontWeight = weight)
        Text(
            number.orEmpty(),
            style = text.copy(fontFeatureSettings = "tnum"),
            maxLines = 1,
            // Read out as the chapter's name when the number stands alone, and before a title as
            // "Chapter 32", never as a bare number.
            modifier = Modifier.width(ChapterNumber).then(
                if (title == null) {
                    Modifier.clearAndSetSemantics { this.text = AnnotatedString(SeriesWords.spokenName(context, row)) }
                } else if (number != null) {
                    Modifier.clearAndSetSemantics { this.text = AnnotatedString(Strings.seriesChapterNumber(context, number)) }
                } else {
                    Modifier
                },
            ),
        )
        Text(title.orEmpty(), style = text, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f).padding(start = 12.dp))
        when {
            ticked != null -> ChooseBox(ticked, Modifier.padding(start = 12.dp))
            state != null -> Row(Modifier.padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                if (row.newChapter && !read && row.download == null) {
                    Box(Modifier.size(EinkSizes.NewDot).clip(CircleShape).background(EinkColours.Ink))
                    Spacer(Modifier.width(7.dp))
                }
                val stateStyle: TextStyle = when {
                    current -> EinkType.Secondary.copy(color = EinkColours.Ink, fontWeight = FontWeight.Bold)
                    read -> EinkType.Tertiary
                    else -> EinkType.Secondary
                }
                Text(state, style = stateStyle.copy(fontFeatureSettings = "tnum"), maxLines = 1)
            }
        }
    }
}

/** While choosing: a 24 dp box with a 2 dp border, filled and ticked when chosen. */
@Composable
private fun ChooseBox(ticked: Boolean, modifier: Modifier = Modifier) {
    Box(
        modifier.size(24.dp).border(EinkSizes.Rule, EinkColours.Ink, RoundedCornerShape(3.dp)).background(if (ticked) EinkColours.Ink else EinkColours.Paper),
        contentAlignment = Alignment.Center,
    ) {
        if (ticked) Icon(Icons.Check, contentDescription = null, tint = EinkColours.Paper, modifier = Modifier.size(18.dp))
    }
}

