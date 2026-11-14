// The e-ink shell's own pages, as the e-ink board draws them: Home on one screen (the series in
// progress with its one solid button, Continue, New chapters on an e-reader or This week on a
// phone, then the Browse row), and the e-reader's Downloads and More tabs. Paper and ink, no
// motion, no spinners: what changes redraws in place.

package app.kaeri.ui.reading

import android.annotation.SuppressLint
import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
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
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.Placeable
import androidx.compose.ui.layout.SubcomposeLayout
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.layoutId
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkLook
import app.kaeri.eink.EinkOutlinedButton
import app.kaeri.eink.EinkPager
import app.kaeri.eink.EinkSizes
import app.kaeri.eink.EinkSolidButton
import app.kaeri.eink.EinkTextButton
import app.kaeri.eink.EinkTheme
import app.kaeri.eink.EinkType
import app.kaeri.eink.PaperWaiting
import app.kaeri.eink.bottomRule
import app.kaeri.eink.chapterNumber
import app.kaeri.eink.einkClick
import app.kaeri.eink.hairline
import app.kaeri.eink.topRule
import app.kaeri.engine.model.BookDownloadRow
import app.kaeri.engine.model.BookDownloadSeries
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.DownloadPick
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.ReadSpotlight
import app.kaeri.engine.model.ReadWeekDay
import app.kaeri.engine.model.ReadWeekEntry
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.SettingKey
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.Wording
import app.kaeri.store.loadSync
import app.kaeri.store.text
import app.kaeri.ui.AccountWords
import app.kaeri.ui.AppActions
import app.kaeri.ui.Destination
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.SettingsPane
import app.kaeri.ui.appVersion
import app.kaeri.ui.devicesSummary
import app.kaeri.ui.einkReaderShape
import app.kaeri.ui.kaeriIsFor
import java.time.Instant
import java.time.format.TextStyle

/** Between the boards' 6 inch panel, 772 dp tall, and their 7 inch one, 896 dp. */
internal const val TALL_PANEL_DP = 834f

/**
 * A 7 inch panel and up, where the covers grow. Measured on the whole window, system bars
 * included, which every Android version reports alike; the configuration's height leaves the bars
 * out before Android 15, which would put a 7 inch e-reader on Android 11 to 14 under the line.
 */
@Composable
private fun tallPanel(): Boolean = tallPanel(LocalWindowInfo.current.containerSize.height, LocalDensity.current.density)

/** Whether a window [heightPx] tall at [density] is a 7 inch panel or larger. */
internal fun tallPanel(heightPx: Int, density: Float): Boolean = density > 0f && heightPx / density >= TALL_PANEL_DP

/** The hero's cover, as the board sizes it for the panel. */
@Composable
private fun heroCover(reader: Boolean): Dp = when {
    !reader -> 96.dp
    tallPanel() -> 124.dp
    else -> 104.dp
}

/** A Continue cover: four across on an e-reader, three on a phone, at the board's size. */
@Composable
private fun continueCover(reader: Boolean): Dp = when {
    !reader -> 100.dp
    tallPanel() -> 104.dp
    else -> 90.dp
}

// Home

/**
 * Reading's Home on e-ink, one screen with no paging: the series in progress naming its next step
 * on the one solid button, Continue, New chapters (This week on a phone) and the Browse row under
 * a 2 dp rule. A first run has the demo in a bordered box and the Browse list in place of shelves.
 */
@Composable
internal fun EinkHomeScreen(actions: AppActions) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val home by store.reading.home.collectAsStateWithLifecycle()
    val failures by store.reading.failures.collectAsStateWithLifecycle()
    val demo by store.demo.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.reading.loadHome() }
    DemoLaunchHost(actions)
    val reader = einkReaderShape()
    // Large text keeps the Browse list to what fits no longer: it scrolls rather than clip.
    val large = LocalDensity.current.fontScale > 1.3f
    val title = if (kaeriIsFor() == KaeriIsFor.READ) Strings.homeTitle(context) else Strings.navRead(context)
    // Offline, the line under the title says what time the Home shown is from, as the colour Home's.
    val since = home?.offlineSince?.takeIf { home?.offline == true }
    PaperPage(back = null, title = title, line = since?.let { Strings.homeOffline(context, clockWords(it)) }) {
        val view = home
        val failure = failures[ReadingAbout.HOME]
        when {
            view == null && failure != null -> PaperFailure(
                Strings.homeFailed(context),
                failureWords(context, failure),
                Strings.homeTryAgain(context),
            ) { store.reading.loadHome(refresh = true) }
            // Loading with no motion: after 600 ms one word, then the page in one refresh.
            view == null -> PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp)) }
            else -> {
                val tile = view.spotlight
                val spot = view.spot
                // The series in the hero isn't repeated in Continue, as on the boards.
                val hero = tile?.work?.takeIf { spot != null && !demo.onlyBooks }
                val continuing = view.continueReading.filter { it.work != hero }
                val came = cameOut(view.week)
                val rows = view.rows.filter { it.tiles.isNotEmpty() }
                val own = continuing.isNotEmpty() || came.isNotEmpty() || (tile != null && !demo.onlyBooks)
                // A 10 inch panel has its own Home, as the large panel board draws it, widened when
                // held on its side; the first run, the demo and large text keep the smaller panels' Home.
                if (reader && own && !large && !demo.onlyBooks && largePanel()) {
                    LargeHome(tile?.takeIf { spot != null }, spot, continuing, came, rows, view.rows.any { it.pending }, actions)
                    return@PaperPage
                }
                // Home is one screen; what can't fit (large text, a small panel) scrolls rather
                // than clip. The first run's Browse list instead keeps the rows that fit.
                val scroll = large || own
                val body: @Composable (Modifier) -> Unit = { lines ->
                    if (demo.onlyBooks) {
                        PaperDemo(actions, reader)
                        PaperDemoLine()
                    } else if (tile != null && spot != null) {
                        PaperHero(tile, spot, actions, reader)
                    }
                    if (continuing.isNotEmpty()) PaperContinue(continuing.take(if (reader) 4 else 3), reader)
                    // An e-reader's second New chapters row gives way when the page would not fit.
                    if (came.isNotEmpty()) PaperChapters(came.take(if (reader) 2 else 1), reader)
                    if (rows.isNotEmpty()) {
                        PaperBrowse(rows, inline = !own, fit = !scroll, lines = lines)
                    } else if (view.rows.any { it.pending }) {
                        PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Secondary, modifier = Modifier.padding(top = 16.dp)) }
                    }
                }
                if (scroll) {
                    BoxWithConstraints(Modifier.fillMaxSize()) {
                        val room = constraints.maxHeight
                        ShedToFit(room, Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) { body(Modifier) }
                    }
                } else {
                    Column(Modifier.fillMaxSize()) { body(Modifier.weight(1f, fill = false)) }
                }
            }
        }
    }
}

// Home on a 10 inch panel

/** A panel this many dp across and wider is the 10.3 inch board's, as [Side] measures it. */
private const val WIDE_PANEL_DP = 900

/** A 10 inch panel: 900 dp across and wider, where lists take two columns and more. */
@Composable
internal fun largePanel(): Boolean = LocalConfiguration.current.screenWidthDp >= WIDE_PANEL_DP

/** The least room New chapters takes on a 10 inch Home: its heading and one 64 dp row, with their gaps. */
private val NewChaptersLeast = 140.dp

/** New chapters beside the hero on a 10 inch panel held on its side: one list column. */
private val NewChaptersBeside = 480.dp

/** Whether the panel is held on its side, wider than it is tall. */
@Composable
private fun landscapePanel(): Boolean = LocalConfiguration.current.let { it.screenWidthDp > it.screenHeightDp }

/** Continue's columns on a 10 inch panel held on its side, filling the wider page. */
private const val LANDSCAPE_CONTINUE_COLUMNS = 5

/** Between list columns on a large panel; each is at least [ListColumnLeast] wide. */
private val ListGap = 40.dp
private val ListColumnLeast = 420.dp

/** How many list columns a width holds: as many of at least 420 dp as fit, one at least. */
private fun listColumns(width: Dp): Int = ((width + ListGap) / (ListColumnLeast + ListGap)).toInt().coerceAtLeast(1)

/** The hero's cover on a 10 inch Home: the grid's 184 × 276. */
private val LargeHeroCover = 184.dp

/** A Continue cover on a 10 inch Home, 140 × 210 in the grid's columns. */
private val LargeContinueCover = 140.dp

/** The hero's synopsis: the body size in grey, at the board's line height of 1.45. */
private val HeroSynopsis = EinkType.Body.copy(color = EinkColours.Secondary, lineHeight = EinkType.Body.fontSize * 1.45f)

/** A series' title in the 10 inch lists: 18 sp in the titles' face. */
private val LargeRowTitle = EinkType.RowTitle

/** A row's own words in the 10 inch lists, such as a setting's name: the label at 18 sp. */
private val LargeRowLabel = EinkType.Label.copy(fontSize = 18.sp)

/**
 * Home on a 10 inch panel, one screen and never paged: the series in progress at the grid's cover
 * size with its synopsis and the one solid button, Continue on the grid's columns, New chapters in
 * two columns filling the room that is left, and the Browse rows as chips closing the page.
 */
@Composable
private fun LargeHome(
    tile: WorkTile?,
    spot: ReadSpotlight?,
    continuing: List<WorkTile>,
    came: List<CameOut>,
    rows: List<app.kaeri.engine.model.ReadHomeRow>,
    pending: Boolean,
    actions: AppActions,
) {
    val context = LocalContext.current
    val hero = tile != null && spot != null
    // On its side the panel has room beside the hero: New chapters goes there, as many rows as
    // the hero's height holds, so the page below keeps Continue and Browse.
    val beside = hero && came.isNotEmpty() && landscapePanel()
    Column(Modifier.fillMaxSize()) {
        if (tile != null && spot != null) {
            if (beside) {
                Row(Modifier.fillMaxWidth()) {
                    Box(Modifier.weight(1f)) { LargeHero(tile, spot, actions) }
                    LargeNewChapters(
                        came,
                        Modifier.padding(start = ListGap).width(NewChaptersBeside).height(LargeHeroCover * 1.5f),
                        beside = true,
                    )
                }
            } else {
                LargeHero(tile, spot, actions)
            }
        }
        if (continuing.isNotEmpty()) LargeContinue(continuing)
        // New chapters takes the room left, and gives way when that can't hold its heading and a
        // row rather than show a clipped heading.
        // Without New chapters Browse follows Continue, so no empty room opens between them.
        if (came.isNotEmpty() && !beside) {
            BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
                if (maxHeight >= NewChaptersLeast) LargeNewChapters(came, Modifier.fillMaxSize())
            }
        } else if (beside) {
            Spacer(Modifier.weight(1f))
        } else {
            Spacer(Modifier.height(18.dp))
        }
        when {
            rows.isNotEmpty() -> LargeBrowseChips(rows)
            pending -> PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Secondary, modifier = Modifier.padding(bottom = 24.dp)) }
        }
    }
}

/**
 * The series in progress on a 10 inch Home: the cover at the grid's size beside what it is, where
 * the person is and its synopsis, then its facts over Continue and the Series page link.
 */
@Composable
private fun LargeHero(tile: WorkTile, spot: ReadSpotlight, actions: AppActions) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val words = actionWords(context, spot.action)
    val chapter = spot.action?.chapter
    val series = { navigator.openSeries(tile.work, "eink-home") }
    // Three lines of synopsis at the panel's 0.85, two from 1.0, so the words keep to the cover's height.
    val synopsisLines = if (LocalDensity.current.fontScale > 0.9f) 2 else 3
    Row(Modifier.padding(top = 14.dp)) {
        PaperCover(tile.cover, tile.title, LargeHeroCover, Modifier.einkClick(onClick = series))
        Column(
            Modifier.weight(1f).padding(start = 32.dp).heightIn(min = LargeHeroCover * 1.5f),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(Modifier.semantics(mergeDescendants = true) {}) {
                Text(spot.label.text(context), style = EinkType.Secondary)
                Text(tile.title, style = EinkType.SeriesTitle, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 2.dp, bottom = 6.dp))
                spot.line?.let { Text(it.text(context), style = EinkType.Label, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                spot.synopsis?.takeIf { it.isNotBlank() }?.let { synopsis ->
                    Text(
                        synopsis.replace(Regex("\\s*\\n+\\s*"), " "),
                        style = HeroSynopsis,
                        maxLines = synopsisLines,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.padding(top = 10.dp),
                    )
                }
            }
            Column {
                // What the series is, where the board names the next chapter: the engine sends no
                // next chapter's length or place for Home.
                val facts = spot.facts.joinToString(" · ") { it.text(context) } // i18n-skip
                if (facts.isNotEmpty()) Text(facts, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 12.dp, bottom = 12.dp))
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (words != null) {
                        EinkSolidButton(words, {
                            if (chapter != null) actions.openReader(tile.work, chapter, tile.cover, readerCoverKey("spotlight", tile.work)) else series()
                        })
                        EinkTextButton(Strings.einkHomeSeriesPage(context), series, underline = true)
                    } else {
                        EinkOutlinedButton(Strings.homeDetails(context), series)
                    }
                }
            }
        }
    }
}

/**
 * Continue on a 10 inch Home: 140 dp covers on the grid's columns, from margin to margin, with
 * their title and place; five columns when the panel is held on its side.
 */
@Composable
private fun LargeContinue(tiles: List<WorkTile>) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    Spacer(Modifier.height(18.dp))
    PaperHeading(Strings.homeReadContinue(context), Strings.einkHomeAll(context), flush = true) {
        ReadPlaces.showList(LibrarySegmentRead.READING)
        navigator.go(Destination.Library)
    }
    BoxWithConstraints(Modifier.fillMaxWidth().padding(top = 4.dp)) {
        // The grid rule's columns upright: as many of at least 200 dp as the width holds.
        val columns = if (landscapePanel()) {
            LANDSCAPE_CONTINUE_COLUMNS
        } else {
            paperGrid(maxWidth, 1000.dp, 0.dp, 0.dp).columns
        }
        // The first cover on the start margin and the last on the end one, the gaps shared out
        // between; with fewer series the tiles keep their places from the start.
        val gap = (maxWidth - LargeContinueCover * columns) / (columns - 1).coerceAtLeast(1)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(gap)) {
            for (tile in tiles.take(columns)) {
                Column(
                    Modifier
                        .width(LargeContinueCover)
                        .einkClick { navigator.openSeries(tile.work, "eink-continue") }
                        .semantics(mergeDescendants = true) {},
                ) {
                    PaperCover(tile.cover, tile.title, LargeContinueCover, Modifier)
                    Text(tile.title, style = EinkType.Label, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
                    val line = tile.line(context)
                    if (line.isNotEmpty()) Text(line, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}

/**
 * New chapters on a 10 inch Home: rows down the first column, then the second, as many as the room
 * holds; [beside] the hero on a panel held on its side, one column as tall as the hero.
 */
@Composable
private fun LargeNewChapters(came: List<CameOut>, modifier: Modifier, beside: Boolean = false) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    Column(if (beside) modifier else modifier.fillMaxWidth()) {
        // Beside the hero the heading shares the baseline of the hero's "Continue reading".
        if (!beside) Spacer(Modifier.height(18.dp))
        PaperHeading(Strings.einkHomeNewChapters(context), Strings.einkHomeAllNewChapters(context), flush = true, top = if (beside) 3.dp else 10.dp) {
            navigator.go(Destination.Updates)
        }
        PagedColumns(came, page = 0, onPage = {}, modifier = Modifier.weight(1f).padding(top = 4.dp, bottom = 12.dp), paged = false) { (day, entry), first ->
            val tile = entry.tile
            val line = cameWords(context, day, entry)
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 64.dp)
                    .einkClick { navigator.openSeries(tile.work, "eink-week") }
                    .then(if (first) Modifier else Modifier.topHairline())
                    .padding(vertical = 9.dp)
                    .semantics(mergeDescendants = true) { contentDescription = "${tile.title}. $line" }, // i18n-skip
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(tile.title, style = LargeRowTitle, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 2.dp)) {
                        Box(Modifier.size(EinkSizes.NewDot).clip(CircleShape).background(EinkColours.Ink))
                        Text(line, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = 7.dp))
                    }
                }
                Icon(Icons.Chevron, contentDescription = null, tint = EinkColours.Ink, modifier = Modifier.padding(start = 12.dp).size(22.dp))
            }
        }
    }
}

/** Browse closing a 10 inch Home: each row as a chip opening its grid, and All rows opening the list. */
@Composable
private fun LargeBrowseChips(rows: List<app.kaeri.engine.model.ReadHomeRow>) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    Column(Modifier.fillMaxWidth().padding(bottom = 24.dp)) {
        PaperHeading(Strings.readHomeBrowse(context), Strings.einkHomeAllRows(context), flush = true) { navigator.openReadPage(READ_PAGE_BROWSE) }
        PaperChips(Modifier.padding(top = 4.dp)) {
            for (row in rows) PaperChip(row.title.text(context)) { navigator.openReadPage(READ_PAGE_ROW + row.id) }
        }
    }
}

// Browse on a 10 inch panel

/** A Browse tile's covers: its row's first four at 90 × 135. */
private val BrowseTileCover = 90.dp
private const val BROWSE_TILE_COVERS = 4

/**
 * Browse on a 10 inch panel: each row as a tile with its first four covers, two to a line on the
 * list columns, paged when they don't fit, then AniList's genres as chips at the foot, each opening
 * Search browsing that genre.
 */
@Composable
internal fun LargeBrowse(rows: List<app.kaeri.engine.model.ReadHomeRow>, page: Int, onPage: (Int) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val genres = store.reading.filters.genres
    Column(Modifier.fillMaxSize().padding(top = 10.dp)) {
        PagedColumns(rows, page, onPage, Modifier.weight(1f).padding(bottom = 12.dp), across = true) { row, _ ->
            BrowseTile(row) { navigator.openReadPage(READ_PAGE_ROW + row.id) }
        }
        if (genres.isNotEmpty()) {
            Column(Modifier.fillMaxWidth().padding(bottom = 22.dp)) {
                Row(Modifier.fillMaxWidth().height(HeadingHeight), verticalAlignment = Alignment.CenterVertically) {
                    Text(Strings.readFilterGenres(context), style = EinkType.RowTitle, modifier = Modifier.weight(1f).semantics { heading() })
                    Text(Strings.einkBrowseGenresFrom(context), style = EinkType.Secondary)
                }
                PaperChips(Modifier.padding(top = 4.dp)) {
                    for (genre in genres) {
                        PaperChip(genre) {
                            // The Search tab itself, browsing the genre with no words, as a row's Search link opens it.
                            ReadPlaces.search(store.reading, app.kaeri.store.ReadingModel.EMPTY_QUERY.copy(genres = listOf(genre)))
                            navigator.goToStart(Destination.Search)
                        }
                    }
                }
            }
        }
    }
}

/** A row as a Browse tile under a grey hairline: its title and a chevron, its first covers, and what it holds. */
@Composable
private fun BrowseTile(row: app.kaeri.engine.model.ReadHomeRow, onClick: () -> Unit) {
    val context = LocalContext.current
    val title = row.title.text(context)
    val line = browseRowLine(context, row)
    Column(
        Modifier
            .fillMaxWidth()
            .einkClick(onClick = onClick)
            .topHairline()
            .padding(top = 12.dp, bottom = 16.dp)
            .semantics(mergeDescendants = true) { contentDescription = "$title. $line" }, // i18n-skip
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(title, style = EinkType.RowTitle, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            Icon(Icons.Chevron, contentDescription = null, tint = EinkColours.Ink, modifier = Modifier.padding(start = 12.dp).size(22.dp))
        }
        // The covers spread across the column, the first at its start and the fourth at its end.
        Row(Modifier.fillMaxWidth().padding(top = 12.dp), horizontalArrangement = Arrangement.SpaceBetween) {
            val covers = row.tiles.take(BROWSE_TILE_COVERS)
            for (tile in covers) PaperCover(tile.cover, tile.title, BrowseTileCover, Modifier)
            repeat(BROWSE_TILE_COVERS - covers.size) { Spacer(Modifier.width(BrowseTileCover)) }
        }
        if (line.isNotEmpty()) Text(line, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 10.dp))
    }
}

/** Chips on paper, wrapping onto as many lines as they take, 10 dp apart. */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
internal fun PaperChips(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    androidx.compose.foundation.layout.FlowRow(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) { content() }
}

/** A chip on paper: 48 dp, a 2 dp ink border with radius 10, the words at 15 sp. */
@Composable
internal fun PaperChip(text: String, onClick: () -> Unit) {
    val shape = RoundedCornerShape(EinkSizes.SegmentRadius)
    Box(
        Modifier
            .heightIn(min = EinkSizes.SegmentHeight)
            .clip(shape)
            .border(EinkSizes.Rule, EinkColours.Ink, shape)
            .einkClick(shape = shape, inset = EinkSizes.FocusInsideBorder, onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 6.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, style = EinkType.Secondary.copy(color = EinkColours.Ink))
    }
}

/** Where a laid out item of [PagedColumns] goes: its index, its column, how far down, and whether it heads its column. */
private data class Placed(val index: Int, val column: Int, val y: Int, val first: Boolean)

/**
 * Items in list columns of at least 420 dp side by side, 40 dp apart, a page at a time with the
 * pager under them. Rows run down the first column, then the next, as a printed index reads; with
 * [across], items fill each line across first, as tiles do. Each item is measured as it is, so
 * large text gets more pages rather than clipped rows. With [paged] off, only what fits the
 * first page shows and there is no pager. [rules] draws a 1 dp grey rule between the columns.
 * [item] is told whether it heads its column, which draws no rule above it.
 */
@Composable
internal fun <T> PagedColumns(
    items: List<T>,
    page: Int,
    onPage: (Int) -> Unit,
    modifier: Modifier = Modifier,
    across: Boolean = false,
    paged: Boolean = true,
    rules: Boolean = false,
    item: @Composable (T, Boolean) -> Unit,
) {
    SubcomposeLayout(modifier.fillMaxSize()) { constraints ->
        val width = constraints.maxWidth
        val gap = ListGap.roundToPx()
        val columns = listColumns(width.toDp())
        val columnWidth = (width - gap * (columns - 1)) / columns
        val loose = Constraints(maxWidth = columnWidth)
        val heights = items.mapIndexed { index, it -> subcompose("measure-$index") { item(it, false) }.sumOf { m -> m.measure(loose).height } }
        fun pack(room: Int): List<List<Placed>> {
            val pages = mutableListOf(mutableListOf<Placed>())
            var y = 0
            if (across) {
                items.indices.chunked(columns).forEach { line ->
                    val height = line.maxOf { heights[it] }
                    if (y > 0 && y + height > room) {
                        pages += mutableListOf<Placed>()
                        y = 0
                    }
                    line.forEachIndexed { column, index -> pages.last() += Placed(index, column, y, y == 0) }
                    y += height
                }
            } else {
                var column = 0
                items.indices.forEach { index ->
                    if (y > 0 && y + heights[index] > room) {
                        y = 0
                        column++
                        if (column == columns) {
                            pages += mutableListOf<Placed>()
                            column = 0
                        }
                    }
                    pages.last() += Placed(index, column, y, y == 0)
                    y += heights[index]
                }
            }
            return pages
        }
        val height = constraints.maxHeight
        var pages = pack(height)
        if (paged && pages.size > 1) pages = pack((height - EinkSizes.PagerRow.roundToPx() - 8.dp.roundToPx()).coerceAtLeast(0))
        val shown = if (paged) page.coerceIn(0, pages.size - 1) else 0
        // Unpaged, an item whose bottom would pass the room is left off rather than clipped.
        val cells = pages[shown].filter { paged || it.y + heights[it.index] <= height }
        val placed = cells.map { cell -> cell to subcompose("item-${cell.index}-${cell.first}") { item(items[cell.index], cell.first) }.map { it.measure(loose) } }
        val ruleHeight = cells.maxOfOrNull { it.y + heights[it.index] } ?: 0
        val ruled = if (rules && ruleHeight > 0) {
            (1 until columns).map { c ->
                c to subcompose("rule-$c") { Box(Modifier.background(EinkColours.Tertiary)) }.map { it.measure(Constraints.fixed(EinkSizes.Hairline.roundToPx().coerceAtLeast(1), ruleHeight)) }
            }
        } else emptyList()
        val pager = if (paged && pages.size > 1) {
            subcompose("pager") { EinkPager(shown, pages.size, onPage) }.map { it.measure(Constraints.fixedWidth(width)) }
        } else emptyList()
        layout(width, height) {
            placed.forEach { (cell, parts) ->
                var y = cell.y
                parts.forEach { it.place(cell.column * (columnWidth + gap), y); y += it.height }
            }
            ruled.forEach { (c, parts) -> parts.forEach { it.place(c * (columnWidth + gap) - gap / 2, 0) } }
            pager.forEach { it.place(0, height - it.height) }
        }
    }
}

/** The series in progress: its cover with an ink edge, what it is, where the person is, and the one solid button. */
@Composable
private fun PaperHero(tile: WorkTile, spot: ReadSpotlight, actions: AppActions, reader: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val words = actionWords(context, spot.action)
    val chapter = spot.action?.chapter
    val open = {
        if (chapter != null) actions.openReader(tile.work, chapter, tile.cover, readerCoverKey("spotlight", tile.work))
        else navigator.openSeries(tile.work, "eink-home")
    }
    val button: @Composable (Modifier) -> Unit = { m ->
        if (words != null) EinkSolidButton(words, open, m)
        else EinkOutlinedButton(Strings.homeDetails(context), { navigator.openSeries(tile.work, "eink-home") }, m)
    }
    Column(Modifier.padding(top = 12.dp)) {
        Row(verticalAlignment = if (reader) Alignment.Top else Alignment.Bottom) {
            PaperCover(tile.cover, tile.title, heroCover(reader), Modifier.einkClick { navigator.openSeries(tile.work, "eink-home") })
            Column(Modifier.weight(1f).padding(start = 22.dp, top = if (reader) 12.dp else 0.dp)) {
                Column(Modifier.semantics(mergeDescendants = true) {}) {
                    Text(spot.label.text(context), style = EinkType.Secondary)
                    Text(tile.title, style = EinkType.ReaderTitle, maxLines = 3, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 6.dp))
                    spot.line?.let { Text(it.text(context), style = EinkType.Secondary, modifier = Modifier.padding(top = 2.dp)) }
                }
                if (reader) button(Modifier.padding(top = 16.dp))
            }
        }
        // On a phone the button runs the full width, within thumb reach.
        if (!reader) button(Modifier.fillMaxWidth().padding(top = 14.dp))
    }
}

/** The demo while it is the only thing to read: a bordered box with its cover, credit line, title, blurb and Read. */
@Composable
private fun PaperDemo(actions: AppActions, reader: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val comic = readingDemo.comic ?: return
    val work = comic.workId
    Row(
        Modifier
            .padding(top = 12.dp)
            .fillMaxWidth()
            .border(EinkSizes.Rule, EinkColours.Ink, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PaperCover(comic.coverArt, comic.series, if (reader) 92.dp else 80.dp, if (work != null) Modifier.einkClick { navigator.openSeries(work, "eink-home") } else Modifier)
        Column(Modifier.weight(1f).padding(start = 18.dp)) {
            Column(Modifier.semantics(mergeDescendants = true) {}) {
                Text(Strings.homeSpotlightDemo(context), style = EinkType.Secondary)
                Text(comic.series, style = EinkType.ReaderTitle, modifier = Modifier.padding(top = 4.dp).semantics { heading() })
                comic.blurb?.let { Text(it, style = EinkType.Secondary.copy(color = EinkColours.Ink), maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 2.dp)) }
            }
            if (work != null) {
                EinkSolidButton(Strings.homeSpotlightRead(context), { actions.openReader(work, null, comic.coverArt, readerCoverKey("spotlight", work)) }, Modifier.padding(top = 12.dp))
            }
        }
    }
}

/** Under the demo: only the demo is on, with Add a source outlined beside the words. */
@Composable
private fun PaperDemoLine() {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    Row(Modifier.fillMaxWidth().padding(top = 14.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(Strings.homeDemoOnlyBooks(context), style = EinkType.Secondary, modifier = Modifier.weight(1f))
        EinkOutlinedButton(Strings.sourcesAddTitle(context), { navigator.open(app.kaeri.ui.PROVIDERS_ROUTE) }, Modifier.padding(start = 12.dp), height = 48.dp)
    }
}

/** A heading on paper with an underlined link at its end, its words on the margin when [flush]. */
@Composable
private fun PaperHeading(title: String, link: String?, flush: Boolean = false, top: Dp = 10.dp, onLink: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(top = top).height(HeadingHeight), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = EinkType.RowTitle, modifier = Modifier.weight(1f).semantics { heading() })
        // The link's words keep to the end, so a short one grows its target to the left.
        if (link != null) EinkTextButton(link, onLink, Modifier.overhang().then(if (flush) FlushEnd else Modifier), underline = true, align = Alignment.CenterEnd)
    }
}

/** Moves a text button's words onto the end margin, as the 10 inch board sets them; its touch room runs into the margin. */
private val FlushEnd = Modifier.offset(x = 12.dp)

/** The heading line's height, as on the board; its link keeps its full touch height above and below. */
private val HeadingHeight = 32.dp

/** Measures at the full touch height but takes [HeadingHeight] in the line, centred on it. */
private fun Modifier.overhang(): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints.copy(minHeight = 0, maxHeight = Constraints.Infinity))
    val height = HeadingHeight.roundToPx()
    layout(placeable.width, height) { placeable.place(0, (height - placeable.height) / 2) }
}

/** Continue: four covers on an e-reader, three on a phone, each with its title and short line. */
@Composable
private fun PaperContinue(tiles: List<WorkTile>, reader: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    PaperHeading(Strings.homeReadContinue(context), Strings.einkHomeAll(context), flush = true) {
        ReadPlaces.showList(LibrarySegmentRead.READING)
        navigator.go(Destination.Library)
    }
    val width = continueCover(reader)
    // A 6 inch panel has no room for a second line under its covers, so the place takes its short
    // form there, "Ch 13 · p 10"; a 7 inch panel and a phone wrap the full words to two lines.
    val short = reader && !tallPanel()
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(if (reader) 24.dp else 20.dp)) {
        for (tile in tiles) {
            Column(
                Modifier
                    .width(width)
                    .einkClick { navigator.openSeries(tile.work, "eink-continue") }
                    .semantics(mergeDescendants = true) {},
            ) {
                PaperCover(tile.cover, tile.title, width, Modifier)
                Text(tile.title, style = EinkType.Label.copy(fontSize = EinkType.Secondary.fontSize), maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
                val line = tile.line(context)
                val place = if (short) tile.next?.let { shortPlace(context, it) } else null
                // The place ("Chapter 13 · page 4") is the shelf's point, so it is never cut off:
                // two lines of full words, or one in the short form.
                if (place != null) {
                    Text(place, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.semantics { contentDescription = line })
                } else if (line.isNotEmpty()) {
                    Text(line, style = EinkType.Secondary, maxLines = if (short) 1 else 2, overflow = TextOverflow.Ellipsis)
                }
            }
        }
    }
}

/** A place in its short form for a narrow cover, "Ch 13 · p 10", or `null` for any other line. */
internal fun shortPlace(context: Context, line: Msg): String? = when (val msg = (line as? Msg.Reading)?.v1) {
    is ReadingMsg.ReadingChapterPage -> Strings.einkPlaceChapterPage(context, msg.chapter, msg.page.toInt())
    is ReadingMsg.ReadingChapterPercent -> Strings.einkPlaceChapterPercent(context, msg.chapter, msg.percent.toInt())
    else -> null
}

/** A chapter that came out this week, with its day. */
private data class CameOut(val day: ReadWeekDay, val entry: ReadWeekEntry)

/** A 1 dp grey hairline at the top of what it is on. */
private fun Modifier.topHairline(): Modifier = drawBehind {
    val y = EinkSizes.Hairline.toPx() / 2
    drawLine(EinkColours.Tertiary, Offset(0f, y), Offset(size.width, y), EinkSizes.Hairline.toPx())
}

/** This week's chapters that came out, the newest first. */
private fun cameOut(week: List<ReadWeekDay>): List<CameOut> =
    week.flatMap { day -> day.entries.filter { !it.usually && it.outAt != null }.map { CameOut(day, it) } }
        .sortedByDescending { it.entry.outAt }

/** A new chapter's line: the chapter, then its day, "Chapter 15 · The Far Shore · today". */
private fun cameWords(context: Context, day: ReadWeekDay, entry: ReadWeekEntry): String {
    val dayWords = if (day.today) Strings.readingAgoToday(context)
    else Instant.ofEpochSecond(day.day).atZone(app.kaeri.store.AppClock.zone).dayOfWeek.getDisplayName(TextStyle.FULL, app.kaeri.store.AirTime.locale)
    return listOfNotNull((entry.chapter ?: entry.tile.next)?.text(context), dayWords).joinToString(" · ") // i18n-skip
}

/**
 * New chapters on an e-reader, with All opening Updates; This week on a phone, with Updates. Each
 * row: the title, then the dot with the chapter and its day, and a chevron.
 */
@Composable
private fun PaperChapters(came: List<CameOut>, reader: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    if (reader) {
        PaperHeading(Strings.einkHomeNewChapters(context), Strings.einkHomeAll(context)) { navigator.go(Destination.Updates) }
    } else {
        PaperHeading(Strings.homeThisWeek(context), Strings.navUpdates(context)) { navigator.go(Destination.Updates) }
    }
    came.forEachIndexed { index, (day, entry) ->
        val tile = entry.tile
        val line = cameWords(context, day, entry)
        Row(
            Modifier
                .then(if (index > 0) Modifier.layoutId(SHEDDABLE) else Modifier)
                .fillMaxWidth()
                .heightIn(min = EinkSizes.Row)
                .einkClick { navigator.openSeries(tile.work, "eink-week") }
                // The rule between rows sits on the later one, so none is left when it gives way.
                .then(if (index > 0) Modifier.topHairline() else Modifier)
                .padding(vertical = 8.dp)
                .semantics(mergeDescendants = true) { contentDescription = "${tile.title}. $line" }, // i18n-skip
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(tile.title, style = EinkType.Label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 2.dp)) {
                    Box(Modifier.size(EinkSizes.NewDot).clip(CircleShape).background(EinkColours.Ink))
                    Text(line, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = 8.dp))
                }
            }
            Icon(Icons.Chevron, contentDescription = null, tint = EinkColours.Ink, modifier = Modifier.padding(start = 12.dp).size(22.dp))
        }
    }
}

/**
 * Browse under a 2 dp rule: one row opening the list of rows, or with nothing of the person's own
 * above it ([inline]), the list itself, as many rows as fit the screen ([fit]), in [lines]: the
 * height that is left, in the page's column.
 */
@Composable
private fun PaperBrowse(rows: List<app.kaeri.engine.model.ReadHomeRow>, inline: Boolean, fit: Boolean, lines: Modifier) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    if (inline) {
        Text(Strings.readHomeBrowse(context), style = EinkType.RowTitle, modifier = Modifier.padding(top = 18.dp, bottom = 4.dp).semantics { heading() })
        FitLines(lines) {
            for (row in rows) BrowseLine(row) { navigator.openReadPage(READ_PAGE_ROW + row.id) }
        }
    } else {
        Box(Modifier.fillMaxWidth().padding(top = if (fit) 0.dp else 8.dp).topRule())
        BrowseLink(Strings.readHomeBrowse(context), Strings.readHomeBrowseLine(context), colourHome = false) {
            navigator.openReadPage(READ_PAGE_BROWSE)
        }
    }
}

/** Lines one under another, as many as fit whole in the height there is; the rest stay off. */
@Composable
private fun FitLines(modifier: Modifier, content: @Composable () -> Unit) {
    Layout(content, modifier) { measurables, constraints ->
        val loose = constraints.copy(minHeight = 0)
        val placed = mutableListOf<Placeable>()
        var used = 0
        for (measurable in measurables) {
            val placeable = measurable.measure(loose)
            if (constraints.hasBoundedHeight && used + placeable.height > constraints.maxHeight) break
            placed += placeable
            used += placeable.height
        }
        layout(constraints.maxWidth, used) {
            var y = 0
            placed.forEach { it.place(0, y); y += it.height }
        }
    }
}

/** Marks a line that gives way when the page would otherwise not fit, in [ShedToFit]. */
private const val SHEDDABLE = "sheddable"

/**
 * Lines one under another, as a column; those marked [SHEDDABLE] leave, the last first, while the
 * whole is taller than [room]. What is still too tall after that is left to scroll.
 */
@Composable
private fun ShedToFit(room: Int, modifier: Modifier, content: @Composable () -> Unit) {
    Layout(content, modifier) { measurables, constraints ->
        val loose = constraints.copy(minHeight = 0)
        val placeables = measurables.map { it.measure(loose) }
        val shed = linesToShed(placeables.map { it.height }, measurables.map { it.layoutId == SHEDDABLE }, room)
        val kept = placeables.filterIndexed { index, _ -> index !in shed }
        val width = if (constraints.hasBoundedWidth) constraints.maxWidth else kept.maxOfOrNull { it.width } ?: 0
        layout(width, kept.sumOf { it.height }) {
            var y = 0
            kept.forEach { it.place(0, y); y += it.height }
        }
    }
}

/**
 * Which lines to leave out so lines of [heights] fit in [room]: the [sheddable] ones, the last
 * first, only as many as it takes. None when they fit, all of them when even that is too tall.
 */
internal fun linesToShed(heights: List<Int>, sheddable: List<Boolean>, room: Int): Set<Int> {
    var total = heights.sum()
    val shed = mutableSetOf<Int>()
    for (index in heights.indices.reversed()) {
        if (total <= room) break
        if (sheddable.getOrElse(index) { false }) {
            shed += index
            total -= heights[index]
        }
    }
    return shed
}

/** A cover on paper: square corners and a 1 dp ink edge, or the title on its plate with no art. */
@Composable
private fun PaperCover(url: String?, title: String, width: Dp, modifier: Modifier) {
    ReadCover(url, width, modifier.border(EinkSizes.Hairline, EinkColours.Ink), shape = RoundedCornerShape(0.dp), title = title)
}

// Downloads

/** A line of the e-reader's Downloads, packed into pages like any paper list. */
private sealed interface DownLine {
    data class Heading(val text: String, val side: String?) : DownLine
    data class Moving(val series: BookDownloadSeries) : DownLine
    data class Here(val series: BookDownloadSeries) : DownLine
    data class Note(val title: String, val line: String) : DownLine
    data object KeepReady : DownLine
}

/**
 * The e-reader's Downloads: what is downloading with Stop, then what is here by series, then Keep
 * next chapters ready under a 2 dp rule. Progress is the engine's words rewritten in place, never
 * a moving bar: a chapter starting, finishing or failing redraws at once, and page counts at most
 * every five seconds, as each redraw is a panel update.
 */
@Composable
internal fun EinkDownloadsPage() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val paced = remember(store) { store.reading.downloads.pacedForPanel(DOWNLOADS_HOLD_MS, shape = ::downloadsShape) }
    // Only the first frame's value, as collecting the StateFlow itself starts from; the paced flow follows it.
    @SuppressLint("StateFlowValueCalledInComposition")
    val first = store.reading.downloads.value
    val view by paced.collectAsStateWithLifecycle(initialValue = first)
    val settings by store.settings.view.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.reading.loadDownloads() }
    var page by rememberSaveable { mutableIntStateOf(0) }
    if (largePanel()) {
        LargeDownloads(view, page) { page = it }
        return
    }
    PaperPage(back = null, title = Strings.downloadsTitle(context)) {
        val v = view
        val used = Wording.bytes(context, v?.meter?.used?.toLong() ?: 0L)
        // The engine's free space, sent with every change, so it follows the downloads; left out
        // when the system reports none.
        val free = v?.meter?.free?.toLong()?.takeIf { it > 0 }
        Text(
            if (free != null) Strings.einkDownloadsSpace(context, used, Wording.bytes(context, free)) else Strings.einkDownloadsSpaceUsed(context, used),
            style = EinkType.Secondary,
        )
        if (v == null) {
            PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp)) }
            return@PaperPage
        }
        val wifiOnly = !store.settings.isOn(SettingKey.CHAPTER_DOWNLOADS_ON_CELLULAR, settings)
        val moving = v.series.filter { it.queued > 0u || it.failed.isNotEmpty() }
        val here = v.series.filter { it.chapters > 0u }
        val lines = buildList {
            if (moving.isNotEmpty()) {
                add(DownLine.Heading(Strings.einkDownloadsDownloading(context), if (wifiOnly) Strings.settingsWifiOnly(context) else null))
                moving.forEach { add(DownLine.Moving(it)) }
            }
            if (here.isNotEmpty()) {
                add(DownLine.Heading(Strings.einkDownloadsHere(context), null))
                here.forEach { add(DownLine.Here(it)) }
            }
            if (moving.isEmpty() && here.isEmpty()) add(DownLine.Note(Strings.downloadsEmptyTitle(context), Strings.readDownloadsEmptyDetail(context)))
            add(DownLine.KeepReady)
        }
        PagedItems(lines, page, { page = it }, Modifier.padding(top = 8.dp)) { line -> PaperDownLine(line, v.rows) }
    }
}

@Composable
private fun PaperDownLine(line: DownLine, rows: List<BookDownloadRow>) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    when (line) {
        is DownLine.Heading -> Row(Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(line.text, style = EinkType.RowTitle, modifier = Modifier.weight(1f).semantics { heading() })
            line.side?.let { Text(it, style = EinkType.Secondary) }
        }
        is DownLine.Moving -> {
            val series = line.series
            val retry = { store.reading.download(series.work, DownloadPick.Chapters(series.failed)) }
            // Only what is happening: the totals are On this device's, below. One line while
            // chapters come, so a rewrite never changes the row's height or moves rows between
            // pages; a failure alone is never rewritten and keeps two for its reason.
            val parts = movingLine(series)
            val words = parts.joinToString(" · ") { it.text(context) } // i18n-skip
            Column(Modifier.fillMaxWidth().heightIn(min = EinkSizes.Row).hairline().padding(vertical = 8.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f).semantics(mergeDescendants = true) {}) {
                        Text(series.title, style = EinkType.Label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (words.isNotEmpty()) Text(words, style = EinkType.Secondary, maxLines = if (series.queued > 0u) 1 else 2, overflow = TextOverflow.Ellipsis)
                    }
                    if (series.queued > 0u) {
                        // Stop while anything is still to come, failures or not.
                        val going = rows.filter { it.work == series.work && it.state !is ChapterDownloadState.Done && it.state !is ChapterDownloadState.Failed }.map { it.chapter.id }
                        EinkTextButton(Strings.actionStop(context), { store.reading.cancelDownloads(series.work, going) })
                    } else {
                        EinkTextButton(stringResource(R.string.chapter_retry), retry)
                    }
                }
                // A failure beside chapters still to come: Retry on a line of its own, saying which
                // chapter unless the engine's line above already does.
                if (series.queued > 0u && series.failed.isNotEmpty()) {
                    val said = parts.any { (it as? Msg.Reading)?.v1 is ReadingMsg.ReadingDlFailed }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        val failed = if (said) null else failedWords(series, rows)?.text(context)
                        Text(failed.orEmpty(), style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                        EinkTextButton(stringResource(R.string.chapter_retry), retry)
                    }
                }
            }
        }
        is DownLine.Here -> {
            val series = line.series
            // While chapters still come, the row says what is here; the engine's line says the progress above.
            val words = if (series.queued > 0u || series.failed.isNotEmpty()) Strings.readDownloadsTotal(context, series.chapters.toInt(), Wording.bytes(context, series.bytes.toLong()))
            else series.line.joinToString(" · ") { it.text(context) } // i18n-skip
            BrowseLink(series.title, words, colourHome = false) { navigator.openSeries(series.work, "eink-downloads") }
        }
        is DownLine.Note -> Column(Modifier.padding(vertical = 12.dp)) {
            Text(line.title, style = EinkType.Body)
            Text(line.line, style = EinkType.Secondary)
        }
        DownLine.KeepReady -> {
            // A row with a chevron, as the board draws it, opening the downloads settings where
            // the feature is set; a tap here never turns it off.
            val settings by store.settings.view.collectAsStateWithLifecycle()
            val on = store.settings.isOn(SettingKey.KEEP_CHAPTERS_READY, settings)
            val state = if (on) Strings.einkKeepReadyOn(context) else Strings.einkKeepReadyOff(context)
            Column(Modifier.padding(top = 16.dp).fillMaxWidth().topRule().padding(top = EinkSizes.Rule)) {
                BrowseLink(Strings.settingsKeepChaptersReady(context), state, colourHome = false) {
                    navigator.openSettings(SettingsPane.Downloads)
                }
            }
        }
    }
}

/** A tab's own page on paper with words at the end of its title line, as the 10 inch board puts Downloads' summary. */
@Composable
private fun PaperTabPage(title: String, side: String?, content: @Composable () -> Unit) {
    EinkTheme(LocalEinkLayout.current ?: EinkLook()) {
        Column(
            Modifier
                .fillMaxSize()
                .background(EinkColours.Paper)
                .windowInsetsPadding(WindowInsets.statusBars)
                .padding(horizontal = Side),
        ) {
            Row(Modifier.fillMaxWidth().padding(top = 22.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(title, style = EinkType.PageTitle, modifier = Modifier.weight(1f).semantics { heading() })
                if (side != null) Text(side, style = EinkType.Secondary, modifier = Modifier.padding(start = 16.dp))
            }
            content()
        }
    }
}

/** How many chapters still to come the 10 inch Downloads lists before saying how many more are queued. */
private const val QUEUE_SHOWN = 4

/**
 * Downloads on a 10 inch panel: the summary in the title line, Keep next chapters ready between
 * two rules, each chapter still to come with its progress in words, then the series on this
 * device in two columns of rows with their covers, paged.
 */
@Composable
private fun LargeDownloads(view: app.kaeri.engine.model.BookDownloadsView?, page: Int, onPage: (Int) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val used = Wording.bytes(context, view?.meter?.used?.toLong() ?: 0L)
    val free = view?.meter?.free?.toLong()?.takeIf { it > 0 }
    val summary = if (free != null) Strings.einkDownloadsSpace(context, used, Wording.bytes(context, free)) else Strings.einkDownloadsSpaceUsed(context, used)
    PaperTabPage(Strings.downloadsTitle(context), summary) {
        LargeKeepReady()
        if (view == null) {
            PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp)) }
            return@PaperTabPage
        }
        // Downloading first, then failures, then what waits, so what needs a look stays in view.
        val queue = view.rows.filter { it.state !is ChapterDownloadState.Done }.sortedBy {
            when (it.state) {
                is ChapterDownloadState.Downloading -> 0
                is ChapterDownloadState.Failed -> 1
                else -> 2
            }
        }
        val here = view.series.filter { it.chapters > 0u }
        if (queue.isNotEmpty()) {
            val going = queue.filter { it.state !is ChapterDownloadState.Failed }
            LargeHeading(Strings.einkDownloadsDownloadingCount(context, queue.size), if (going.isNotEmpty()) Strings.einkDownloadsStopAll(context) else null) {
                going.groupBy { it.work }.forEach { (work, rows) -> store.reading.cancelDownloads(work, rows.map { it.chapter.id }) }
            }
            queue.take(QUEUE_SHOWN).forEachIndexed { index, row -> QueueRow(row, first = index == 0) }
            val more = queue.size - QUEUE_SHOWN
            if (more > 0) {
                Text(
                    Msg.Reading(ReadingMsg.ReadingDlMoreQueued(more.toLong())).text(context),
                    style = EinkType.Secondary,
                    modifier = Modifier.fillMaxWidth().topHairline().padding(vertical = 12.dp),
                )
            }
        }
        if (here.isNotEmpty()) {
            LargeHeading(Strings.einkDownloadsHereCount(context, here.size), null) {}
            PagedColumns(here, page, onPage, Modifier.padding(top = 4.dp, bottom = 12.dp), rules = true) { series, first -> HereRow(series, first) }
        } else if (queue.isEmpty()) {
            Column(Modifier.padding(vertical = 16.dp)) {
                Text(Strings.downloadsEmptyTitle(context), style = EinkType.Body)
                Text(Strings.readDownloadsEmptyDetail(context), style = EinkType.Secondary)
            }
        }
    }
}

/** A chapter by its label: a bare number reads as Chapter 15, a label with words of its own stays as it is. */
private fun chapterWords(context: android.content.Context, label: String): String =
    if (label.firstOrNull()?.isDigit() == true) Strings.einkChapter(context, label) else label

/** A heading in the 10 inch lists, 22 dp under what is above, with an underlined link at its end. */
@Composable
private fun LargeHeading(title: String, link: String?, onLink: () -> Unit) {
    Spacer(Modifier.height(12.dp))
    PaperHeading(title, link, flush = true, onLink = onLink)
}

/** Keep next chapters ready between two 2 dp rules, saying how it is set, with Change opening the downloads settings. */
@Composable
private fun LargeKeepReady() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val on = store.settings.isOn(SettingKey.KEEP_CHAPTERS_READY, settings)
    val change = { navigator.openSettings(SettingsPane.Downloads) }
    Row(
        Modifier
            .padding(top = 12.dp)
            .fillMaxWidth()
            .heightIn(min = 76.dp)
            .topRule()
            .bottomRule()
            .einkClick(onClick = change)
            .padding(vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f).semantics(mergeDescendants = true) {}) {
            Text(Strings.settingsKeepChaptersReady(context), style = LargeRowLabel)
            Text(if (on) Strings.einkKeepReadyOn(context) else Strings.einkKeepReadyOff(context), style = EinkType.Secondary, modifier = Modifier.padding(top = 2.dp))
        }
        EinkTextButton(Strings.einkKeepReadyChange(context), change, FlushEnd, underline = true)
    }
}

/** A chapter still to come: the series and chapter, and at the end where it stands in words; Retry on a failure. */
@Composable
private fun QueueRow(row: BookDownloadRow, first: Boolean) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val state = row.state
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .then(if (first) Modifier else Modifier.topHairline())
            .padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        val words = when (state) {
            is ChapterDownloadState.Downloading -> Msg.Reading(ReadingMsg.ReadingDlPages(state.done.toLong(), state.total.toLong())).text(context)
            is ChapterDownloadState.Failed -> Strings.chapterStateFailed(context, failureWords(context, state.failure))
            ChapterDownloadState.WaitsForWifi -> stringResource(R.string.chapter_state_wifi)
            else -> stringResource(R.string.chapter_state_queued)
        }
        Text(
            listOf(row.title, chapterWords(context, row.chapter.label)).joinToString(" · "), // i18n-skip
            style = LargeRowTitle,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        Text(
            words,
            style = EinkType.Secondary.copy(
                color = EinkColours.Ink,
                fontWeight = if (state is ChapterDownloadState.Downloading) androidx.compose.ui.text.font.FontWeight.Bold else EinkType.Secondary.fontWeight,
                fontFeatureSettings = "tnum", // i18n-skip
            ),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(start = 16.dp),
        )
        if (state is ChapterDownloadState.Failed) {
            EinkTextButton(stringResource(R.string.chapter_retry), { store.reading.download(row.work, DownloadPick.Chapters(listOf(row.chapter.id))) }, FlushEnd, underline = true)
        }
    }
}

/** A series on this device: its 60 dp cover, the title, what is here and its size, and a chevron. */
@Composable
private fun HereRow(series: BookDownloadSeries, first: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val words = if (series.queued > 0u || series.failed.isNotEmpty()) Strings.readDownloadsTotal(context, series.chapters.toInt(), Wording.bytes(context, series.bytes.toLong()))
    else series.line.joinToString(" · ") { it.text(context) } // i18n-skip
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 110.dp)
            .einkClick { navigator.openSeries(series.work, "eink-downloads") }
            .then(if (first) Modifier else Modifier.topHairline())
            .padding(vertical = 10.dp)
            .semantics(mergeDescendants = true) { contentDescription = "${series.title}. $words" }, // i18n-skip
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PaperCover(series.cover, series.title, 60.dp, Modifier)
        Column(Modifier.weight(1f).padding(start = 16.dp)) {
            Text(series.title, style = LargeRowTitle, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (words.isNotEmpty()) Text(words, style = EinkType.Secondary, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 2.dp))
        }
        Icon(Icons.Chevron, contentDescription = null, tint = EinkColours.Ink, modifier = Modifier.padding(start = 12.dp).size(22.dp))
    }
}

/**
 * A series' Downloading line without the totals On this device already says: the engine's line
 * begins with the chapters here, their size and how many are read.
 */
internal fun movingLine(series: BookDownloadSeries): List<Msg> = series.line.filterNot { part ->
    val reading = (part as? Msg.Reading)?.v1
    reading is ReadingMsg.ReadingChapters || reading is ReadingMsg.ReadingDlParts ||
        reading is ReadingMsg.ReadingDlSize || reading is ReadingMsg.ReadingDlRead
}

/** "Chapter 2 couldn't download", for the first chapter of [series] that failed. */
private fun failedWords(series: BookDownloadSeries, rows: List<BookDownloadRow>): Msg? {
    val first = series.failed.firstOrNull() ?: return null
    val row = rows.firstOrNull { it.work == series.work && it.chapter.id == first }
    return Msg.Reading(ReadingMsg.ReadingDlFailed(row?.let { chapterNumber(it.chapter) } ?: return null))
}

// More

/**
 * The e-reader's More: Sources, Trackers, Sync, Reader settings, Settings and About, each saying
 * what is behind it, as rows with a chevron.
 */
@Composable
internal fun EinkMorePage() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val books by store.bookSources.view.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val sync by store.sync.status.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) {
        store.bookSources.load()
        store.loadSync()
    }
    val answer = kaeriIsFor()
    val version = appVersion()
    val rows = listOf(
        Triple(Strings.sourcesTitle(context), bookSourcesSummary(context, books).orEmpty(), SettingsPane.Sources),
        Triple(Strings.einkMoreTrackers(context), AccountWords.connected(context, trackers, answer), SettingsPane.Accounts),
        Triple(Strings.einkMoreSync(context), devicesSummary(context, sync), SettingsPane.Devices),
        Triple(Strings.readerSettings(context), readingSummary().orEmpty(), SettingsPane.Reading),
        Triple(Strings.settingsTitle(context), Strings.einkMoreSettingsLine(context), null),
        Triple(Strings.aboutTitle(context), Strings.aboutVersion(context, version), SettingsPane.About),
    )
    if (largePanel()) {
        LargeMore(rows.take(4), rows.drop(4))
        return
    }
    PaperPage(back = null, title = Strings.einkTabMore(context)) {
        Column(Modifier.padding(top = 8.dp)) {
            for ((title, line, pane) in rows) BrowseLink(title, line, colourHome = false) { navigator.openSettings(pane) }
        }
    }
}

/**
 * More on a 10 inch panel: the rows in two columns, Your reading and This device, then under a
 * 2 dp rule the newest messages since the app started with their times, as many as fit.
 */
@Composable
private fun LargeMore(reading: List<Triple<String, String, SettingsPane?>>, device: List<Triple<String, String, SettingsPane?>>) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val log by store.notices.log.collectAsStateWithLifecycle()
    PaperPage(back = null, title = Strings.einkTabMore(context)) {
        Row(Modifier.fillMaxWidth().padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(ListGap)) {
            for ((name, rows) in listOf(Strings.einkMoreYourReading(context) to reading, Strings.einkMoreThisDevice(context) to device)) {
                Column(Modifier.weight(1f)) {
                    Text(name, style = EinkType.Secondary.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Bold), modifier = Modifier.padding(bottom = 6.dp).semantics { heading() })
                    rows.forEachIndexed { index, (title, line, pane) -> MoreRow(title, line, first = index == 0) { navigator.openSettings(pane) } }
                }
            }
        }
        Box(Modifier.padding(top = 26.dp).fillMaxWidth().height(EinkSizes.Rule).background(EinkColours.Ink))
        Spacer(Modifier.height(6.dp))
        PaperHeading(Strings.einkMoreMessages(context), null) {}
        if (log.isEmpty()) {
            Text(Strings.einkMoreMessagesNone(context), style = EinkType.Secondary, modifier = Modifier.padding(top = 12.dp))
        } else {
            FitLines(Modifier.padding(top = 4.dp)) {
                log.forEachIndexed { index, item ->
                    val words = item.text(context)
                    val time = messageTime(context, item.at)
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 56.dp)
                            .then(if (index == 0) Modifier else Modifier.topHairline())
                            .padding(vertical = 8.dp)
                            .semantics(mergeDescendants = true) { contentDescription = "$time. $words" }, // i18n-skip
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(time, style = EinkType.Folio, maxLines = 1, modifier = Modifier.width(130.dp).padding(end = 12.dp))
                        Text(words, style = EinkType.Body, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
                    }
                }
            }
        }
    }
}

/** A row of the 10 inch More: what it opens, its line, and a chevron, 80 dp at the least. */
@Composable
private fun MoreRow(title: String, line: String, first: Boolean, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 80.dp)
            .einkClick(onClick = onClick)
            .then(if (first) Modifier else Modifier.topHairline())
            .padding(vertical = 10.dp)
            .semantics(mergeDescendants = true) { contentDescription = "$title. $line" }, // i18n-skip
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = LargeRowLabel, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (line.isNotEmpty()) Text(line, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 2.dp))
        }
        Icon(Icons.Chevron, contentDescription = null, tint = EinkColours.Ink, modifier = Modifier.padding(start = 12.dp).size(22.dp))
    }
}

/** When a message came, as the board writes it: the time today, Yesterday, else the day and month. */
private fun messageTime(context: Context, millis: Long): String {
    val zone = app.kaeri.store.AppClock.zone
    val at = Instant.ofEpochMilli(millis)
    val day = at.atZone(zone).toLocalDate()
    val today = app.kaeri.store.AppClock.now().atZone(zone).toLocalDate()
    return when (day) {
        today -> clockWords(at.epochSecond)
        today.minusDays(1) -> Strings.chapterUpdatesYesterday(context)
        else -> java.time.format.DateTimeFormatter.ofPattern(app.kaeri.store.AirTime.bestPattern("dMMMM", "d MMMM"), app.kaeri.store.AirTime.locale).format(day) // i18n-skip
    }
}
