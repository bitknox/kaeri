// Search's reading side, built from the shows' Search so the two scopes look and move alike: the
// same top (the Shows or Reading control with Both, the field, the row of ReadSearchRow.kt), then
// with nothing asked the start (Home's discovery rows as rails with See all, then the genres), and
// once something is asked the shows' grid under its caption line. Words show AniList's covers and
// a row per source (ReadSourceResults.kt); with Both the last row counts the matches in Shows.
// Reading has no recent searches and no Surprise me.

package app.kaeri.ui.reading

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.saveable.rememberSaveable
import app.kaeri.design.Motion
import app.kaeri.design.SwapFade
import app.kaeri.engine.model.ReadHomeRow
import app.kaeri.ui.PosterGrid
import app.kaeri.ui.RAIL_COVER
import app.kaeri.ui.Rail
import app.kaeri.ui.RailPlaceholder
import app.kaeri.ui.RailPlaceholderRow
import app.kaeri.ui.RemovableFilters
import app.kaeri.ui.SearchFieldPlace
import app.kaeri.ui.SearchNothing
import app.kaeri.ui.SearchTop
import app.kaeri.ui.StartGenres
import app.kaeri.ui.SurpriseButton
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SearchBarDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberSearchBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.rememberStillFade
import app.kaeri.design.stillFade
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.ReadFilterChoice
import app.kaeri.engine.model.ReadFilters
import app.kaeri.engine.model.ReadRowKind
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.SurprisePool
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.ReadSearchState
import app.kaeri.store.ReadingModel
import app.kaeri.store.text
import app.kaeri.ui.Choice
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.MenuChip
import app.kaeri.ui.SearchChip
import app.kaeri.ui.kaeriIsFor
import app.kaeri.ui.wideWindow
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.map

/** The Started filter's choice that sets nothing. */
internal const val STARTED_ANY = "any" // i18n-skip

/** How long typing rests before the words search. */
private const val TYPING_REST = 350L

@OptIn(ExperimentalMaterial3Api::class, FlowPreview::class)
@Composable
internal fun ReadSearchScreen(startText: String? = null) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val search by store.reading.search.collectAsStateWithLifecycle()
    val demo by store.demo.collectAsStateWithLifecycle()
    val failures by store.reading.failures.collectAsStateWithLifecycle()
    val home by store.reading.home.collectAsStateWithLifecycle()
    val filters = store.reading.filters
    val both = kaeriIsFor() == KaeriIsFor.BOTH
    val phone = !wideWindow()
    val text = rememberTextFieldState(search.query.text)
    val bar = rememberSearchBarState()
    var filtersShown by rememberSaveable { mutableStateOf(false) }
    // The start's row whose See all is on show, by its id: the line over the results keeps its words.
    var seeAll by rememberSaveable { mutableStateOf<String?>(null) }
    fun go(query: ReadSearchQuery) = if (query.isStart()) store.reading.searchStart() else store.reading.runSearch(query)
    LaunchedEffect(Unit) {
        // The words the app was opened with, or what was asked before; with nothing asked, the start.
        val now = store.reading.search.value
        if (now.tiles.isEmpty() && !now.loading) {
            val asked = startText?.let { now.query.copy(text = it) } ?: now.query
            if (!asked.isStart()) store.reading.runSearch(asked)
        }
        // The start's rows are Home's discovery rows.
        if (store.reading.home.value == null) store.reading.loadHome()
    }
    // The words search once the typing rests.
    LaunchedEffect(text) {
        snapshotFlow { text.text.toString() }.drop(1).debounce(TYPING_REST).collect { typed ->
            val now = store.reading.search.value.query
            if (typed.trim() != now.text) go(now.copy(text = typed.trim()))
        }
    }
    // Words from elsewhere, a link from Home or Clear filters, take the field.
    LaunchedEffect(search.query.text) {
        if (search.query.text != text.text.toString().trim()) text.setTextAndPlaceCursorAtEnd(search.query.text)
    }
    // A new query starts the grid at its top and holds it there while the answer lands, rather
    // than following an item it kept down the new list; a drag lets go. Loading more of the same
    // query doesn't change the query, so it keeps its place.
    val gridState = rememberLazyGridState()
    val startGrid = rememberLazyGridState()
    val rowScroll = rememberLazyListState()
    var holdTop by remember { mutableStateOf(false) }
    LaunchedEffect(gridState) {
        store.reading.search.map { it.query }.distinctUntilChanged().drop(1).collect { holdTop = true }
    }
    LaunchedEffect(gridState) {
        gridState.interactionSource.interactions.collect { if (it is DragInteraction.Start) holdTop = false }
    }
    if (holdTop) SideEffect { gridState.requestScrollToItem(0) }
    // New words start the row at its first pill, as the shows' row does.
    LaunchedEffect(rowScroll) {
        store.reading.search.map { it.query.text }.distinctUntilChanged().drop(1).collect { rowScroll.scrollToItem(0) }
    }
    val query = search.query
    val browsing = query.text.isBlank()
    val clearLabel = stringResource(R.string.search_clear)
    val gridFade = rememberStillFade()
    val held = rememberShownSearch(search, gridFade)
    // Browsing that failed while away from Search says so, from the failures the store keeps.
    val shown = held.search.let { drawn ->
        val failure = drawn.failure ?: failures[ReadingAbout.SEARCH]?.takeIf { !drawn.loading }
        if (failure == drawn.failure) held else held.copy(search = drawn.copy(failure = failure))
    }
    // The start, while nothing is asked; it stays while a first search is under 600 ms away, then
    // gives way to the results or their placeholders in one crossfade, as the shows' start does.
    val startShown = query.isStart() || (search.loading && search.tiles.isEmpty() && search.failure == null && !held.waited)
    val seeAllRow = home?.rows?.firstOrNull { it.id == seeAll }
    LaunchedEffect(query) { if (seeAllRow != null && query != seeAllRow.viewAll) seeAll = null }
    val scrolledUnder = phone && (if (startShown) startGrid else gridState).canScrollBackward
    Column {
        SearchTop(
            phone, scrolledUnder, both, reading = true,
            field = {
                Surface(shape = SearchBarDefaults.inputFieldShape, color = MaterialTheme.colorScheme.surfaceContainerHigh, modifier = SearchFieldPlace) {
                    SearchBarDefaults.InputField(
                        textFieldState = text,
                        searchBarState = bar,
                        onSearch = { go(store.reading.search.value.query.copy(text = it.trim())) },
                        placeholder = { Text(Strings.readSearchPlaceholder(context)) },
                        leadingIcon = { Icon(Icons.Search, contentDescription = null) },
                        trailingIcon = {
                            if (text.text.isNotEmpty()) {
                                IconButton(onClick = { text.clearText() }, modifier = Modifier.semantics { contentDescription = clearLabel }) {
                                    Icon(Icons.Clear, contentDescription = null)
                                }
                            } else if (browsing) {
                                // Surprise me, from reading's filters, in the shows' place.
                                SurpriseButton(SurprisePool.Reading(query))
                            }
                        },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            },
            row = { ReadSearchRow(query, filters, phone, rowScroll, onFilters = { filtersShown = true }, go = ::go) },
        )
        // The one announcement once every part answered, outside the grid so it is always there.
        if (!browsing) SearchSaid(search)
        Crossfade(startShown, Modifier.weight(1f), animationSpec = Motion.standard(), label = "read search") { start -> // i18n-skip
            if (start) {
                ReadSearchStart(
                    startGrid,
                    home?.rows,
                    filters.genres,
                    onSeeAll = { row ->
                        row.viewAll?.let { asked ->
                            seeAll = row.id
                            store.reading.runSearch(asked)
                        }
                    },
                    onGenre = { genre -> go(ReadingModel.EMPTY_QUERY.copy(genres = listOf(genre))) },
                )
                return@Crossfade
            }
            ReadGrid(PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp), Modifier.stillFade(gridFade), gridState, search = true) { grid ->
                // A wide window says the filters set over the results, each with its cross, as the shows' does.
                if (!phone) {
                    val active = activeFilters(context, query, filters)
                    if (active.isNotEmpty()) {
                        item(span = { GridItemSpan(maxLineSpan) }, key = "active", contentType = "active") {
                            RemovableFilters(
                                active.map { (label, remove) -> label to { go(remove(store.reading.search.value.query)) } },
                                onClearAll = { go(store.reading.search.value.query.clearedFilters()) },
                            )
                        }
                    }
                }
                // What shows follows the held answer: words' results stay until browsing's land, and back.
                if (shown.search.query.text.isNotBlank()) {
                    // With words: AniList's grid, then the sources' rows (ReadSourceResults.kt).
                    wordsResults(shown, both, demo.onlyBooks, grid)
                } else {
                    browseCells(
                        shown,
                        grid,
                        viewAll = false,
                        lineFor = { viewAllLine(it, seeAllRow, shown.search, filters) },
                        nothing = {
                            Column {
                                val asked = shown.search.query
                                SearchNothing(
                                    Strings.readSearchNothingBrowse(context),
                                    onClearFilters = if (anyFilter(asked)) ({ go(asked.clearedFilters()) }) else null,
                                )
                                if (both) OtherScopeRow(shown.search)
                            }
                        },
                    )
                }
            }
        }
    }
    if (filtersShown) ReadFiltersSheet(onDismiss = { filtersShown = false }, go = ::go)
}

/**
 * Search's start with nothing asked: Home's discovery rows as the shows' start has its rails, each
 * with See all, then the genres. Rows on their way hold placeholders as the shows' rails do.
 */
@Composable
private fun ReadSearchStart(
    state: LazyGridState,
    rows: List<ReadHomeRow>?,
    genres: List<String>,
    onSeeAll: (ReadHomeRow) -> Unit,
    onGenre: (String) -> Unit,
) {
    val context = LocalContext.current
    PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp), state) {
        if (rows == null) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "rail-placeholder") { RailPlaceholder() }
        } else {
            // Catalogue-wide, as the shows' start: Because you finished stays on Home.
            for (row in rows.filter { it.kind != ReadRowKind.BECAUSE && (it.tiles.isNotEmpty() || it.pending) }) {
                item(span = { GridItemSpan(maxLineSpan) }, key = "rail-${row.id}") {
                    val title = row.title.text(context)
                    val side = row.side?.text(context)
                    SwapFade(row.tiles.isEmpty(), animationSpec = Motion.standard(), label = "start row") { waiting -> // i18n-skip
                        if (waiting) {
                            RailPlaceholderRow(title, side)
                        } else {
                            Rail(title, side, onSeeAll = row.viewAll?.let { { onSeeAll(row) } }) {
                                val oneFormat = row.format != null
                                items(row.tiles, key = { "${row.id}-${it.work}" }) { tile ->
                                    val caption = tile.rowCaption(context, row.kind, oneFormat)
                                    var menu by remember { mutableStateOf(false) }
                                    Box {
                                        ReadTile(
                                            tile, RAIL_COVER, from = "read-search-${row.id}", lines = 1, reserveLine = true, actions = readTileActions(tile, current = null),
                                            caption = caption.shown, spoken = caption.spoken, onLongClick = { menu = true }, titleMax = 2,
                                        )
                                        ReadTileMenu(tile, menu, current = null) { menu = false }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if (genres.isNotEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "genres") { StartGenres(genres, onGenre) }
        }
    }
}

/**
 * Search on reading's side, for the fixtures' shots: [words] typed, and with [filters] the row's
 * filters set (Fantasy, Manga, Japan, Releasing), which the fixtures' books answer.
 */
fun showReadingSearch(store: app.kaeri.store.AppStore, words: String?, filters: Boolean) {
    var query = ReadingModel.EMPTY_QUERY.copy(text = words.orEmpty())
    if (filters) {
        query = query.copy(genres = listOf("Fantasy"), formats = listOf("manga", "one-shot"), origins = listOf("JP"), status = "releasing") // i18n-skip
    }
    if (query == ReadingModel.EMPTY_QUERY) ReadPlaces.searchReading = true else ReadPlaces.search(store.reading, query)
}

/** "2 matches in Shows", and Show them, which switches the scope. */
@Composable
internal fun OtherScopeRow(search: ReadSearchState) {
    val context = LocalContext.current
    val n = search.otherScope?.toInt() ?: return
    if (n <= 0) return
    Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(
            Strings.searchOtherScope(context, n, Strings.libraryScopeShows(context)),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.weight(1f),
        )
        TextButton(
            onClick = { ReadPlaces.searchReading = false },
            colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
        ) {
            Text(Strings.searchShowThem(context))
            Icon(Icons.Chevron, contentDescription = null, modifier = Modifier.padding(start = 4.dp).size(18.dp))
        }
    }
}

// Filters

/**
 * Format, origin, status and genres as chips that open menus, then the order while browsing. A set
 * chip says its value and fills with the neutral colour; nothing here is amber.
 */
@Composable
internal fun ReadFilterRow(
    query: ReadSearchQuery,
    filters: ReadFilters,
    browsing: Boolean,
    /**
     * View all's chips: the filters set come first, each saying its value alone and checked, then
     * the order, set too, then the filters left to set.
     */
    viewAll: Boolean = false,
    /** Run the changed query; Search's own by default. */
    run: ((ReadSearchQuery) -> Unit)? = null,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val phone = !wideWindow()
    fun change(edit: (ReadSearchQuery) -> ReadSearchQuery) {
        if (run != null) run(edit(query)) else store.reading.runSearch(edit(store.reading.search.value.query))
    }
    // On View all a set chip says its value alone: "Manga", "Last 12 months".
    fun chipWords(context: android.content.Context, name: String, value: String?): String =
        if (viewAll && value != null) value else app.kaeri.ui.reading.chipWords(context, name, value)
    val row = rememberLazyListState()
    // Your sources only joins at the start once there are words: the row goes back to it rather
    // than keeping Format first.
    val sourcesChip = !browsing && !viewAll
    androidx.compose.runtime.LaunchedEffect(sourcesChip) { if (sourcesChip) row.scrollToItem(0) }
    // AniList's filters say nothing to the sources: dimmed while only they are asked.
    val on = !query.sourcesOnly || browsing
    val formatValue = formatsValue(context, query.formats, filters.formats)
    val originValue = value(context, query.origins, filters.origins)
    val statusValue = query.status?.let { value(context, listOf(it), filters.statuses) }
    val genresValue = genresValue(context, query)
    val startedValue = query.started?.takeIf { it != STARTED_ANY }?.let { value(context, listOf(it), filters.started) }
    val format: @Composable () -> Unit = {
        MenuChip(chipWords(context, Strings.readFilterFormat(context), formatValue), selected = formatValue != null, enabled = on, phone = phone) { _ ->
            // One-shots are part of Manga, so the menu has no choice of their own.
            for (choice in filters.formats.filter { it.key != FORMAT_ONE_SHOT }) {
                Choice(choice.label.text(context), choice.key in query.formats) { change { it.copy(formats = it.formats.togglingFormat(choice.key)) } }
            }
        }
    }
    val origin: @Composable () -> Unit = {
        MenuChip(chipWords(context, Strings.readFilterOrigin(context), originValue), selected = originValue != null, enabled = on, phone = phone) { _ ->
            for (choice in filters.origins) {
                Choice(choice.label.text(context), choice.key in query.origins) { change { it.copy(origins = it.origins.toggling(choice.key)) } }
            }
        }
    }
    val status: @Composable () -> Unit = {
        MenuChip(chipWords(context, Strings.readFilterStatus(context), statusValue), selected = statusValue != null, enabled = on, phone = phone) { close ->
            Choice(Strings.readFilterAny(context), query.status == null) { close(); change { it.copy(status = null) } }
            for (choice in filters.statuses) {
                Choice(choice.label.text(context), query.status == choice.key) { close(); change { it.copy(status = choice.key) } }
            }
        }
    }
    val genres: @Composable () -> Unit = {
        MenuChip(chipWords(context, Strings.readFilterGenres(context), genresValue), selected = genresValue != null, enabled = on, phone = phone) { _ ->
            // Each genre cycles Any, Included, Left out; the menu stays open for the next.
            for (genre in filters.genres) {
                val state = when (genre) {
                    in query.genres -> Strings.readFilterIncluded(context)
                    in query.excludeGenres -> Strings.readFilterLeftOut(context)
                    else -> null
                }
                DropdownMenuItem(
                    text = { Text(genre) },
                    trailingIcon = {
                        Text(
                            state ?: Strings.readFilterAny(context),
                            style = MaterialTheme.typography.labelLarge,
                            color = if (genre in query.genres) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    },
                    onClick = { change { it.cyclingGenre(genre) } },
                )
            }
        }
    }
    val started: @Composable () -> Unit = {
        MenuChip(chipWords(context, Strings.readSearchStarted(context), startedValue), selected = startedValue != null, enabled = on, phone = phone) { close ->
            for (choice in filters.started) {
                val chosen = (query.started ?: STARTED_ANY) == choice.key
                Choice(choice.label.text(context), chosen) { close(); change { it.copy(started = choice.key.takeIf { key -> key != STARTED_ANY }) } }
            }
        }
    }
    val sort: @Composable () -> Unit = {
        val current = query.sort ?: filters.sorts.firstOrNull()
        // On View all the row's order is part of what it asks, so it shows as set.
        MenuChip(sortWords(context, current), selected = viewAll, phone = phone) { close ->
            for (sort in filters.sorts) {
                Choice(sortWords(context, sort), sort == current) { close(); change { it.copy(sort = sort) } }
            }
        }
    }
    val chips = listOf(
        Triple("format", formatValue != null, format),
        Triple("origin", originValue != null, origin),
        Triple("status", statusValue != null, status),
        Triple("genres", genresValue != null, genres),
        Triple("started", startedValue != null, started),
    )
    // View all: set first, the order, then the rest; Search keeps one order, the sort at the end.
    val ordered = if (viewAll) {
        chips.filter { it.second } + Triple("sort", true, sort) + chips.filterNot { it.second }
    } else {
        chips + listOfNotNull(Triple("sort", false, sort).takeIf { browsing })
    }
    LazyRow(
        state = row,
        contentPadding = PaddingValues(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.padding(top = 4.dp),
    ) {
        // Your sources only comes first, so it is never past the edge; it needs words to ask by.
        if (sourcesChip) {
            item(key = "sources-only") {
                SearchChip(
                    Strings.readSearchSourcesOnly(context),
                    selected = query.sourcesOnly,
                    phone = phone,
                ) { change { it.copy(sourcesOnly = !it.sourcesOnly) } }
            }
        }
        for ((key, _, chip) in ordered) item(key = key) { Box(Modifier.animateItem()) { chip() } }
    }
}

/** A filter's chip words: its name, or once set, "Format: Manga". */
internal fun chipWords(context: android.content.Context, name: String, value: String?): String =
    if (value == null) name else "${Strings.readFilterSet(context, name)} $value" // i18n-skip

/** The set choices' words: one, or the first and how many more. */
internal fun value(context: android.content.Context, keys: List<String>, choices: List<ReadFilterChoice>): String? {
    val words = keys.mapNotNull { key -> choices.firstOrNull { it.key == key }?.label?.text(context) }
    val first = words.firstOrNull() ?: return null
    return if (words.size == 1) first else Strings.readFilterMany(context, first, words.size - 1)
}

/** The formats set in words. Manga with one-shot is Manga: one-shot counts as part of it. */
internal fun formatsValue(context: android.content.Context, keys: List<String>, choices: List<ReadFilterChoice>): String? {
    val shown = if (FORMAT_MANGA in keys) keys - FORMAT_ONE_SHOT else keys
    return value(context, shown, choices)
}

/** Format keys the chip treats as one: Manga takes one-shot with it. */
internal const val FORMAT_MANGA = "manga" // i18n-skip
internal const val FORMAT_ONE_SHOT = "one-shot" // i18n-skip

/** Turns a format on or off; Manga takes one-shot with it both ways, as one-shots are part of Manga. */
internal fun List<String>.togglingFormat(key: String): List<String> = when {
    key == FORMAT_MANGA && key in this -> this - FORMAT_MANGA - FORMAT_ONE_SHOT
    key == FORMAT_MANGA -> this - FORMAT_ONE_SHOT + FORMAT_MANGA + FORMAT_ONE_SHOT
    else -> toggling(key)
}

private fun genresValue(context: android.content.Context, query: ReadSearchQuery): String? {
    val count = query.genres.size + query.excludeGenres.size
    return when {
        count == 0 -> null
        count == 1 -> query.genres.firstOrNull() ?: Strings.readFilterNotGenre(context, query.excludeGenres.first())
        else -> Strings.readFilterGenresCount(context, count)
    }
}

internal fun List<String>.toggling(key: String): List<String> = if (key in this) this - key else this + key

internal fun ReadSearchQuery.cyclingGenre(genre: String): ReadSearchQuery = when (genre) {
    in genres -> copy(genres = genres - genre, excludeGenres = excludeGenres + genre)
    in excludeGenres -> copy(excludeGenres = excludeGenres - genre)
    else -> copy(genres = genres + genre)
}

internal fun anyFilter(query: ReadSearchQuery): Boolean =
    query.formats.isNotEmpty() || query.origins.isNotEmpty() || query.status != null || query.genres.isNotEmpty() ||
        query.excludeGenres.isNotEmpty() || (query.started != null && query.started != STARTED_ANY)

/** An order in words, as the shows' search words it. */
internal fun sortWords(context: android.content.Context, key: String?): String = when (key) {
    "TRENDING_DESC" -> Strings.readSortTrending(context) // i18n-skip
    "SCORE_DESC" -> Strings.readSortScore(context) // i18n-skip
    "START_DATE_DESC" -> Strings.readSortNewest(context) // i18n-skip
    "TITLE_ROMAJI" -> Strings.librarySortTitle(context) // i18n-skip
    else -> Strings.sortPopularity(context)
}

/**
 * The line over browsing: what is shown and where the list comes from, "Popular · from AniList";
 * once every result is listed, with how many.
 */
internal fun browseLine(context: android.content.Context, search: ReadSearchState, filters: ReadFilters): String {
    val query = search.query
    val parts = buildList {
        // One-shots are said as part of Manga.
        val formats = if (FORMAT_MANGA in query.formats) query.formats - FORMAT_ONE_SHOT else query.formats
        addAll(formats.mapNotNull { key -> filters.formats.firstOrNull { it.key == key }?.label?.text(context) })
        addAll(query.origins.mapNotNull { key -> filters.origins.firstOrNull { it.key == key }?.label?.text(context) })
        query.status?.let { key -> filters.statuses.firstOrNull { it.key == key }?.label?.text(context)?.let(::add) }
        addAll(query.genres)
        query.started?.takeIf { it != STARTED_ANY }?.let { key -> filters.started.firstOrNull { it.key == key }?.label?.text(context)?.let(::add) }
    }
    val what = if (parts.isEmpty()) sortWords(context, query.sort ?: filters.sorts.firstOrNull()) else parts.joinToString(", ") // i18n-skip
    val total = search.total?.takeIf { !search.hasMore } ?: return Strings.readSearchBrowseLine(context, what)
    return listOf(what, Strings.readSearchTotal(context, total.toInt())).joinToString(" · ") // i18n-skip
}
