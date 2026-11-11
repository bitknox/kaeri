// Discovery on e-ink, as the discovery board draws it: one Browse row on Home opening the list of
// the rows, each row a paged 3 × 3 grid whose Search link opens the same filter. Search is in
// EinkSearch.kt. Paper and ink, no motion: a page turn redraws the page. Pages are packed by
// measuring, so large text gets more pages rather than clipped rows.

package app.kaeri.ui.reading

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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.SubcomposeLayout
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkLook
import app.kaeri.eink.EinkOutlinedButton
import app.kaeri.eink.EinkPager
import app.kaeri.eink.EinkSizes
import app.kaeri.eink.EinkTextButton
import app.kaeri.eink.EinkTheme
import app.kaeri.eink.EinkType
import app.kaeri.eink.PaperWaiting
import app.kaeri.eink.einkClick
import app.kaeri.eink.hairline
import app.kaeri.engine.model.ReadHomeRow
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.Strings
import app.kaeri.store.text
import app.kaeri.ui.Destination
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator

/**
 * The e-ink look while the panel is e-ink, else `null`: Home then shows one Browse row in place
 * of its discovery rows, and Browse, a row's grid and Search draw on paper.
 */
val LocalEinkLayout = staticCompositionLocalOf<EinkLook?> { null }

/**
 * The gutter of the paper pages: 28 dp, as the boards draw the 6 and 7 inch panels, and 40 on a
 * panel 900 dp across and wider, as the e-ink board draws the 10.3 inch.
 */
internal val Side: Dp
    @Composable @ReadOnlyComposable
    get() = if (LocalConfiguration.current.screenWidthDp >= LARGE_PANEL_DP) 40.dp else 28.dp

/** A panel this many dp across and wider takes the 10.3 inch board's gutter. */
private const val LARGE_PANEL_DP = 900

/** A paged cover grid's column is at least this wide; a panel holds as many as fit, three at least. */
private val GridColumnLeast = 200.dp

/** Between a grid's columns. */
private val GridColumnGap = 24.dp

/** What a grid row adds to its cover and its words: the gap above the title and the gap under the row. */
private val GridGaps = 8.dp + 16.dp

/** A grid cover at its smallest; below it a page holds fewer rows. */
private val GridCoverLeast = 56.dp

/** From this cover width a tile's title and line take the body size rather than the small one. */
internal val GridLargeFrom = 140.dp

/** How a paged cover grid fills its page: its columns and rows, and the cover's width. */
@Immutable
internal data class PaperGrid(val columns: Int, val rows: Int, val cover: Dp) {
    val perPage: Int get() = columns * rows
    /** The tile's words at the body size, under a large cover. */
    val large: Boolean get() = cover >= GridLargeFrom
}

/**
 * The grid for a page [width] across with [height] under the title, the pager included. The
 * columns come from the width: as many of at least [GridColumnLeast] as fit, three at least. The
 * rows come from the height: as few as fill it with covers no wider than their column, three at
 * least on an [upright] panel and two on its side, so a page is always full. The covers then take
 * the size that fits; a row's words are [small] or, under a large cover, [large] tall.
 */
internal fun paperGrid(width: Dp, height: Dp, small: Dp, large: Dp, upright: Boolean = true): PaperGrid {
    val columns = (((width + GridColumnGap) / (GridColumnLeast + GridColumnGap)).toInt()).coerceAtLeast(3)
    val column = (width - GridColumnGap * (columns - 1)) / columns
    val room = height - EinkSizes.PagerRow
    val least = if (upright) 3 else 2
    fun cover(rows: Int, words: Dp): Dp = minOf(column, ((room / rows) - GridGaps - words - 1.dp) / 1.5f)
    var rows = maxOf(least, kotlin.math.ceil((room / (column * 1.5f + GridGaps + large)).toDouble()).toInt())
    var words = large
    var width0 = cover(rows, words)
    if (width0 < GridLargeFrom) {
        words = small
        width0 = cover(rows, words)
    }
    // Never below the least cover: fewer rows instead.
    while (width0 < GridCoverLeast && rows > 1) {
        rows--
        width0 = cover(rows, words)
    }
    return PaperGrid(columns, rows, width0.coerceAtLeast(GridCoverLeast))
}

// Home's Browse row

/**
 * Browse on e-ink Home: under a 2 dp rule, one row opening the list of rows; with nothing of the
 * person's own above it ([inline]), the list itself, so a first run has something to open.
 */
@Composable
internal fun BrowseEntry(rows: List<ReadHomeRow>, inline: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    val landed = rows.filter { it.tiles.isNotEmpty() }
    Column(Modifier.padding(top = 24.dp).padding(horizontal = 16.dp)) {
        Box(Modifier.fillMaxWidth().height(EinkSizes.Rule).background(colors.onSurface))
        if (inline) {
            Text(
                Strings.readHomeBrowse(context),
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.padding(top = 16.dp, bottom = 4.dp).semantics { heading() },
            )
            for (row in landed) BrowseLine(row, colourHome = true) { navigator.openReadPage(READ_PAGE_ROW + row.id) }
        } else {
            BrowseLink(
                title = Strings.readHomeBrowse(context),
                line = Strings.readHomeBrowseLine(context),
                colourHome = true,
            ) { navigator.openReadPage(READ_PAGE_BROWSE) }
        }
    }
}

/** A row of the Browse list: its title, its first two series and how many follow, and a chevron. */
@Composable
internal fun BrowseLine(row: ReadHomeRow, colourHome: Boolean = false, onClick: () -> Unit) {
    val context = LocalContext.current
    BrowseLink(row.title.text(context), browseRowLine(context, row), colourHome, onClick)
}

/** "Iron Kite Brigade, Saltwind Diary and 18 more", or the one or two there are. */
internal fun browseRowLine(context: android.content.Context, row: ReadHomeRow): String {
    val tiles = row.tiles
    return when (tiles.size) {
        0 -> ""
        1 -> tiles[0].title
        2 -> joinedNames(context, tiles.map { it.title })
        else -> Strings.readHomeBrowseRow(context, tiles[0].title, tiles[1].title, tiles.size - 2)
    }
}

/** A list row with a title, a line under it and a chevron, on paper or in the colour Home. */
@Composable
internal fun BrowseLink(title: String, line: String, colourHome: Boolean, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val titleStyle = if (colourHome) MaterialTheme.typography.titleMedium else EinkType.Label
    val lineStyle = if (colourHome) MaterialTheme.typography.bodyMedium.copy(color = colors.onSurfaceVariant) else EinkType.Secondary
    val ink = if (colourHome) colors.onSurface else EinkColours.Ink
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = EinkSizes.Row)
            .einkClick(onClick = onClick)
            .hairline(if (colourHome) colors.outlineVariant else EinkColours.Tertiary)
            .padding(vertical = 8.dp)
            .semantics(mergeDescendants = true) { contentDescription = "$title. $line" }, // i18n-skip
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = titleStyle.copy(color = ink))
            if (line.isNotEmpty()) Text(line, style = lineStyle, modifier = Modifier.padding(top = 2.dp))
        }
        Icon(Icons.Chevron, contentDescription = null, tint = ink, modifier = Modifier.padding(start = 12.dp).size(22.dp))
    }
}

// Paper pages

/** A page on paper: "‹ Back to", the title with an optional link at its end, then [content]. */
@Composable
internal fun PaperPage(
    /** The back line's words; null on a tab's own page, which has none. */
    back: String?,
    title: String,
    link: String? = null,
    onLink: () -> Unit = {},
    content: @Composable () -> Unit,
) {
    val navigator = LocalNavigator.current
    val look = LocalEinkLayout.current ?: EinkLook()
    EinkTheme(look) {
        Column(
            Modifier
                .fillMaxSize()
                .background(EinkColours.Paper)
                .windowInsetsPadding(WindowInsets.statusBars)
                .padding(horizontal = Side),
        ) {
            if (back != null) {
                Text(
                    "‹ $back", // i18n-skip
                    style = EinkType.Label,
                    modifier = Modifier.heightIn(min = 48.dp).einkClick { navigator.back() }.padding(top = 14.dp),
                )
            }
            Row(Modifier.fillMaxWidth().padding(top = if (back != null) 8.dp else 22.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(title, style = EinkType.PageTitle, modifier = Modifier.weight(1f).semantics { heading() })
                if (link != null) EinkTextButton(link, onLink, underline = true, modifier = Modifier.padding(start = 8.dp))
            }
            content()
        }
    }
}

/**
 * Items packed into pages that fit the height there is, measured as they are, with the pager
 * under them: large text gets more pages, never a clipped row. [most] caps a page's items.
 */
@Composable
internal fun <T> PagedItems(
    items: List<T>,
    page: Int,
    onPage: (Int) -> Unit,
    modifier: Modifier = Modifier,
    most: Int = Int.MAX_VALUE,
    item: @Composable (T) -> Unit,
) {
    SubcomposeLayout(modifier.fillMaxSize()) { constraints ->
        val width = constraints.maxWidth
        val pagerHeight = EinkSizes.PagerRow.roundToPx()
        val room = (constraints.maxHeight - pagerHeight).coerceAtLeast(0)
        val loose = Constraints(maxWidth = width)
        val heights = items.mapIndexed { index, it ->
            subcompose("measure-$index") { item(it) }.sumOf { m -> m.measure(loose).height }
        }
        // Pack: a page takes items while they fit; an item taller than a page has one of its own.
        val starts = mutableListOf(0)
        var used = 0
        var count = 0
        heights.forEachIndexed { index, h ->
            if (count > 0 && (used + h > room || count >= most)) {
                starts += index
                used = 0
                count = 0
            }
            used += h
            count++
        }
        val pages = starts.size
        val shown = page.coerceIn(0, pages - 1)
        val from = starts[shown]
        val to = if (shown + 1 < pages) starts[shown + 1] else items.size
        val placeables = (from until to).flatMap { index ->
            subcompose("page-$index") { item(items[index]) }.map { it.measure(loose) }
        }
        val pager = if (pages > 1) {
            subcompose("pager") { EinkPager(shown, pages, onPage) }.map { it.measure(Constraints.fixedWidth(width)) }
        } else emptyList()
        layout(width, constraints.maxHeight) {
            var y = 0
            placeables.forEach { it.place(0, y); y += it.height }
            pager.forEach { it.place(0, constraints.maxHeight - it.height) }
        }
    }
}

/** E-ink Browse: the same rows and order as the colour Home, as a list, each opening its grid. */
@Composable
internal fun EinkBrowsePage() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val home by store.reading.home.collectAsStateWithLifecycle()
    // Opened with Home not held, as after the process was let go: ask for it.
    LaunchedEffect(Unit) { if (store.reading.home.value == null) store.reading.loadHome() }
    var page by rememberSaveable { mutableIntStateOf(0) }
    val view = home
    val rows = view?.rows.orEmpty().filter { it.tiles.isNotEmpty() }
    PaperPage(back = Strings.homeTitle(context), title = Strings.readHomeBrowse(context)) {
        when {
            view?.failed == true || (view?.offline == true && rows.isNotEmpty()) -> {
                val since = view.offlineSince
                PaperFailure(
                    title = Strings.readSearchAnilistFailed(context),
                    line = since?.let { Strings.readHomeBrowseKept(context, clockWords(it)) },
                    action = Strings.actionTryAgain(context),
                ) { store.reading.loadHome(refresh = true) }
            }
            else -> Text(Strings.readHomeBrowseNote(context), style = EinkType.Secondary, modifier = Modifier.padding(bottom = 8.dp))
        }
        if (rows.isEmpty()) {
            // Loading with no motion: after 600 ms one word, and the list in one refresh when it lands.
            if (view == null || view.rows.any { it.pending }) {
                PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp)) }
            }
        } else if (largePanel()) {
            // A 10 inch panel: each row as a tile with its first covers, AniList's genres at the foot.
            LargeBrowse(rows, page) { page = it }
        } else {
            PagedItems(rows, page, { page = it }) { row ->
                BrowseLine(row) { navigator.openReadPage(READ_PAGE_ROW + row.id) }
            }
        }
    }
}

/** A row from Browse: a grid sized to the panel, a page at a time; Search opens the same filter for more. */
@Composable
internal fun EinkRowPage(id: String) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val home by store.reading.home.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { if (store.reading.home.value == null) store.reading.loadHome() }
    var page by rememberSaveable(id) { mutableIntStateOf(0) }
    val row = home?.rows?.firstOrNull { it.id == id } ?: return
    val query = row.viewAll
    PaperPage(
        back = Strings.readHomeBrowse(context),
        title = row.title.text(context),
        link = query?.let { Strings.homeSearch(context) },
        onLink = {
            if (query != null) {
                // The Search tab itself at its start, so the bar marks Search rather than Home.
                ReadPlaces.search(store.reading, query)
                navigator.goToStart(Destination.Search)
            }
        },
    ) {
        PagedCovers(row.tiles, page, { page = it }, Modifier.padding(top = 8.dp)) { tile, grid ->
            PaperTile(tile, grid) { navigator.openSeries(tile.work, "eink-row") }
        }
    }
}

/** The heights of a grid tile's two lines of words, small and large, at the text size in use. */
@Composable
private fun tileWords(): Pair<Dp, Dp> {
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    fun height(title: androidx.compose.ui.text.TextStyle, line: androidx.compose.ui.text.TextStyle) =
        with(density) { (measurer.measure("Ag", title).size.height + measurer.measure("Ag", line).size.height).toDp() } // i18n-skip
    return height(TileTitle, EinkType.Secondary) to height(TileTitleLarge, TileLineLarge)
}

/**
 * Covers in a grid sized to the page ([paperGrid]), a page at a time with the pager under them.
 * The page count follows from how many a page holds.
 */
@Composable
internal fun <T> PagedCovers(
    items: List<T>,
    page: Int,
    onPage: (Int) -> Unit,
    modifier: Modifier = Modifier,
    tile: @Composable (T, PaperGrid) -> Unit,
) {
    val (small, large) = tileWords()
    BoxWithConstraints(modifier.fillMaxSize()) {
        val upright = LocalConfiguration.current.let { it.screenHeightDp >= it.screenWidthDp }
        val grid = paperGrid(maxWidth, maxHeight, small, large, upright)
        val pages = items.chunked(grid.perPage)
        val shown = page.coerceIn(0, (pages.size - 1).coerceAtLeast(0))
        Column(Modifier.fillMaxSize()) {
            Column(Modifier.weight(1f)) {
                for (row in pages.getOrNull(shown).orEmpty().chunked(grid.columns)) {
                    Row(Modifier.fillMaxWidth().padding(bottom = 16.dp), horizontalArrangement = Arrangement.spacedBy(GridColumnGap)) {
                        for (each in row) Box(Modifier.weight(1f)) { tile(each, grid) }
                        repeat(grid.columns - row.size) { Spacer(Modifier.weight(1f)) }
                    }
                }
            }
            if (pages.size > 1) EinkPager(shown, pages.size, onPage)
        }
    }
}

/** A grid tile's title. */
private val TileTitle = EinkType.Label.copy(fontSize = EinkType.Secondary.fontSize)

/** A large grid tile's title and line: the body size, one step up the type scale. */
private val TileTitleLarge = EinkType.Label
private val TileLineLarge = EinkType.Body.copy(color = EinkColours.Secondary)

/**
 * A cover on paper with its edge at the grid's size, the title and its line under it, one line
 * each and cut with an ellipsis, as the board draws them. Cover and words are centred in the
 * column. A new chapter takes the line's place as the dot and "New", which TalkBack hears too.
 */
@Composable
internal fun PaperTile(
    tile: WorkTile,
    grid: PaperGrid,
    line: String = tile.line(LocalContext.current),
    onClick: () -> Unit,
) {
    val newWord = Strings.einkNew(LocalContext.current)
    val lineStyle = if (grid.large) TileLineLarge else EinkType.Secondary
    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
        Column(
            Modifier
                .width(grid.cover)
                .einkClick(onClick = onClick)
                .semantics(mergeDescendants = true) { if (tile.newChapter) stateDescription = newWord },
        ) {
            ReadCover(tile.cover, grid.cover, Modifier.border(EinkSizes.Hairline, EinkColours.Ink), shape = RoundedCornerShape(0.dp), title = tile.title)
            Text(tile.title, style = if (grid.large) TileTitleLarge else TileTitle, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
            if (tile.newChapter) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(EinkSizes.NewDot).background(EinkColours.Ink, CircleShape))
                    Text(newWord, style = lineStyle.copy(color = EinkColours.Ink), maxLines = 1, modifier = Modifier.padding(start = 7.dp))
                }
            } else if (line.isNotEmpty()) {
                Text(line, style = lineStyle, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** The glyph, what happened and its line, and an outlined button at the end. */
@Composable
internal fun PaperFailure(title: String, line: String?, action: String, onAction: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(Icons.Problem, contentDescription = null, tint = EinkColours.Ink, modifier = Modifier.padding(end = 12.dp).size(24.dp))
        Column(Modifier.weight(1f).semantics(mergeDescendants = true) {}) {
            Text(title, style = EinkType.Body)
            if (line != null) Text(line, style = EinkType.Secondary)
        }
        EinkOutlinedButton(action, onAction, Modifier.padding(start = 12.dp), height = 48.dp)
    }
}
