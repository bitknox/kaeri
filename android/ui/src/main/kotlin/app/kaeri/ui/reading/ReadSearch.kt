// Search's reading side: the M3 search bar with the query, then four filters as chips that open
// menus (format, origin, status, genres) and the order; browsing with no words shows covers under
// a line saying what is shown and where the list comes from, words show rows with the list status
// on the right. With Both, the Shows or Reading control comes first, and the last row counts the
// matches in Shows. No results names the query and offers Clear filters.

package app.kaeri.ui.reading

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ShowTitle
import app.kaeri.design.tabular
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.ReadFilterChoice
import app.kaeri.engine.model.ReadFilters
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.ReadSearchState
import app.kaeri.store.ReadingModel
import app.kaeri.store.text
import app.kaeri.ui.AfterWait
import app.kaeri.ui.Choice
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.MenuChip
import app.kaeri.ui.PosterGrid
import app.kaeri.ui.Shared
import app.kaeri.ui.TextPlaceholder
import app.kaeri.ui.itemMotion
import app.kaeri.ui.kaeriIsFor
import app.kaeri.ui.sharedWork
import app.kaeri.ui.wideWindow
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.drop

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
    val filters = store.reading.filters
    val both = kaeriIsFor() == KaeriIsFor.BOTH
    val text = rememberTextFieldState(search.query.text)
    val bar = rememberSearchBarState()
    LaunchedEffect(Unit) {
        // The first search: the words the app was opened with, else browse.
        if (search.tiles.isEmpty() && !search.loading) {
            store.reading.runSearch(startText?.let { search.query.copy(text = it) } ?: search.query)
        }
    }
    // The words search once the typing rests.
    LaunchedEffect(text) {
        snapshotFlow { text.text.toString() }.drop(1).debounce(TYPING_REST).collect { typed ->
            val now = store.reading.search.value.query
            if (typed.trim() != now.text) store.reading.runSearch(now.copy(text = typed.trim()))
        }
    }
    // Words from elsewhere, a link from Home or Clear filters, take the field.
    LaunchedEffect(search.query.text) {
        if (search.query.text != text.text.toString().trim()) text.setTextAndPlaceCursorAtEnd(search.query.text)
    }
    val query = search.query
    val browsing = query.text.isBlank()
    val failure = search.failure ?: failures[ReadingAbout.SEARCH]?.takeIf { !search.loading }
    val clearLabel = stringResource(R.string.search_clear)
    Column {
        Column(Modifier.windowInsetsPadding(WindowInsets.statusBars).padding(top = 8.dp)) {
            if (both) {
                ScopeControl(reading = true, modifier = Modifier.padding(horizontal = 16.dp).padding(bottom = 8.dp)) { ReadPlaces.searchReading = it }
            }
            Surface(
                shape = SearchBarDefaults.inputFieldShape,
                color = MaterialTheme.colorScheme.surfaceContainerHigh,
                modifier = Modifier.padding(horizontal = 16.dp).fillMaxWidth(),
            ) {
                SearchBarDefaults.InputField(
                    textFieldState = text,
                    searchBarState = bar,
                    onSearch = { store.reading.runSearch(store.reading.search.value.query.copy(text = it.trim())) },
                    placeholder = { Text(Strings.readSearchPlaceholder(context)) },
                    leadingIcon = { Icon(Icons.Search, contentDescription = null) },
                    trailingIcon = {
                        if (text.text.isNotEmpty()) {
                            IconButton(onClick = { text.clearText() }, modifier = Modifier.semantics { contentDescription = clearLabel }) {
                                Icon(Icons.Clear, contentDescription = null)
                            }
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            ReadFilterRow(query, filters, browsing)
        }
        PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp)) {
            when {
                failure != null && search.tiles.isEmpty() -> item(span = { GridItemSpan(maxLineSpan) }, key = "failed", contentType = "quiet") {
                    Box(itemMotion()) {
                        PlainEmpty(
                            Strings.readSearchFailed(context),
                            failureWords(context, failure),
                            inset = 0.dp,
                            action = Strings.actionTryAgain(context),
                            onAction = { store.reading.runSearch(query) },
                        )
                    }
                }
                search.tiles.isEmpty() && search.loading -> {
                    if (browsing) {
                        items(9, key = { "cell-$it" }, contentType = { "placeholder" }) { Box(itemMotion()) { AfterWait { CoverPlaceholder() } } }
                    } else {
                        items(6, key = { "row-$it" }, span = { GridItemSpan(maxLineSpan) }, contentType = { "placeholder" }) {
                            Box(itemMotion()) { AfterWait { RowPlaceholder() } }
                        }
                    }
                }
                search.tiles.isEmpty() -> item(span = { GridItemSpan(maxLineSpan) }, key = "nothing", contentType = "quiet") {
                    Column(itemMotion()) {
                        val filtered = anyFilter(query)
                        PlainEmpty(
                            if (browsing) Strings.readSearchNothingBrowse(context) else Strings.readSearchNothing(context, query.text),
                            if (filtered) Strings.readSearchNothingFiltersLine(context) else Strings.readSearchNothingLine(context),
                            inset = 0.dp,
                            action = if (filtered) Strings.readSearchClearFilters(context) else null,
                            onAction = { store.reading.runSearch(ReadingModel.EMPTY_QUERY.copy(text = query.text, sort = query.sort)) },
                        )
                        if (both) OtherScopeRow(search)
                    }
                }
                browsing -> {
                    item(span = { GridItemSpan(maxLineSpan) }, key = "line", contentType = "line") {
                        Text(
                            browseLine(context, search, filters),
                            style = MaterialTheme.typography.bodySmall.tabular(),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = itemMotion(),
                        )
                    }
                    items(search.tiles, key = { "b-${it.work}" }, contentType = { "cover" }) { tile ->
                        Box(itemMotion()) { SearchCell(tile) }
                    }
                    moreItem(search)
                }
                else -> {
                    items(search.tiles, key = { "r-${it.work}" }, span = { GridItemSpan(maxLineSpan) }, contentType = { "row" }) { tile ->
                        Column(itemMotion()) {
                            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                            ResultRow(tile)
                        }
                    }
                    moreItem(search)
                    // Once every match is listed, how many there are: the count matches the rows.
                    val total = search.total
                    // Not for the demo's works alone: they are no AniList count.
                    if (!search.hasMore && total != null && !search.tiles.all { it.fromDemo }) {
                        item(span = { GridItemSpan(maxLineSpan) }, key = "total", contentType = "line") {
                            Column(itemMotion()) {
                                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                                Text(
                                    Strings.readSearchTotal(context, total.toInt()),
                                    style = MaterialTheme.typography.bodyMedium.tabular(),
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier.padding(vertical = 12.dp),
                                )
                            }
                        }
                    }
                    if (both) {
                        item(span = { GridItemSpan(maxLineSpan) }, key = "other", contentType = "line") { Box(itemMotion()) { OtherScopeRow(search) } }
                    }
                    if (demo.onlyBooks) {
                        item(span = { GridItemSpan(maxLineSpan) }, key = "demo", contentType = "line") { Box(itemMotion()) { DemoSearchNote() } }
                    }
                }
            }
        }
    }
}

/** The next page, asked as its row comes into view, with a small ring while it comes. */
private fun androidx.compose.foundation.lazy.grid.LazyGridScope.moreItem(search: ReadSearchState) {
    if (!search.hasMore) return
    item(span = { GridItemSpan(maxLineSpan) }, key = "more-${search.page}", contentType = "more") {
        val store = LocalAppStore.current
        LaunchedEffect(search.page) { store.reading.searchMore() }
        Box(Modifier.fillMaxWidth().heightIn(min = 48.dp), contentAlignment = Alignment.Center) {
            if (search.loading) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
        }
    }
}

/** "2 matches in Shows", and Show them, which switches the scope. */
@Composable
private fun OtherScopeRow(search: ReadSearchState) {
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

/** A result: the cover, the title and its facts, and the list it is on to the right. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ResultRow(tile: WorkTile) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val haptics = LocalHapticFeedback.current
    val colors = MaterialTheme.colorScheme
    var menu by remember { mutableStateOf(false) }
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 72.dp)
                .sharedWork(Shared.Page, tile.work, "read-search")
                .semantics(mergeDescendants = true) {}
                .combinedClickable(
                    onClickLabel = stringResource(R.string.title_open),
                    onLongClickLabel = stringResource(R.string.action_more),
                    onLongClick = { haptics.performHapticFeedback(HapticFeedbackType.LongPress); menu = true },
                    onClick = { navigator.openSeries(tile.work, "read-search") },
                )
                .padding(vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ReadCover(tile.cover, 40.dp, Modifier.sharedWork(Shared.Cover, tile.work, "read-search"))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(tile.title, style = ShowTitle.row.copy(fontWeight = FontWeight.SemiBold), color = colors.onSurface)
                // The demo's works say Demo, Reads now.
                val facts = if (tile.fromDemo) Strings.searchDemoReads(context) else tile.factsLine(context)
                if (facts.isNotEmpty()) Text(facts, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
            }
            tile.next?.let { Text(it.text(context), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant) }
        }
        ReadTileMenu(tile, menu, current = null) { menu = false }
    }
}

/** A cover while browsing, with its long-press menu. */
@Composable
private fun SearchCell(tile: WorkTile) {
    var menu by remember { mutableStateOf(false) }
    BoxWithConstraints {
        ReadTile(tile, maxWidth, from = "read-search", lines = if (wideWindow()) 1 else 2, onLongClick = { menu = true })
        ReadTileMenu(tile, menu, current = null) { menu = false }
    }
}

// Filters

/**
 * Format, origin, status and genres as chips that open menus, then the order while browsing. A set
 * chip says its value and fills with the neutral colour; nothing here is amber.
 */
@Composable
private fun ReadFilterRow(query: ReadSearchQuery, filters: ReadFilters, browsing: Boolean) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val phone = !wideWindow()
    fun change(edit: (ReadSearchQuery) -> ReadSearchQuery) = store.reading.runSearch(edit(store.reading.search.value.query))
    val row = rememberLazyListState()
    LazyRow(
        state = row,
        contentPadding = PaddingValues(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier.padding(top = 4.dp),
    ) {
        item(key = "format") {
            val value = value(context, query.formats, filters.formats)
            MenuChip(chipWords(context, Strings.readFilterFormat(context), value), selected = value != null, phone = phone) { _ ->
                for (choice in filters.formats) {
                    Choice(choice.label.text(context), choice.key in query.formats) { change { it.copy(formats = it.formats.toggling(choice.key)) } }
                }
            }
        }
        item(key = "origin") {
            val value = value(context, query.origins, filters.origins)
            MenuChip(chipWords(context, Strings.readFilterOrigin(context), value), selected = value != null, phone = phone) { _ ->
                for (choice in filters.origins) {
                    Choice(choice.label.text(context), choice.key in query.origins) { change { it.copy(origins = it.origins.toggling(choice.key)) } }
                }
            }
        }
        item(key = "status") {
            val value = query.status?.let { value(context, listOf(it), filters.statuses) }
            MenuChip(chipWords(context, Strings.readFilterStatus(context), value), selected = value != null, phone = phone) { close ->
                Choice(Strings.readFilterAny(context), query.status == null) { close(); change { it.copy(status = null) } }
                for (choice in filters.statuses) {
                    Choice(choice.label.text(context), query.status == choice.key) { close(); change { it.copy(status = choice.key) } }
                }
            }
        }
        item(key = "genres") {
            val value = genresValue(context, query)
            MenuChip(chipWords(context, Strings.readFilterGenres(context), value), selected = value != null, phone = phone) { _ ->
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
        if (browsing) {
            item(key = "sort") {
                val current = query.sort ?: filters.sorts.firstOrNull()
                MenuChip(sortWords(context, current), selected = false, phone = phone) { close ->
                    for (sort in filters.sorts) {
                        Choice(sortWords(context, sort), sort == current) { close(); change { it.copy(sort = sort) } }
                    }
                }
            }
        }
    }
}

/** A filter's chip words: its name, or once set, "Format: Manga". */
private fun chipWords(context: android.content.Context, name: String, value: String?): String =
    if (value == null) name else "${Strings.readFilterSet(context, name)} $value" // i18n-skip

/** The set choices' words: one, or the first and how many more. */
private fun value(context: android.content.Context, keys: List<String>, choices: List<ReadFilterChoice>): String? {
    val words = keys.mapNotNull { key -> choices.firstOrNull { it.key == key }?.label?.text(context) }
    val first = words.firstOrNull() ?: return null
    return if (words.size == 1) first else Strings.readFilterMany(context, first, words.size - 1)
}

private fun genresValue(context: android.content.Context, query: ReadSearchQuery): String? {
    val count = query.genres.size + query.excludeGenres.size
    return when {
        count == 0 -> null
        count == 1 -> query.genres.firstOrNull() ?: Strings.readFilterNotGenre(context, query.excludeGenres.first())
        else -> Strings.readFilterGenresCount(context, count)
    }
}

private fun List<String>.toggling(key: String): List<String> = if (key in this) this - key else this + key

private fun ReadSearchQuery.cyclingGenre(genre: String): ReadSearchQuery = when (genre) {
    in genres -> copy(genres = genres - genre, excludeGenres = excludeGenres + genre)
    in excludeGenres -> copy(excludeGenres = excludeGenres - genre)
    else -> copy(genres = genres + genre)
}

private fun anyFilter(query: ReadSearchQuery): Boolean =
    query.formats.isNotEmpty() || query.origins.isNotEmpty() || query.status != null || query.genres.isNotEmpty() || query.excludeGenres.isNotEmpty()

/** An order in words, as the shows' search words it. */
private fun sortWords(context: android.content.Context, key: String?): String = when (key) {
    "TRENDING_DESC" -> Strings.readSortTrending(context) // i18n-skip
    "SCORE_DESC" -> Strings.readSortScore(context) // i18n-skip
    "START_DATE_DESC" -> Strings.readSortNewest(context) // i18n-skip
    "TITLE_ROMAJI" -> Strings.librarySortTitle(context) // i18n-skip
    else -> Strings.readSortPopular(context)
}

/**
 * The line over browsing: what is shown and where the list comes from, "Popular · from AniList";
 * once every result is listed, with how many.
 */
private fun browseLine(context: android.content.Context, search: ReadSearchState, filters: ReadFilters): String {
    val query = search.query
    val parts = buildList {
        addAll(query.formats.mapNotNull { key -> filters.formats.firstOrNull { it.key == key }?.label?.text(context) })
        addAll(query.origins.mapNotNull { key -> filters.origins.firstOrNull { it.key == key }?.label?.text(context) })
        query.status?.let { key -> filters.statuses.firstOrNull { it.key == key }?.label?.text(context)?.let(::add) }
        addAll(query.genres)
    }
    val what = if (parts.isEmpty()) sortWords(context, query.sort ?: filters.sorts.firstOrNull()) else parts.joinToString(", ") // i18n-skip
    val total = search.total?.takeIf { !search.hasMore } ?: return Strings.readSearchBrowseLine(context, what)
    return listOf(what, Strings.readSearchTotal(context, total.toInt())).joinToString(" · ") // i18n-skip
}

@Composable
private fun RowPlaceholder() {
    Row(Modifier.padding(vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.width(40.dp)) { TextPlaceholder(1f, 60.dp) }
        Column(Modifier.weight(1f)) {
            TextPlaceholder(0.6f, 22.dp)
            TextPlaceholder(0.4f, 20.dp)
        }
    }
}

@Composable
private fun CoverPlaceholder() {
    Column {
        app.kaeri.ui.PosterPlaceholder()
        Spacer(Modifier.size(4.dp))
    }
}
