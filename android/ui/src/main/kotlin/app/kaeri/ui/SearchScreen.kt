// Search: what is typed and every filter as one query, the titles found as posters, and before
// anything is asked, the recent searches and rails of titles. The engine searches as the query
// changes, words every filter, and keeps the recent searches.

package app.kaeri.ui

import app.kaeri.store.plainReason
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.expandHorizontally
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.core.snap
import androidx.compose.animation.shrinkHorizontally
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.BottomSheetDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExpandedFullScreenSearchBar
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.RangeSlider
import androidx.compose.material3.SearchBar
import androidx.compose.material3.SearchBarDefaults
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberBottomSheetState
import androidx.compose.material3.rememberSearchBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.sp
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import app.kaeri.design.Motion
import app.kaeri.design.tabular
import app.kaeri.engine.model.Bounds
import app.kaeri.engine.model.BrowseRail
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.Available
import app.kaeri.engine.model.AvailableAt
import app.kaeri.engine.model.FilterKey
import app.kaeri.engine.model.LibrarySegment
import app.kaeri.engine.model.OnList
import app.kaeri.engine.model.Pick
import app.kaeri.engine.model.SearchQuery
import app.kaeri.engine.model.Season
import app.kaeri.engine.model.SearchSort
import app.kaeri.engine.model.SurprisePool
import app.kaeri.engine.model.TrendPeriod
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.ffi.trendPeriodChoice
import app.kaeri.engine.ffi.trendPeriodLabel
import app.kaeri.engine.ffi.trendPeriods
import app.kaeri.store.TrendAnswer
import app.kaeri.engine.model.mediaFormatLabel
import app.kaeri.engine.model.searchCountries
import app.kaeri.engine.model.searchCountryLabel
import app.kaeri.engine.model.searchFormats
import app.kaeri.engine.model.searchSeasonLabel
import app.kaeri.engine.model.searchSortLabel
import app.kaeri.engine.model.searchSorts
import app.kaeri.engine.model.searchSourceLabel
import app.kaeri.engine.model.searchSources
import app.kaeri.engine.model.searchStatusLabel
import app.kaeri.engine.model.searchStatuses
import app.kaeri.store.AppClock
import app.kaeri.store.AppStore
import app.kaeri.store.BugContext
import app.kaeri.store.ListState
import app.kaeri.store.SearchModel
import app.kaeri.store.label
import app.kaeri.store.normalized
import app.kaeri.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.res.pluralStringResource
import app.kaeri.i18n.Strings
import android.content.Context
import app.kaeri.store.percentText
import app.kaeri.store.SearchWords

// Words

/** A recent search in one line: its words, then its filters. "hoshi · Mecha · TV". */
internal val SearchQuery.recentLine: String
    get() {
        val text = normalized.text
        if (text.isEmpty()) return headline()
        val summary = summary()
        return if (summary.isEmpty()) text else "$text · $summary"
    }

/** The year, when the years are a single one: the only case a season narrows. */
internal val SearchQuery.singleYear: UInt?
    get() = years.min?.takeIf { it == years.max }

/** The engine's words for one of the filters set, as its chip says it. */
internal fun SearchQuery.label(key: FilterKey): String? = active().firstOrNull { it.key == key }?.label

/** What a tap makes a genre or tag: off, then in, then out, then off again. */
internal val Pick.next: Pick
    get() = when (this) {
        Pick.OFF -> Pick.INCLUDE
        Pick.INCLUDE -> Pick.EXCLUDE
        Pick.EXCLUDE -> Pick.OFF
    }

private fun Pick.words(context: Context): String? = when (this) {
    Pick.OFF -> null
    Pick.INCLUDE -> Strings.filterIncluded(context)
    Pick.EXCLUDE -> Strings.filterLeftOut(context)
}

private val SEASONS = listOf(Season.WINTER, Season.SPRING, Season.SUMMER, Season.FALL)

/** "Hide obscure titles": at least this many people have the title on a list. The engine's `OBSCURE_BELOW`. */
private const val OBSCURE_BELOW = 1_000u

/** How strongly a tag must apply when nothing else is set. The engine's `DEFAULT_TAG_RANK`. */
private const val DEFAULT_TAG_RANK = 60

/** The first year the year filters offer. */
private const val FIRST_YEAR = 1970

/** Change the search as a filter does: the engine searches once the changes stop. */
private fun AppStore.editSearch(change: (SearchQuery) -> SearchQuery) = searchLive(change(search.status.value.query))

/** "Watching · Ep 7", "Planning", or for a title not on a list, "TV · 2024". */
private fun searchDetail(context: Context, entry: Entry): String? {
    if (entry.isDemo) return Strings.searchDemoPlays(context)
    entry.listStatus?.let { status ->
        val label = status.label(context).replaceFirstChar { it.uppercase() }
        val next = entry.progress?.next
        val line = if (next != null && status == LibrarySegment.WATCHING) Strings.searchDetailEpisode(context, label, next.toInt()) else label
        return listOfNotNull(line, availableLine(context, entry.availableAt)).joinToString(" · ") // i18n-skip
    }
    return listOfNotNull(formatLabel(context, entry.format), entry.year?.toString(), availableLine(context, entry.availableAt)).joinToString(" · ").ifEmpty { null } // i18n-skip
}

/** Where the title plays, as the end of its line says it; only the first match, in source order. */
private fun availableLine(context: Context, at: AvailableAt?): String? = when (at) {
    null -> null
    AvailableAt.PlaysHere -> context.getString(R.string.search_line_plays)
    AvailableAt.InLibrary -> context.getString(R.string.search_line_library)
    is AvailableAt.OnServer -> context.getString(R.string.search_line_server, at.server)
    is AvailableAt.OnService -> context.getString(R.string.search_line_service, at.service)
}

// The screen

/**
 * Search. [start] is searched as the screen first shows, and [startFilters] opens the filters
 * sheet: the fixtures' `query`, `genre` and `sheet` extras. [open] asks for the field to open for
 * typing as the screen shows, as a link to Search does; [opened] says it has, so coming back later
 * leaves the screen as it was.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreen(
    start: SearchQuery? = null,
    startFilters: Boolean = false,
    open: Boolean = false,
    opened: () -> Unit = {},
) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    val text = rememberTextFieldState(status.query.text)
    val bar = rememberSearchBarState()
    val scope = rememberCoroutineScope()
    var filtersShown by rememberSaveable { mutableStateOf(false) }
    // The phone pins its pill row under the field, with the filters button at its end. A wide
    // window keeps the row it had.
    val phone = !wideWindow()
    val grid = rememberLazyGridState()
    val startGrid = rememberLazyGridState()
    val rowScroll = rememberLazyListState()

    LaunchedEffect(Unit) {
        store.loadSearchStart()
        if (start != null && store.search.asked == null) store.search(start)
        if (startFilters) filtersShown = true
    }
    LaunchedEffect(open) {
        // Said once it is open: saying it first would restart this and stop the opening.
        if (open) {
            bar.animateToExpanded()
            opened()
        }
    }
    // Each keystroke searches, once the typing stops; the engine waits for the pause.
    // What the field itself gave the store since they last agreed. The store echoes each of these
    // a frame or more later; an echo is not words from outside and must not overwrite what was
    // typed in between.
    val sent = remember { mutableSetOf<String>() }
    LaunchedEffect(text) {
        snapshotFlow { text.text.toString() }.drop(1).collect {
            sent += it
            store.searchLive(it)
        }
    }
    // A recent search or a rail's See all brings its own words.
    LaunchedEffect(status.query.text) {
        val stored = status.query.text
        val typed = text.text.toString()
        if (stored == typed) {
            sent.clear()
        } else if (stored !in sent) {
            sent.clear()
            text.setTextAndPlaceCursorAtEnd(stored)
        }
    }

    fun submit() {
        store.search(text.text.toString())
        scope.launch { bar.animateToCollapsed() }
    }

    val clearLabel = stringResource(R.string.search_clear)
    val input = @Composable {
        SearchBarDefaults.InputField(
            textFieldState = text,
            searchBarState = bar,
            onSearch = { submit() },
            placeholder = { Text(stringResource(R.string.search_placeholder)) },
            leadingIcon = { Icon(Icons.Search, contentDescription = null) },
            trailingIcon = {
                if (text.text.isNotEmpty()) {
                    IconButton(
                        onClick = { text.clearText() },
                        modifier = Modifier.semantics { contentDescription = clearLabel },
                    ) { Icon(Icons.Clear, contentDescription = null) }
                } else if (status.query.isBrowse()) {
                    // Surprise me, from the filters in effect: only where there are no words.
                    SurpriseButton(SurprisePool.Filters(status.query))
                }
            },
        )
    }
    val colors = SearchBarDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh)

    // The destination's name for TalkBack; the bar says the rest.
    val searchTitle = stringResource(R.string.nav_search)
    // A new search starts the row at its first pill; a new answer shows from the first title, just
    // under the row, as it replaces the one before (the earlier answer stays where it was until
    // then). Only changes count: coming back from a title page keeps both where they were.
    LaunchedEffect(rowScroll) {
        snapshotFlow { status.query.text }.drop(1).collect { rowScroll.scrollToItem(0) }
    }
    LaunchedEffect(grid, phone) {
        if (phone) snapshotFlow { status.answered }.drop(1).collect { grid.scrollToItem(0) }
    }
    val overdue = searchOverdue(status)
    // The start stays while a first search is under 600 ms away, then gives way to the
    // results or their placeholders in one crossfade.
    val startShown = status.atStart || (status.answered == null && status.pending && !overdue)
    // The field and the row sit on the screen until the start or the results scroll under them,
    // then on a surface.
    val scrolledUnder = phone && (if (startShown) startGrid else grid).canScrollBackward
    val barSurface by animateColorAsState(
        if (scrolledUnder) MaterialTheme.colorScheme.surfaceContainer else MaterialTheme.colorScheme.surface,
        Motion.standard(),
        label = "search bar surface", // i18n-skip
    )
    Column(Modifier.semantics { paneTitle = searchTitle }) {
        Column(Modifier.then(if (phone) Modifier.background(barSurface) else Modifier).windowInsetsPadding(WindowInsets.statusBars)) {
            SearchBar(
                state = bar,
                inputField = input,
                colors = colors,
                modifier = Modifier.padding(horizontal = 16.dp).padding(top = 8.dp).fillMaxWidth(),
            )
            FilterRow(status.query, phone, rowScroll, onFilters = { filtersShown = true })
        }
        // While the demo is the only source: other titles play once a plugin store is joined.
        val demo by store.demo.collectAsStateWithLifecycle()
        androidx.compose.animation.AnimatedVisibility(demo.only, enter = androidx.compose.animation.fadeIn(Motion.standard()), exit = androidx.compose.animation.fadeOut(Motion.quick())) {
            Text(
                stringResource(R.string.search_demo_only),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
        Crossfade(startShown, Modifier.weight(1f), animationSpec = Motion.standard(), label = "search") { start -> // i18n-skip
            if (start) SearchStart(startGrid) else SearchResults(status.query, overdue, phone, grid)
        }
    }
    ExpandedFullScreenSearchBar(state = bar, inputField = input, colors = colors) {
        Typing(text.text.toString(), onPicked = {
            text.clearText()
            scope.launch { bar.animateToCollapsed() }
        })
    }
    if (filtersShown) FiltersSheet(onDismiss = { filtersShown = false })
}

/**
 * The search view while typing: the genres and tags the words start, which become filters, the
 * recent searches before anything is typed, and the titles found as the typing pauses.
 */
@Composable
private fun Typing(typed: String, onPicked: () -> Unit) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    val words = typed.trim()
    val facets = status.facets
    val genres = if (words.length >= 2) facets?.genresMatching(words).orEmpty().take(3) else emptyList()
    val tags = if (words.length >= 2) facets?.tagsMatching(words, 3u).orEmpty() else emptyList()
    Column(Modifier.fillMaxSize()) {
        for (genre in genres) {
            Suggestion(genre, stringResource(R.string.filter_genre)) {
                store.searchLive(status.query.copy(text = "").pickingGenre(genre, Pick.INCLUDE))
                onPicked()
            }
        }
        for (tag in tags) {
            Suggestion(tag, stringResource(R.string.filter_tag)) {
                store.searchLive(status.query.copy(text = "").pickingTag(tag, Pick.INCLUDE))
                onPicked()
            }
        }
        if (genres.isNotEmpty() || tags.isNotEmpty()) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
        Box(Modifier.weight(1f)) {
            when {
                words.isEmpty() && status.recents.isNotEmpty() -> PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp)) {
                    recentSearches(status.recents)
                }
                !status.atStart -> SearchResults(status.query, searchOverdue(status), phone = false, gridState = rememberLazyGridState())
                else -> {}
            }
        }
    }
}

/** A genre or tag the words start: a tap makes it a filter in place of the words. */
@Composable
private fun Suggestion(name: String, kind: String, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).clickable(onClickLabel = stringResource(R.string.search_filter_by, name), onClick = onClick).padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(name, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(kind, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

// The filters up front

/**
 * The filters up front: the sort, genres, year, season, format and status, each a menu. On a phone
 * the row sits under the field and scrolls sideways, with Clear all first while anything is set
 * and the filters button pinned at its end, outside the scrolling; a wide window keeps the filters
 * button as the last chip. Changing one searches.
 */
@Composable
private fun FilterRow(query: SearchQuery, phone: Boolean, rowScroll: LazyListState, onFilters: () -> Unit) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val words = filterRowState(context, query, phone)
    // A pill keeps its place by its key; on a phone the others make room as its width changes.
    fun LazyItemScope.slot(): Modifier =
        if (phone) Modifier.animateItem(fadeInSpec = Motion.standard(), placementSpec = Motion.standard(), fadeOutSpec = Motion.quick()).padding(end = 8.dp) else Modifier
    Row(Modifier.padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        LazyRow(
            state = rowScroll,
            contentPadding = PaddingValues(start = 16.dp, end = if (phone) 0.dp else 16.dp),
            horizontalArrangement = if (phone) Arrangement.Start else Arrangement.spacedBy(8.dp),
            modifier = Modifier.weight(1f).then(if (phone) Modifier.edgeFades(rowScroll) else Modifier),
        ) {
            if (phone) {
                item(key = "clear") {
                    AnimatedVisibility(
                        words.clearAll,
                        enter = fadeIn(Motion.standard()) + scaleIn(Motion.standard()) + expandHorizontally(Motion.standard()),
                        exit = fadeOut(Motion.quick()) + scaleOut(Motion.standard()) + shrinkHorizontally(Motion.standard()),
                    ) {
                        Row {
                            SearchChip(
                                stringResource(R.string.action_clear_all),
                                selected = false,
                                phone = true,
                                leading = Icons.Clear,
                                describe = Strings.searchPillClear(context),
                            ) { store.searchLive(store.search.status.value.query.cleared()) }
                            Spacer(Modifier.width(8.dp))
                        }
                    }
                }
            }
            item(key = "sort") {
                val current = query.sortInEffect()
                MenuChip(words.sort.label, selected = words.sort.set, phone = phone, describe = words.sort.spoken, modifier = slot()) { close ->
                    for (sort in searchSorts(!query.isBrowse())) {
                        if (sort == SearchSort.TRENDING && query.isBrowse()) {
                            // Trending is its three periods, so See all and the sort say the same.
                            val shown = query.periodInEffect() ?: TrendPeriod.TODAY
                            for (period in trendPeriods()) {
                                Choice(trendPeriodLabel(period), current == SearchSort.TRENDING && period == shown) {
                                    close()
                                    store.editSearch { it.copy(sort = sort, period = period.takeIf { p -> p != TrendPeriod.TODAY }) }
                                }
                            }
                        } else {
                            Choice(SearchWords.sort(LocalContext.current, sort), sort == current) {
                                close()
                                store.editSearch { it.copy(sort = sort, period = null) }
                            }
                        }
                    }
                }
            }
            item(key = "genres") {
                val set = query.genres.size + query.genresExcluded.size
                MenuChip(words.genres.label, selected = words.genres.set, phone = phone, describe = words.genres.spoken, modifier = slot()) { _ ->
                    if (set > 0) Choice(stringResource(R.string.filter_any_genre), false) { store.editSearch { it.copy(genres = emptyList(), genresExcluded = emptyList()) } }
                    val names = status.facets?.genres.orEmpty()
                    if (names.isEmpty()) DropdownMenuItem(text = { Text(stringResource(R.string.search_loading_genres)) }, onClick = {}, enabled = false)
                    for (name in names) {
                        Choice(name, query.genrePick(name) == Pick.INCLUDE) {
                            val now = store.search.status.value.query
                            store.searchLive(now.pickingGenre(name, if (now.genrePick(name) == Pick.INCLUDE) Pick.OFF else Pick.INCLUDE))
                        }
                    }
                }
            }
            item(key = "year") {
                MenuChip(words.year.label, selected = words.year.set, phone = phone, describe = words.year.spoken, modifier = slot()) { close ->
                    Choice(stringResource(R.string.filter_any_year), false) {
                        close()
                        store.searchLive(query.inYear(null))
                    }
                    for (year in (AppClock.now().atZone(AppClock.zone).year + 1) downTo FIRST_YEAR) {
                        Choice(year.toString(), query.singleYear == year.toUInt()) {
                            close()
                            store.searchLive(query.inYear(year.toUInt()))
                        }
                    }
                }
            }
            item(key = "season") {
                val season = query.season
                // A season is a season of one year.
                MenuChip(words.season.label, selected = words.season.set, enabled = words.season.enabled, phone = phone, describe = words.season.spoken, modifier = slot()) { close ->
                    Choice(stringResource(R.string.filter_any_season), false) {
                        close()
                        store.editSearch { it.copy(season = null) }
                    }
                    for (each in SEASONS) {
                        Choice(SearchWords.season(LocalContext.current, each), each == season) {
                            close()
                            store.editSearch { it.copy(season = each) }
                        }
                    }
                }
            }
            item(key = "format") {
                MenuChip(words.format.label, selected = words.format.set, phone = phone, describe = words.format.spoken, modifier = slot()) { _ ->
                    if (words.format.set) Choice(stringResource(R.string.filter_any_format), false) { store.editSearch { it.copy(formats = emptyList()) } }
                    for (format in searchFormats()) {
                        Choice(SearchWords.format(LocalContext.current, format), format in query.formats) {
                            store.editSearch { it.togglingFormat(format) }
                        }
                    }
                }
            }
            item(key = "status") {
                MenuChip(words.status.label, selected = words.status.set, phone = phone, describe = words.status.spoken, modifier = slot()) { _ ->
                    if (words.status.set) Choice(stringResource(R.string.filter_any_status), false) { store.editSearch { it.copy(statuses = emptyList()) } }
                    for (each in searchStatuses()) {
                        Choice(SearchWords.status(LocalContext.current, each), each in query.statuses) {
                            store.editSearch { it.togglingStatus(each) }
                        }
                    }
                }
            }
            if (!phone) {
                item(key = "filters") {
                    val count = query.advancedCount()
                    SearchChip(
                        if (count == 0u) stringResource(R.string.filters_button) else stringResource(R.string.filters_button_count, count.toInt()),
                        selected = count > 0u,
                        describe = if (count > 0u) stringResource(R.string.filters_button_count, count.toInt()) else stringResource(R.string.filters_button),
                        onClick = onFilters,
                    )
                }
            }
        }
        if (phone) FiltersPill(words.sheetCount, words.sheetSpoken, onFilters)
    }
}

/**
 * The filters button of a phone's row, pinned at its end: the glyph alone, then with how many
 * filters only the sheet holds are set, on the neutral fill the set pills have.
 */
@Composable
internal fun FiltersPill(count: Int, spoken: String, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.small
    val set = count > 0
    val fill by animateColorAsState(if (set) colors.secondaryContainer else Color.Transparent, Motion.quick(), label = "filters fill") // i18n-skip
    val edge by animateColorAsState(if (set) Color.Transparent else colors.outline, Motion.quick(), label = "filters edge") // i18n-skip
    val content by animateColorAsState(if (set) colors.onSecondaryContainer else colors.onSurfaceVariant, Motion.quick(), label = "filters content") // i18n-skip
    Box(
        Modifier
            .padding(start = 4.dp, end = 16.dp)
            .heightIn(min = 48.dp)
            .widthIn(min = 48.dp)
            .clickable(onClick = onClick)
            .semantics {
                selected = set
                contentDescription = spoken
            },
        contentAlignment = Alignment.Center,
    ) {
        Row(
            Modifier
                .padding(vertical = 8.dp)
                .animateContentSize(Motion.standard())
                .heightIn(min = with(LocalDensity.current) { 32.sp.toDp() })
                .clip(shape)
                .background(fill)
                .border(1.dp, edge, shape)
                .padding(horizontal = 12.dp, vertical = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(Icons.Sort, contentDescription = null, tint = content, modifier = Modifier.size(with(LocalDensity.current) { 18.sp.toDp() }))
            if (set) {
                Crossfade(count, animationSpec = Motion.quick(), label = "filters count") { n -> // i18n-skip
                    Text(n.toString(), style = MaterialTheme.typography.labelMedium.tabular(), color = content, maxLines = 1)
                }
            }
        }
    }
}

/** A chip that opens a menu of [choices]; a choice that settles the filter calls the close it is handed. */
@Composable
private fun MenuChip(
    title: String,
    selected: Boolean,
    enabled: Boolean = true,
    describe: String? = null,
    phone: Boolean = false,
    modifier: Modifier = Modifier,
    choices: @Composable ColumnScope.(close: () -> Unit) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        SearchChip(title, selected, enabled = enabled, menu = true, describe = describe, phone = phone) { open = true }
        Menu(expanded = open, onDismissRequest = { open = false }, modifier = Modifier.heightIn(max = 420.dp)) {
            choices { open = false }
        }
    }
}

/** One choice in a filter's menu, with a check when it is set. */
@Composable
private fun Choice(text: String, chosen: Boolean, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(text) },
        leadingIcon = {
            Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                if (chosen) Icon(Icons.Check, contentDescription = null)
            }
        },
        onClick = onClick,
        modifier = Modifier.semantics { selected = chosen },
    )
}

/**
 * A filter chip, 32 high in a 48 target: outlined with haze words, or once set, filled with a
 * check. A wide window fills it with primaryContainer, as the Library's chips are; a [phone] fills
 * it with secondaryContainer, so the row has no amber, and moves it on the motion tokens: the fill
 * on quick, the width and the words on quick and standard. [menu] adds the arrow of a chip that
 * opens a menu; a [leading] glyph takes the check's place on a chip that is never set.
 */
@Composable
private fun SearchChip(
    text: String,
    selected: Boolean,
    enabled: Boolean = true,
    menu: Boolean = false,
    describe: String? = null,
    phone: Boolean = false,
    leading: ImageVector? = null,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.small
    val content = when {
        !enabled -> colors.onSurfaceVariant.copy(alpha = 0.38f)
        selected -> if (phone) colors.onSecondaryContainer else colors.onPrimaryContainer
        leading != null -> colors.onSurface
        else -> colors.onSurfaceVariant
    }
    val setFill = if (phone) colors.secondaryContainer else colors.primaryContainer
    val fill = if (selected) setFill else Color.Transparent
    val edge = if (selected) Color.Transparent else colors.outline
    // A wide window changes at once, as it always did.
    val fillShown by animateColorAsState(fill, if (phone) Motion.quick() else snap(), label = "chip fill") // i18n-skip
    val edgeShown by animateColorAsState(edge, if (phone) Motion.quick() else snap(), label = "chip edge") // i18n-skip
    val contentShown by animateColorAsState(content, if (phone) Motion.quick() else snap(), label = "chip content") // i18n-skip
    val glyph = if (selected) Icons.Check else leading
    Box(
        Modifier
            .heightIn(min = 48.dp)
            .clickable(enabled = enabled, onClick = onClick)
            .semantics {
                this.selected = selected
                describe?.let { contentDescription = it }
            },
        contentAlignment = Alignment.Center,
    ) {
        Row(
            Modifier
                .padding(vertical = 8.dp)
                .then(if (phone) Modifier.animateContentSize(Motion.standard()) else Modifier)
                .heightIn(min = 32.dp)
                .clip(shape)
                .background(fillShown)
                .border(1.dp, edgeShown, shape)
                .padding(start = if (glyph != null) 8.dp else 16.dp, end = if (menu) 8.dp else 16.dp, top = 6.dp, bottom = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (glyph != null) Icon(glyph, contentDescription = null, tint = contentShown, modifier = Modifier.size(18.dp))
            if (phone) {
                Crossfade(text, animationSpec = Motion.quick(), label = "chip words") { words -> // i18n-skip
                    Text(words, style = MaterialTheme.typography.labelMedium, color = contentShown, maxLines = 1)
                }
            } else {
                Text(text, style = MaterialTheme.typography.labelMedium, color = contentShown, maxLines = 1)
            }
            if (menu) Icon(Icons.Down, contentDescription = null, tint = contentShown, modifier = Modifier.size(18.dp))
        }
    }
}

/** The filters set, each in the engine's words with a cross that takes it off, and Clear all. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ActiveFilters(query: SearchQuery) {
    val store = LocalAppStore.current
    val active = query.active()
    if (active.isEmpty()) return
    val colors = MaterialTheme.colorScheme
    val shape = MaterialTheme.shapes.small
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.Center) {
        for (filter in active) {
            val removeLabel = stringResource(R.string.filter_remove, filter.label)
            Box(
                Modifier
                    .heightIn(min = 48.dp)
                    .clickable(onClickLabel = stringResource(R.string.action_remove)) { store.searchLive(store.search.status.value.query.without(filter.key)) }
                    .semantics { contentDescription = removeLabel },
                contentAlignment = Alignment.Center,
            ) {
                Row(
                    Modifier
                        .heightIn(min = 32.dp)
                        .clip(shape)
                        .background(colors.surfaceContainer)
                        .padding(start = 12.dp, end = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(filter.label, style = MaterialTheme.typography.labelMedium, color = colors.onSurface, maxLines = 1)
                    Icon(Icons.Clear, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(16.dp))
                }
            }
        }
        if (active.size > 1) {
            Box(Modifier.heightIn(min = 48.dp), contentAlignment = Alignment.Center) {
                QuietButton(stringResource(R.string.action_clear_all)) { store.searchLive(store.search.status.value.query.cleared()) }
            }
        }
    }
}

// The titles found

/** The filters set and what is said above the titles, as the first rows of the grid. */
private fun LazyGridScope.resultsHeader(query: SearchQuery, caption: String?, livePaused: Boolean, phone: Boolean) {
    // The phone's row says what is set and takes it off from its own pills and Clear all.
    if (!phone && query.active().isNotEmpty()) {
        item(span = { GridItemSpan(maxLineSpan) }, key = "active", contentType = "active") { ActiveFilters(query) }
    }
    if (livePaused && !query.isBrowse()) {
        item(span = { GridItemSpan(maxLineSpan) }, key = "paused", contentType = "line") {
            Text(
                stringResource(R.string.search_paused_rate_limit),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
    caption?.let {
        item(span = { GridItemSpan(maxLineSpan) }, key = "caption", contentType = "line") {
            Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/**
 * Whether the answer to what is asked has been waited for past 600 ms (T8): the results before it
 * then give way to placeholders and the search shows as a job. Once up, the placeholders stay for
 * a further search rather than the earlier results coming back. Never while typing waits for the
 * search key.
 */
@Composable
private fun searchOverdue(status: SearchModel.Status): Boolean {
    var overdue by remember { mutableStateOf(false) }
    val waiting = status.pending && !status.livePaused
    LaunchedEffect(status.asked, waiting) {
        if (!waiting) {
            overdue = false
        } else if (!overdue) {
            delay(SEARCH_QUIET_MILLIS)
            overdue = true
        }
    }
    return overdue
}

private const val SEARCH_QUIET_MILLIS = 600L

/** The search as a job, said where its results will land: Searching for “lantern”. */
private fun SearchQuery.searchingLine(context: Context): String {
    val words = text.trim()
    return if (words.isEmpty()) Strings.searchSearchingFilters(context, headline()) else Strings.searchSearching(context, words)
}

/**
 * The titles found, a page at a time: the next is asked for as its placeholders come into view.
 * Past [overdue], placeholders in place of results that answer an earlier search.
 */
@Composable
private fun SearchResults(query: SearchQuery, overdue: Boolean, phone: Boolean, gridState: LazyGridState) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val state by store.search.list.state.collectAsStateWithLifecycle()
    val caption by store.search.list.caption.collectAsStateWithLifecycle()
    val status by store.search.status.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val grid = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp)
    val searching = status.asked?.searchingLine(LocalContext.current)
    val answered = state.takeIf { !overdue && (it is ListState.Loaded || it is ListState.Empty || it is ListState.Failed) }
    Loading(
        answered,
        returns = true,
        waited = overdue,
        placeholders = {
            PosterGrid(grid, gridState) {
                resultsHeader(query, searching, status.livePaused, phone)
                items(9, contentType = { "placeholder" }) { PosterPlaceholder() }
            }
        },
    ) { list ->
        PosterGrid(grid, gridState) {
            resultsHeader(query, caption, status.livePaused, phone)
            when (list) {
                is ListState.Loaded -> {
                    val entries = list.ids.mapNotNull { titles[it] }
                    items(entries, key = { it.id.toLong() }, contentType = { "poster" }) {
                        PosterCell(it, itemMotion(), from = "search", detail = searchDetail(LocalContext.current, it), onOpen = store::rememberSearch, detailLines = 2, count = it.countLineNow(), narrow = phone)
                    }
                    if (status.hasNext) {
                        items(3, key = { "more-$it" }, contentType = { "placeholder" }) { index ->
                            if (index == 0) LaunchedEffect(status.pages) { store.searchMore() }
                            PosterPlaceholder()
                        }
                    }
                }
                is ListState.Failed -> item(span = { GridItemSpan(maxLineSpan) }, key = "failed") {
                    Column {
                        Text(plainReason(context, list.reason), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Row {
                            QuietButton(stringResource(R.string.action_try_again)) { store.retrySearch() }
                            QuietButton(stringResource(R.string.action_report)) { openBugReport(context, store, BugContext("load", list.reason)) }
                        }
                    }
                }
                else -> item(span = { GridItemSpan(maxLineSpan) }, key = "nothing") { NothingFound(query) }
            }
        }
    }
}

/** Nothing found: what was asked, in the engine's words, and the way out when filters are set. */
@Composable
private fun NothingFound(query: SearchQuery) {
    val store = LocalAppStore.current
    Column(Modifier.padding(top = 8.dp)) {
        Text(query.nothingFound(), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        if (query.hasFilters()) {
            Box(Modifier.padding(top = 8.dp)) {
                QuietButton(stringResource(R.string.search_clear_filters)) { store.searchLive(store.search.status.value.query.cleared()) }
            }
        }
    }
}

// Search's start

/** Before anything is asked: the recent searches, the rails, and the genres. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SearchStart(gridState: LazyGridState) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp), gridState) {
        recentSearches(status.recents)
        val rails = status.rails
        if (rails == null) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "rail-placeholder") { RailPlaceholder() }
        } else {
            for (rail in rails.filter { it.entries.isNotEmpty() }) {
                item(span = { GridItemSpan(maxLineSpan) }, key = "rail-${rail.title}") {
                    if (rail.isTrending()) TrendingRail(rail, titles) else RailRow(rail, rail.entries.map { titles[it.id] ?: it })
                }
            }
        }
        val genres = status.facets?.genres.orEmpty()
        if (genres.isNotEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "genres") {
                Column {
                    StartHeading(stringResource(R.string.search_genres))
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        for (genre in genres) SearchChip(genre, selected = false) { store.search(SearchQuery(genres = listOf(genre))) }
                    }
                }
            }
        }
    }
}

/** The recent searches, newest first, each with its filters: a tap searches again, the cross forgets it. */
private fun LazyGridScope.recentSearches(recents: List<SearchQuery>) {
    if (recents.isEmpty()) return
    item(span = { GridItemSpan(maxLineSpan) }, key = "recent") {
        val store = LocalAppStore.current
        Column {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.weight(1f)) { StartHeading(stringResource(R.string.search_recent)) }
                QuietButton(stringResource(R.string.action_clear)) { store.clearRecentSearches() }
            }
            for (query in recents.take(5)) {
                val removeLabel = stringResource(R.string.search_recent_remove, query.recentLine)
                Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        query.recentLine,
                        style = MaterialTheme.typography.bodyLarge,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f).heightIn(min = 48.dp).clickable { store.search(query) }.padding(vertical = 12.dp),
                    )
                    IconButton(onClick = { store.forgetRecentSearch(query) }, modifier = Modifier.semantics { contentDescription = removeLabel }) {
                        Icon(Icons.Clear, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
                    }
                }
            }
        }
    }
}

/** One rail: its heading, what it is, See all, and its posters in a row that scrolls. */
@Composable
private fun RailRow(rail: BrowseRail, entries: List<Entry>) {
    val store = LocalAppStore.current
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                StartHeading(rail.title)
                rail.detail?.let {
                    Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                }
            }
            QuietButton(stringResource(R.string.search_see_all)) { store.search(rail.query) }
        }
        LazyRow(horizontalArrangement = Arrangement.spacedBy(13.dp)) {
            items(entries, key = { it.id.toLong() }) { PosterCell(it, itemMotion().width(104.dp), from = "rail-${rail.title}", detail = it.genreLine, count = it.countLineNow()) }
        }
    }
}

/** Whether this is Search's Trending rail: the one sorted by Trending, with nothing else set. */
private fun BrowseRail.isTrending(): Boolean = query.sort == SearchSort.TRENDING && !query.hasFilters()

/**
 * The Trending rail: its heading is a menu of the periods, and the shelf below crossfades when the
 * period changes. The old covers stay while the new ones load; past 600 ms they crossfade to
 * plates, which hold at least 0.3 s, then to the new covers. Never blank.
 */
@Composable
private fun TrendingRail(rail: BrowseRail, titles: Map<AnilistId, Entry>) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    val period = status.period
    val answer = if (period == TrendPeriod.TODAY) TrendAnswer(rail.entries) else status.trending[period]
    var held by remember { mutableStateOf(answer?.takeIf { !it.failed }) }
    var plates by remember { mutableStateOf(false) }
    var platesAt by remember { mutableStateOf(0L) }
    LaunchedEffect(period, answer) {
        if (answer != null && !answer.failed) {
            if (plates) {
                val held = System.currentTimeMillis() - platesAt
                if (held < 300) delay(300 - held)
            }
            held = answer
            plates = false
        } else if (answer == null) {
            delay(600)
            platesAt = System.currentTimeMillis()
            plates = true
        } else {
            held = answer
            plates = false
        }
    }
    val shown = if (plates) null else held
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                TrendingHeading(period, store::chooseTrending)
                val age = shown?.updatedAt
                if (age != null) {
                    val ago = android.text.format.DateUtils.getRelativeTimeSpanString(age * 1000, System.currentTimeMillis(), android.text.format.DateUtils.MINUTE_IN_MILLIS).toString()
                    Text(Strings.trendingUpdated(LocalContext.current, ago), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                }
            }
            QuietButton(stringResource(R.string.search_see_all)) {
                store.search(SearchQuery(sort = SearchSort.TRENDING, period = period.takeIf { it != TrendPeriod.TODAY }))
            }
        }
        if (answer?.failed == true && !plates) {
            Text(
                Strings.trendingFailed(LocalContext.current, trendPeriodChoice(period).lowercase()),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            QuietButton(stringResource(R.string.action_try_again)) { store.chooseTrending(period) }
        } else {
            Crossfade(shown, animationSpec = Motion.standard(), label = "trending shelf") { answered -> // i18n-skip
                if (answered == null) {
                    Row(horizontalArrangement = Arrangement.spacedBy(13.dp)) {
                        repeat(4) { Box(Modifier.width(104.dp)) { PosterPlaceholder() } }
                    }
                } else {
                    LazyRow(horizontalArrangement = Arrangement.spacedBy(13.dp)) {
                        items(answered.entries.map { titles[it.id] ?: it }, key = { it.id.toLong() }) {
                            PosterCell(it, Modifier.width(104.dp), from = "rail-trending", detail = it.genreLine, count = it.countLineNow())
                        }
                    }
                }
            }
        }
    }
}

/** Rails to come, once they are 600 ms away: headings and posters where the rails will be. */
@Composable
private fun RailPlaceholder() {
    AfterWait {
        Column {
            repeat(3) {
                TextPlaceholder(0.4f, 24.dp, Modifier.padding(vertical = 12.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(13.dp)) {
                    repeat(4) { Box(Modifier.width(104.dp)) { PosterPlaceholder() } }
                }
            }
        }
    }
}

@Composable
private fun StartHeading(text: String) {
    Text(text, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(vertical = 12.dp).semantics { heading() })
}

// Every filter

/**
 * Every filter, in sections, in a sheet over the results: each change searches, so there is
 * nothing to apply. Half the screen at first, so the results show change under it.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun FiltersSheet(onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    val query = status.query
    val facets = status.facets
    val scoreAny = stringResource(R.string.filter_score_any)
    val colors = MaterialTheme.colorScheme
    // On a phone a chosen chip takes the neutral fill of the pinned row's set pills, leaving amber
    // to the switches and the action; a wide window keeps primaryContainer for now.
    val phone = !wideWindow()
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberBottomSheetState(SheetValue.Hidden, setOf(SheetValue.Hidden, SheetValue.PartiallyExpanded, SheetValue.Expanded)),
        containerColor = colors.surfaceContainer,
        scrimColor = androidx.compose.material3.MaterialTheme.colorScheme.scrim.copy(alpha = 0.32f),
        dragHandle = { BottomSheetDefaults.DragHandle(color = colors.onSurfaceVariant.copy(alpha = 0.4f)) },
        modifier = Modifier.padding(top = 77.dp - 24.dp),
    ) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) {
            Row(Modifier.padding(start = 16.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.filters_title), style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f).semantics { heading() })
                QuietButton(stringResource(R.string.action_reset), enabled = query.hasFilters()) { store.searchLive(query.cleared()) }
            }

            FilterHeading(stringResource(R.string.filter_genres))
            FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (name in facets?.genres.orEmpty()) {
                    PickChip(name, query.genrePick(name)) { store.editSearch { it.pickingGenre(name, it.genrePick(name).next) } }
                }
            }
            FilterNote(stringResource(R.string.filter_hint_touch))

            TagsSection(query)

            FilterHeading(stringResource(R.string.filter_released))
            YearsRange(query.years) { bounds ->
                store.editSearch { now ->
                    val next = now.copy(years = bounds)
                    if (next.singleYear == null) next.copy(season = null) else next
                }
            }

            FilterHeading(stringResource(R.string.filter_length))
            FilterLabel(stringResource(R.string.filter_episodes))
            Presets(
                listOf(Bounds(1u, 1u), Bounds(null, 12u), Bounds(13u, 26u), Bounds(50u, null)),
                query.episodes,
                words = { SearchQuery(episodes = it).label(FilterKey.Episodes) },
            ) { bounds -> store.editSearch { it.copy(episodes = bounds) } }
            FilterLabel(stringResource(R.string.filter_episode_length))
            Presets(
                listOf(Bounds(null, 15u), Bounds(20u, 30u), Bounds(60u, null)),
                query.minutes,
                words = { SearchQuery(minutes = it).label(FilterKey.Minutes) },
            ) { bounds -> store.editSearch { it.copy(minutes = bounds) } }

            FilterHeading(stringResource(R.string.filter_reception))
            StepSlider(
                stringResource(R.string.filter_score_at_least),
                value = query.minScore?.toInt() ?: 0,
                range = 0..95,
                step = 5,
                words = { if (it == 0) scoreAny else percentText(it) },
            ) { score -> store.editSearch { it.copy(minScore = score.takeIf { it > 0 }?.toUByte()) } }
            FilterSwitch(stringResource(R.string.filter_hide_obscure), query.minPopularity != null) { on ->
                store.editSearch { it.copy(minPopularity = if (on) OBSCURE_BELOW else null) }
            }

            FilterHeading(stringResource(R.string.filter_origin))
            FilterLabel(stringResource(R.string.filter_source_material))
            FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (source in searchSources()) {
                    SearchChip(searchSourceLabel(source), source in query.sources, phone = phone) { store.editSearch { it.togglingSource(source) } }
                }
            }
            FilterLabel(stringResource(R.string.filter_country))
            FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                for (code in searchCountries()) {
                    SearchChip(SearchWords.country(LocalContext.current, code), code in query.countries, phone = phone) { store.editSearch { it.togglingCountry(code) } }
                }
            }

            FilterHeading(stringResource(R.string.filter_where_to_watch))
            val sites = facets?.streamingSites.orEmpty()
            if (sites.isNotEmpty()) {
                FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (site in sites) {
                        SearchChip(site, site in query.streamingOn, phone = phone) { store.editSearch { it.togglingStreaming(site) } }
                    }
                }
            }
            FilterSwitch(stringResource(R.string.filter_licensed_only), query.licensedOnly) { on -> store.editSearch { it.copy(licensedOnly = on) } }

            FilterHeading(stringResource(R.string.search_available))
            val availableChoices = listOf<Available?>(null, Available.PLAYS_HERE, Available.ON_YOUR_SERVICES)
            ConnectedChoice(
                listOf(stringResource(R.string.search_available_all), stringResource(R.string.search_available_here), stringResource(R.string.search_available_services)),
                selected = availableChoices.indexOf(query.available),
                onSelect = { index -> store.editSearch { it.copy(available = availableChoices[index]) } },
                modifier = Modifier.padding(horizontal = 16.dp),
            )

            if (anilistSignedIn()) {
                FilterHeading(stringResource(R.string.calendar_filter_mine))
                val choices = listOf<OnList?>(null, OnList.HIDE, OnList.ONLY)
                ConnectedChoice(
                    listOf(stringResource(R.string.filter_show), stringResource(R.string.filter_hide), stringResource(R.string.filter_only)),
                    selected = choices.indexOf(query.onList),
                    onSelect = { index -> store.editSearch { it.copy(onList = choices[index]) } },
                    modifier = Modifier.padding(horizontal = 16.dp),
                )
            }

            FilterNote(stringResource(R.string.filter_live_note))
        }
    }
}

/** The tags: a field to find one, their categories each opening its tags, and how strongly they apply. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TagsSection(query: SearchQuery) {
    val store = LocalAppStore.current
    val status by store.search.status.collectAsStateWithLifecycle()
    var find by rememberSaveable { mutableStateOf("") }
    var spoilers by rememberSaveable { mutableStateOf(false) }
    var opened by rememberSaveable { mutableStateOf<String?>(null) }
    val groups = status.facets?.tagGroups.orEmpty()

    FilterHeading(stringResource(R.string.filter_tags))
    Box(Modifier.padding(horizontal = 16.dp)) { SheetField(find, { find = it }, stringResource(R.string.filter_tag_find)) }
    val words = find.trim().lowercase()
    if (words.isNotEmpty()) {
        val found = groups.flatMap { it.tags }.filter { (spoilers || !it.spoiler) && words in it.name.lowercase() }.take(40)
        if (found.isEmpty()) FilterNote(stringResource(R.string.filter_tag_none))
        FlowRow(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (tag in found) {
                PickChip(tag.name, query.tagPick(tag.name)) { store.editSearch { it.pickingTag(tag.name, it.tagPick(tag.name).next) } }
            }
        }
    } else {
        Spacer(Modifier.heightIn(min = 8.dp))
        for (group in groups) {
            val set = group.tags.count { query.tagPick(it.name) != Pick.OFF }
            val isOpen = opened == group.category
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 48.dp)
                    .clickable(onClickLabel = stringResource(if (isOpen) R.string.action_close else R.string.action_open)) { opened = if (isOpen) null else group.category }
                    .padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(group.category, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
                if (set > 0) {
                    Text(
                        "$set",
                        style = MaterialTheme.typography.bodyMedium.tabular(),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(end = 8.dp),
                    )
                }
                Icon(if (isOpen) Icons.Down else Icons.Chevron, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (isOpen) {
                FlowRow(Modifier.padding(horizontal = 16.dp).padding(bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for (tag in group.tags.filter { spoilers || !it.spoiler }) {
                        PickChip(tag.name, query.tagPick(tag.name)) { store.editSearch { it.pickingTag(tag.name, it.tagPick(tag.name).next) } }
                    }
                }
            }
        }
    }
    FilterSwitch(stringResource(R.string.filter_spoiler_tags), spoilers) { spoilers = it }
    val tagged = query.tags.isNotEmpty() || query.tagsExcluded.isNotEmpty()
    StepSlider(
        stringResource(R.string.filter_tag_rank_title),
        value = query.minTagRank?.toInt() ?: DEFAULT_TAG_RANK,
        range = 0..100,
        step = 10,
        enabled = tagged,
        words = { percentText(it) },
    ) { rank -> store.editSearch { it.copy(minTagRank = if (rank == DEFAULT_TAG_RANK) null else rank.toUByte()) } }
}

/**
 * A genre or tag with three states: outlined when off, filled with a check when included, and
 * struck through on the raised fill when left out. On a phone the included fill is the neutral
 * secondaryContainer of the pinned row's set pills, leaving amber to the sheet's switches and
 * its action; a wide window keeps primaryContainer for now.
 */
@Composable
private fun PickChip(name: String, pick: Pick, onClick: () -> Unit) {
    val words = pick.words(LocalContext.current)
    val colors = MaterialTheme.colorScheme
    val phone = !wideWindow()
    val shape = MaterialTheme.shapes.small
    Box(
        Modifier
            .heightIn(min = 48.dp)
            .clickable(onClick = onClick)
            .semantics {
                words?.let { stateDescription = it }
                selected = pick != Pick.OFF
            },
        contentAlignment = Alignment.Center,
    ) {
        val background = when (pick) {
            Pick.OFF -> Modifier.border(1.dp, colors.outline, shape)
            Pick.INCLUDE -> Modifier.background(if (phone) colors.secondaryContainer else colors.primaryContainer)
            Pick.EXCLUDE -> Modifier.background(colors.surfaceContainerHighest)
        }
        val content = when (pick) {
            Pick.OFF -> colors.onSurfaceVariant
            Pick.INCLUDE -> if (phone) colors.onSecondaryContainer else colors.onPrimaryContainer
            Pick.EXCLUDE -> colors.onSurface
        }
        Row(
            Modifier.heightIn(min = 32.dp).clip(shape).then(background).padding(start = if (pick == Pick.OFF) 16.dp else 8.dp, end = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            when (pick) {
                Pick.INCLUDE -> Icon(Icons.Check, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
                Pick.EXCLUDE -> Icon(Icons.Clear, contentDescription = null, tint = content, modifier = Modifier.size(18.dp))
                Pick.OFF -> {}
            }
            Text(
                name,
                style = MaterialTheme.typography.labelMedium.copy(textDecoration = if (pick == Pick.EXCLUDE) TextDecoration.LineThrough else null),
                color = content,
                maxLines = 1,
            )
        }
    }
}

/** The years a title started in, from 1970 to next year, as a range; at both ends, any year. */
@Composable
private fun YearsRange(years: Bounds, onChange: (Bounds) -> Unit) {
    val last = AppClock.now().atZone(AppClock.zone).year + 1
    val low = years.min?.toInt()?.coerceIn(FIRST_YEAR, last) ?: FIRST_YEAR
    val high = years.max?.toInt()?.coerceIn(FIRST_YEAR, last) ?: last
    var shown by remember(low, high) { mutableStateOf(low.toFloat()..high.toFloat()) }
    val from = shown.start.toInt()
    val to = shown.endInclusive.toInt()
    val yearName = stringResource(R.string.filter_year)
    val words = when {
        from == FIRST_YEAR && to == last -> stringResource(R.string.filter_any_year)
        from == to -> "$from"
        from == FIRST_YEAR -> stringResource(R.string.filter_years_up_to, "$to")
        to == last -> stringResource(R.string.filter_years_from, "$from")
        else -> stringResource(R.string.filter_years_between, "$from", "$to")
    }
    val described = yearName + ", " + words // i18n-skip
    Column(Modifier.padding(horizontal = 16.dp)) {
        Row(Modifier.heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.filter_year), style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
            Text(words, style = MaterialTheme.typography.bodyMedium.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        RangeSlider(
            value = shown,
            onValueChange = { shown = it },
            onValueChangeFinished = {
                val a = shown.start.toInt()
                val b = shown.endInclusive.toInt()
                onChange(Bounds(a.takeIf { it > FIRST_YEAR }?.toUInt(), b.takeIf { it < last }?.toUInt()))
            },
            valueRange = FIRST_YEAR.toFloat()..last.toFloat(),
            steps = last - FIRST_YEAR - 1,
            colors = sliderColors(),
            modifier = Modifier.semantics { contentDescription = described },
        )
    }
}

/** A slider that steps by [step] across [range], its value in [words] at the end of its label. */
@Composable
private fun StepSlider(
    title: String,
    value: Int,
    range: IntRange,
    step: Int,
    enabled: Boolean = true,
    words: (Int) -> String,
    onChange: (Int) -> Unit,
) {
    var shown by remember(value) { mutableStateOf(value.toFloat()) }
    val described = title + ", " + words(shown.toInt()) // i18n-skip
    val now = shown.toInt()
    Column(Modifier.padding(horizontal = 16.dp)) {
        Row(Modifier.heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge,
                color = if (enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            Text(words(now), style = MaterialTheme.typography.bodyMedium.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Slider(
            value = shown,
            onValueChange = { shown = it },
            onValueChangeFinished = { onChange(shown.toInt()) },
            valueRange = range.first.toFloat()..range.last.toFloat(),
            steps = (range.last - range.first) / step - 1,
            enabled = enabled,
            colors = sliderColors(),
            modifier = Modifier.semantics { contentDescription = described },
        )
    }
}

/** The brief's slider: the amber track and thumb on outline, with no tick marks. */
@Composable
private fun sliderColors() = SliderDefaults.colors(
    activeTickColor = Color.Transparent,
    inactiveTickColor = Color.Transparent,
    inactiveTrackColor = MaterialTheme.colorScheme.outline,
)

/** Ready-made ranges, each in the engine's words; a tap on the one set takes it off. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Presets(choices: List<Bounds>, current: Bounds, words: (Bounds) -> String?, onPick: (Bounds) -> Unit) {
    // On a phone a chosen range takes the neutral fill, like the sheet's other chips.
    val phone = !wideWindow()
    FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for (bounds in choices) {
            SearchChip(words(bounds).orEmpty(), selected = current == bounds, phone = phone) { onPick(if (current == bounds) Bounds(null, null) else bounds) }
        }
    }
}

/** A switch at the end of its row, its label at the start, the whole row the target. */
@Composable
private fun FilterSwitch(title: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clickable { onChange(!checked) }
            .padding(horizontal = 16.dp)
            .semantics { selected = checked },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        SettingSwitchMark(checked)
    }
}

/** A section's heading in the sheet, in haze, as the show settings sheet has them. */
@Composable
private fun FilterHeading(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp).padding(top = 24.dp, bottom = 4.dp).semantics { heading() },
    )
}

/** What a group of chips under a heading chooses. */
@Composable
private fun FilterLabel(text: String) {
    Text(text, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.padding(horizontal = 16.dp).padding(top = 8.dp, bottom = 4.dp))
}

@Composable
private fun FilterNote(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 16.dp).padding(top = 8.dp, bottom = 4.dp),
    )
}
