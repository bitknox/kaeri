// Discovery on e-ink, as the discovery board draws it: one Browse row on Home opening the list of
// the rows, each row a paged 3 × 3 grid whose Search link opens the same filter, and Search as a
// paged list with Your sources only as a check box beside Filters. Paper and ink, no motion: a
// page turn redraws the page. Pages are packed by measuring, so large text gets more pages
// rather than clipped rows.

package app.kaeri.ui.reading

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.layout.SubcomposeLayout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
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
import app.kaeri.eink.einkClick
import app.kaeri.eink.hairline
import app.kaeri.eink.topRule
import app.kaeri.engine.model.ReadAnilistState
import app.kaeri.engine.model.ReadFilterChoice
import app.kaeri.engine.model.ReadHomeRow
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.engine.model.ReadSourceGroup
import app.kaeri.engine.model.ReadSourceState
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.Strings
import app.kaeri.store.ReadSearchState
import app.kaeri.store.text
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.drop

/**
 * The e-ink look while the panel is e-ink, else `null`: Home then shows one Browse row in place
 * of its discovery rows, and Browse, a row's grid and Search draw on paper.
 */
val LocalEinkLayout = staticCompositionLocalOf<EinkLook?> { null }

/** The gutter of the paper pages. */
internal val Side = 28.dp

/** A cover in the e-ink grid, as the board draws it. */
private val GridCover = 84.dp

/** A cover in the e-ink lists. */
private val ListCover = 40.dp

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
private fun browseRowLine(context: android.content.Context, row: ReadHomeRow): String {
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
            // Loading with no motion: one word, and the list in one refresh when it lands.
            if (view == null || view.rows.any { it.pending }) Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp))
        } else {
            PagedItems(rows, page, { page = it }) { row ->
                BrowseLine(row) { navigator.openReadPage(READ_PAGE_ROW + row.id) }
            }
        }
    }
}

/** A row from Browse: three by three, a page at a time; Search opens the same filter for more. */
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
                ReadPlaces.search(store.reading, query)
                navigator.openReadPage(READ_PAGE_SEARCH)
            }
        },
    ) {
        PagedItems(row.tiles.chunked(3), page, { page = it }, Modifier.padding(top = 8.dp), most = 3) { three ->
            Row(Modifier.fillMaxWidth().padding(bottom = 16.dp)) {
                for (tile in three) Box(Modifier.weight(1f)) { PaperTile(tile) }
                repeat(3 - three.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

/** A cover on paper with its edge, the title and its line under it. */
@Composable
private fun PaperTile(tile: WorkTile) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    Column(
        Modifier
            .padding(end = 12.dp)
            .einkClick { navigator.openSeries(tile.work, "eink-row") }
            .semantics(mergeDescendants = true) {},
    ) {
        ReadCover(tile.cover, GridCover, Modifier.border(EinkSizes.Hairline, EinkColours.Ink), shape = RoundedCornerShape(0.dp), title = tile.title)
        Text(tile.title, style = EinkType.Label.copy(fontSize = EinkType.Secondary.fontSize), maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
        val line = tile.line(context)
        if (line.isNotEmpty()) Text(line, style = EinkType.Secondary, maxLines = 2, overflow = TextOverflow.Ellipsis)
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

// Search on paper

/** One line of e-ink Search's paged list. */
private sealed interface Found {
    data class Anilist(val tile: WorkTile, val joined: String?) : Found
    data class Heading(val text: String) : Found
    data class Source(val name: String) : Found
    data class Hit(val tile: WorkTile) : Found
    data class Asking(val name: String) : Found
    data class Failed(val group: ReadSourceGroup, val reason: String) : Found
    data class Note(val text: String, val ruled: Boolean = false) : Found
    data class AnilistFailed(val query: ReadSearchQuery) : Found
    data class More(val shown: Int, val total: UInt?) : Found
    data class SourceMore(val sourceId: String, val n: Int) : Found
    /** Headings with the line under them, so a page never ends on a heading. */
    data class Together(val lines: List<Found>) : Found
}

/** Each run of headings joined to the line after it, as one item to page. */
private fun keepHeadingsWithNext(lines: List<Found>): List<Found> {
    val out = mutableListOf<Found>()
    val heads = mutableListOf<Found>()
    for (line in lines) {
        if (line is Found.Heading || line is Found.Source) {
            heads += line
        } else if (heads.isNotEmpty()) {
            out += Found.Together(heads + line)
            heads.clear()
        } else {
            out += line
        }
    }
    out += heads
    return out
}

/** The search as lines, in the colour apps' order and states. */
private fun foundLines(context: android.content.Context, search: ReadSearchState, opened: Set<String>): List<Found> = buildList {
    val sources = search.sources
    val query = search.query
    val anilistFailed = !query.sourcesOnly && ((search.failure != null && search.tiles.isEmpty()) || sources?.anilist == ReadAnilistState.FAILED)
    if (anilistFailed) add(Found.AnilistFailed(query))
    if (!query.sourcesOnly) {
        for (tile in search.tiles) {
            val join = sources?.joins?.firstOrNull { it.work == tile.work }
            add(
                Found.Anilist(
                    tile,
                    join?.let { if (it.sourceNames.size == 1) Strings.readSearchInSource(context, it.sourceNames.first()) else Strings.readSearchInSources(context, it.sourceNames.size) },
                ),
            )
        }
        if (search.tiles.isEmpty() && !search.loading && !anilistFailed && query.text.isNotBlank()) {
            add(Found.Note(Strings.readSearchAnilistNothing(context)))
        }
    }
    val groups = sources?.groups.orEmpty()
    val shown = groups.filter { it.state !is ReadSourceState.Nothing && !(it.state is ReadSourceState.Hits && it.tiles.isEmpty()) }
    if (shown.isNotEmpty()) {
        add(Found.Heading(Strings.readSearchFromSources(context)))
        for (group in shown) {
            add(Found.Source(group.sourceName))
            when (val state = group.state) {
                is ReadSourceState.Asking -> add(Found.Asking(group.sourceName))
                is ReadSourceState.Failed -> add(Found.Failed(group, failureWords(context, state.failure)))
                is ReadSourceState.TimedOut -> add(Found.Failed(group, Strings.readSearchSourceTimeout(context, state.seconds.toInt())))
                else -> {
                    // Five hits, then "Show N more" until it's pressed, as in the colour apps.
                    val capped = group.sourceId !in opened && group.tiles.size > SOURCE_HITS_SHOWN
                    (if (capped) group.tiles.take(SOURCE_HITS_SHOWN) else group.tiles).forEach { add(Found.Hit(it)) }
                    if (capped) add(Found.SourceMore(group.sourceId, group.tiles.size - SOURCE_HITS_SHOWN))
                }
            }
        }
    }
    val nothing = groups.filter { it.state is ReadSourceState.Nothing }
    if (nothing.isNotEmpty()) add(Found.Note(Strings.readSearchSourceNothing(context, joinedNames(context, nothing.map { it.sourceName })), ruled = true))
    if (!query.sourcesOnly && search.hasMore) add(Found.More(search.tiles.size, search.total))
}.let(::keepHeadingsWithNext)

/**
 * E-ink Search: the field, Your sources only as a check box beside Filters, and a paged list:
 * AniList's results, then From your sources with each source's hits under its name. No spinner:
 * a slow source reads Asking until it answers, and the page redraws once.
 */
@OptIn(FlowPreview::class)
@Composable
internal fun EinkSearchPage(
    /** The Search tab itself on an e-ink panel, rather than a page pushed over Home. */
    tab: Boolean = false,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val search by store.reading.search.collectAsStateWithLifecycle()
    val text = rememberTextFieldState(search.query.text)
    var page by rememberSaveable { mutableIntStateOf(0) }
    var filters by remember { mutableStateOf(false) }
    var opened by remember(search.query) { mutableStateOf(emptySet<String>()) }
    LaunchedEffect(text) {
        snapshotFlow { text.text.toString() }.drop(1).debounce(TYPING_REST_EINK).collect { typed ->
            val now = store.reading.search.value.query
            if (typed.trim() != now.text) store.reading.runSearch(now.copy(text = typed.trim()))
        }
    }
    LaunchedEffect(search.query.text) {
        if (search.query.text != text.text.toString().trim()) text.setTextAndPlaceCursorAtEnd(search.query.text)
    }
    // A new search starts at its first page.
    LaunchedEffect(search.query) { page = 0 }
    PaperPage(back = if (tab) null else Strings.einkBack(context), title = Strings.navSearch(context)) {
        BasicTextField(
            state = text,
            textStyle = EinkType.Body,
            cursorBrush = SolidColor(EinkColours.Ink),
            lineLimits = androidx.compose.foundation.text.input.TextFieldLineLimits.SingleLine,
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 52.dp)
                .border(EinkSizes.Rule, EinkColours.Ink, RoundedCornerShape(EinkSizes.SegmentRadius))
                .padding(horizontal = 16.dp, vertical = 14.dp),
            decorator = { field ->
                Box {
                    if (text.text.isEmpty()) Text(Strings.readSearchPlaceholder(context), style = EinkType.Tertiary)
                    field()
                }
            },
        )
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            PaperCheck(Strings.readSearchSourcesOnly(context), search.query.sourcesOnly, Modifier.weight(1f)) {
                store.reading.runSearch(search.query.copy(sourcesOnly = !search.query.sourcesOnly))
            }
            EinkTextButton(Strings.readFilterTitle(context), { filters = true }, underline = true)
        }
        val lines = foundLines(context, search, opened)
        when {
            search.query.text.isBlank() && search.tiles.isEmpty() && search.loading ->
                Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp))
            lines.isEmpty() && search.loading ->
                Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp))
            lines.isEmpty() -> Column(Modifier.padding(top = 16.dp)) {
                Text(Strings.readSearchNothing(context, search.query.text), style = EinkType.Body)
                Text(Strings.readSearchNothingLine(context), style = EinkType.Secondary)
            }
            else -> PagedItems(lines, page, { page = it }, Modifier.padding(top = 8.dp)) { line -> FoundLine(line) { opened = opened + it } }
        }
    }
    if (filters) PaperFilters(search.query) { filters = false }
}

/** How long typing rests before e-ink Search asks: a little longer, as each refresh costs. */
private const val TYPING_REST_EINK = 600L

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun FoundLine(line: Found, open: (String) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    when (line) {
        is Found.Anilist -> PaperResult(line.tile, line.joined)
        is Found.Hit -> PaperResult(line.tile, null, hit = true)
        is Found.Heading -> Text(line.text, style = EinkType.RowTitle, modifier = Modifier.padding(top = 16.dp, bottom = 4.dp).semantics { heading() })
        is Found.Source -> Text(line.name, style = EinkType.Label.copy(color = EinkColours.Secondary), modifier = Modifier.padding(top = 10.dp).semantics { heading() })
        is Found.Asking -> Text(Strings.readSearchAsking(context, line.name), style = EinkType.Secondary, modifier = Modifier.padding(vertical = 8.dp))
        is Found.Failed -> PaperFailure(
            Strings.readSearchSourceFailed(context, line.group.sourceName),
            line.reason,
            Strings.actionTryAgain(context),
        ) { store.reading.retrySource(line.group.sourceId) }
        is Found.AnilistFailed -> PaperFailure(
            Strings.readSearchAnilistFailed(context),
            Strings.readSearchAnilistFailedLine(context),
            Strings.actionTryAgain(context),
        ) { store.reading.runSearch(line.query) }
        is Found.Note -> Column(Modifier.padding(top = 8.dp)) {
            if (line.ruled) Box(Modifier.fillMaxWidth().height(EinkSizes.Hairline).background(EinkColours.Tertiary))
            Text(line.text, style = EinkType.Secondary, modifier = Modifier.padding(vertical = 8.dp))
        }
        // Wraps under the button at large text rather than squeezing the count.
        is Found.More -> androidx.compose.foundation.layout.FlowRow(itemVerticalAlignment = Alignment.CenterVertically) {
            EinkTextButton(Strings.readSearchMoreAnilist(context), { store.reading.searchMore() }, underline = true)
            // "30 of 212 shown" once the count is exact, "30 shown" until then.
            Text(
                if (line.total != null) Strings.readSearchShown(context, line.shown, line.total.toInt()) else Strings.readSearchShownSome(context, line.shown),
                style = EinkType.Secondary,
                modifier = Modifier.padding(horizontal = 12.dp),
            )
        }
        is Found.SourceMore -> EinkTextButton(Strings.readSearchSourceMore(context, line.n), { open(line.sourceId) }, underline = true)
        is Found.Together -> Column { line.lines.forEach { FoundLine(it, open) } }
    }
}

/** A result on paper: a small cover, the title, its facts, "In Home library" under them, a chevron. */
@Composable
private fun PaperResult(tile: WorkTile, joined: String?, hit: Boolean = false) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = EinkSizes.Row)
            .einkClick { navigator.openSeries(tile.work, "eink-search") }
            .padding(vertical = 6.dp)
            .semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ReadCover(tile.cover, ListCover, Modifier.border(EinkSizes.Hairline, EinkColours.Ink), shape = RoundedCornerShape(0.dp), title = tile.title)
        Column(Modifier.weight(1f).padding(start = 16.dp)) {
            Text(tile.title, style = EinkType.Label, maxLines = 2, overflow = TextOverflow.Ellipsis)
            // AniList's "Webtoon · Korea"; a source's hit says how much it holds, or nothing.
            val facts = if (hit) tile.hitLine(context) else tile.anilistLine(context)
            if (facts.isNotEmpty()) Text(facts, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (joined != null) Text(joined, style = EinkType.Label.copy(fontSize = EinkType.Secondary.fontSize))
        }
        Text("›", style = EinkType.Button, modifier = Modifier.padding(start = 12.dp)) // i18n-skip
    }
}

/** A check box on paper: a 24 dp square with a 2 dp border, ticked in ink. */
@Composable
private fun PaperCheck(label: String, checked: Boolean, modifier: Modifier = Modifier, onToggle: () -> Unit) {
    Row(
        modifier
            .heightIn(min = 48.dp)
            .einkClick(role = Role.Checkbox, onClick = onToggle)
            .semantics(mergeDescendants = true) { toggleableState = ToggleableState(checked) },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier.size(24.dp).border(EinkSizes.Rule, EinkColours.Ink, RoundedCornerShape(3.dp)).background(if (checked) EinkColours.Ink else EinkColours.Paper),
            contentAlignment = Alignment.Center,
        ) {
            if (checked) Icon(Icons.Check, contentDescription = null, tint = EinkColours.Paper, modifier = Modifier.size(18.dp))
        }
        Text(label, style = EinkType.Body, modifier = Modifier.padding(start = 12.dp))
    }
}

/** One choice in the filters page: its words, ticked when chosen. */
private data class FilterLine(val heading: String?, val choice: String?, val chosen: Boolean, val apply: (ReadSearchQuery) -> ReadSearchQuery)

/**
 * Filters on paper: a whole page over Search with each filter's choices as rows, ticked when set,
 * paged as the lists are, and Done.
 */
@Composable
private fun PaperFilters(query: ReadSearchQuery, onDone: () -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val filters = store.reading.filters
    var page by remember { mutableIntStateOf(0) }
    var draft by remember { mutableStateOf(query) }
    fun group(name: String, choices: List<ReadFilterChoice>, chosen: (ReadSearchQuery, String) -> Boolean, toggle: (ReadSearchQuery, String) -> ReadSearchQuery): List<FilterLine> =
        listOf(FilterLine(name, null, false) { it }) + choices.map { c -> FilterLine(null, c.label.text(context), chosen(draft, c.key)) { q -> toggle(q, c.key) } }
    val lines = buildList {
        addAll(group(Strings.readFilterFormat(context), filters.formats, { q, k -> k in q.formats }) { q, k -> q.copy(formats = if (k in q.formats) q.formats - k else q.formats + k) })
        addAll(group(Strings.readFilterOrigin(context), filters.origins, { q, k -> k in q.origins }) { q, k -> q.copy(origins = if (k in q.origins) q.origins - k else q.origins + k) })
        addAll(group(Strings.readFilterStatus(context), filters.statuses, { q, k -> q.status == k }) { q, k -> q.copy(status = if (q.status == k) null else k) })
        addAll(group(Strings.readSearchStarted(context), filters.started.filter { it.key != STARTED_ANY }, { q, k -> q.started == k }) { q, k -> q.copy(started = if (q.started == k) null else k) })
        add(FilterLine(Strings.readFilterGenres(context), null, false) { it })
        for (genre in filters.genres) add(FilterLine(null, genre, genre in draft.genres) { q -> q.copy(genres = if (genre in q.genres) q.genres - genre else q.genres + genre) })
    }
    Dialog(onDismissRequest = onDone, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        EinkTheme(LocalEinkLayout.current ?: EinkLook()) {
            Column(Modifier.fillMaxSize().background(EinkColours.Paper).padding(horizontal = Side, vertical = 16.dp)) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text(Strings.readFilterTitle(context), style = EinkType.PageTitle, modifier = Modifier.weight(1f).semantics { heading() })
                    EinkSolidButton(Strings.actionDone(context), {
                        if (draft != query) store.reading.runSearch(draft)
                        onDone()
                    })
                }
                Spacer(Modifier.height(12.dp).fillMaxWidth().topRule())
                PagedItems(lines, page, { page = it }) { line ->
                    val heading = line.heading
                    if (heading != null) {
                        Text(heading, style = EinkType.RowTitle, modifier = Modifier.padding(top = 14.dp, bottom = 4.dp).semantics { heading() })
                    } else {
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = 52.dp)
                                .einkClick(role = Role.Checkbox) { draft = line.apply(draft) }
                                .hairline()
                                .semantics(mergeDescendants = true) { selected = line.chosen },
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(line.choice.orEmpty(), style = EinkType.Body.copy(fontWeight = if (line.chosen) androidx.compose.ui.text.font.FontWeight.Bold else androidx.compose.ui.text.font.FontWeight.Medium), modifier = Modifier.weight(1f))
                            if (line.chosen) Icon(Icons.Check, contentDescription = null, tint = EinkColours.Ink, modifier = Modifier.size(22.dp))
                        }
                    }
                }
            }
        }
    }
}
