// Reading's Search row and filters sheet, built from the shows' Search parts so the two scopes
// look and move alike: the order, then Genres, Started, Origin, Format and Status as the shows'
// chips that open menus, Your sources only once there are words, and the filters button (pinned
// at the row's end on a phone). A set chip says its value alone. The sheet holds every reading
// filter in the shows' sections and chips.

package app.kaeri.ui.reading

import android.content.Context
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandHorizontally
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.shrinkHorizontally
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.engine.model.Pick
import app.kaeri.engine.model.ReadFilters
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.engine.model.SearchSort
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.SearchWords
import app.kaeri.store.text
import app.kaeri.ui.Choice
import app.kaeri.ui.FilterHeading
import app.kaeri.ui.FilterNote
import app.kaeri.ui.FilterSwitch
import app.kaeri.ui.FiltersPill
import app.kaeri.ui.FiltersSheetFrame
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.MenuChip
import app.kaeri.ui.PickChip
import app.kaeri.ui.RowPill
import app.kaeri.ui.SearchChip
import app.kaeri.ui.edgeFades
import app.kaeri.ui.wideWindow

/** Nothing asked: no words, no filters and no order chosen, so Search shows its start. */
internal fun ReadSearchQuery.isStart(): Boolean = text.isBlank() && !anyFilter(this) && sort == null && medium == null

/** The query with every filter off, its words and order kept, as the shows' Clear all does. */
internal fun ReadSearchQuery.clearedFilters(): ReadSearchQuery = copy(
    medium = null,
    formats = emptyList(),
    origins = emptyList(),
    status = null,
    genres = emptyList(),
    excludeGenres = emptyList(),
    started = null,
    sourcesOnly = false,
)

/** The row as the query sets it, worded as the shows' row is. */
@Immutable
internal data class ReadRowState(
    val clearAll: Boolean,
    val sort: RowPill,
    val genres: RowPill,
    val started: RowPill,
    val origin: RowPill,
    val format: RowPill,
    val status: RowPill,
)

/** Up to this many formats or origins are named on a phone's pill; past it, the count. */
private const val NAMED_ON_PHONE = 2

/**
 * What each pill shows: its name, or once set its value alone ("Manga", "Fantasy"). On a phone a
 * single genre is named and two formats or origins are joined; past that a pill counts. With words
 * and no order chosen, the order is Relevance, as AniList then orders by match.
 */
internal fun readRowState(context: Context, query: ReadSearchQuery, filters: ReadFilters, phone: Boolean): ReadRowState {
    fun pill(name: String, value: String?) = RowPill(
        label = value ?: name,
        set = value != null,
        spoken = if (value != null) Strings.searchPillSet(context, name, value) else Strings.searchPillAny(context, name),
    )
    fun labels(keys: List<String>, choices: List<app.kaeri.engine.model.ReadFilterChoice>) =
        keys.mapNotNull { key -> choices.firstOrNull { it.key == key }?.label?.text(context) }

    val sortWord = sortWordsFor(context, query)
    val sort = RowPill(sortWord, set = query.sort != null, spoken = Strings.sortLabel(context, sortWord))

    val picked = query.genres.size + query.excludeGenres.size
    val genres = pill(
        Strings.readFilterGenres(context),
        when {
            picked == 0 -> null
            phone && picked == 1 -> query.genres.firstOrNull() ?: Strings.readFilterNotGenre(context, query.excludeGenres.first())
            else -> Strings.searchGenresPicked(context, picked)
        },
    )
    val started = pill(Strings.readSearchStarted(context), query.started?.takeIf { it != STARTED_ANY }?.let { labels(listOf(it), filters.started).firstOrNull() })
    val origins = labels(query.origins, filters.origins)
    val origin = pill(
        Strings.readFilterOrigin(context),
        when {
            origins.isEmpty() -> null
            origins.size == 1 || (phone && origins.size <= NAMED_ON_PHONE) -> origins.joinToString(", ") // i18n-skip
            else -> value(context, query.origins, filters.origins)
        },
    )
    // One-shots are said as part of Manga.
    val formatKeys = if (FORMAT_MANGA in query.formats) query.formats - FORMAT_ONE_SHOT else query.formats
    val formats = labels(formatKeys, filters.formats)
    val format = pill(
        Strings.readFilterFormat(context),
        when {
            formats.isEmpty() -> null
            formats.size == 1 || (phone && formats.size <= NAMED_ON_PHONE) -> formats.joinToString(", ") // i18n-skip
            else -> Strings.searchFormatsPicked(context, formats.size)
        },
    )
    val status = pill(Strings.readFilterStatus(context), query.status?.let { labels(listOf(it), filters.statuses).firstOrNull() })
    return ReadRowState(
        clearAll = anyFilter(query) || query.medium != null || (query.sourcesOnly && query.text.isNotBlank()),
        sort = sort,
        genres = genres,
        started = started,
        origin = origin,
        format = format,
        status = status,
    )
}

/** The order in words: the one chosen, else Relevance with words and Popular without. */
private fun sortWordsFor(context: Context, query: ReadSearchQuery): String =
    if (query.sort == null && query.text.isNotBlank()) SearchWords.sort(context, SearchSort.RELEVANCE) else sortWords(context, query.sort)

/**
 * The filters set, each in its words with what takes it off: the genres in and left out,
 * Started, the origins, the formats, Status, and Your sources only while there are words.
 */
internal fun activeFilters(context: Context, query: ReadSearchQuery, filters: ReadFilters): List<Pair<String, (ReadSearchQuery) -> ReadSearchQuery>> = buildList {
    fun label(key: String, choices: List<app.kaeri.engine.model.ReadFilterChoice>) = choices.firstOrNull { it.key == key }?.label?.text(context)
    for (genre in query.genres) add(genre to { q: ReadSearchQuery -> q.copy(genres = q.genres - genre) })
    for (genre in query.excludeGenres) add(Strings.readFilterNotGenre(context, genre) to { q: ReadSearchQuery -> q.copy(excludeGenres = q.excludeGenres - genre) })
    query.started?.takeIf { it != STARTED_ANY }?.let { key -> label(key, filters.started)?.let { add(it to { q: ReadSearchQuery -> q.copy(started = null) }) } }
    for (key in query.origins) label(key, filters.origins)?.let { add(it to { q: ReadSearchQuery -> q.copy(origins = q.origins - key) }) }
    val formats = if (FORMAT_MANGA in query.formats) query.formats - FORMAT_ONE_SHOT else query.formats
    for (key in formats) label(key, filters.formats)?.let { add(it to { q: ReadSearchQuery -> q.copy(formats = q.formats.togglingFormat(key)) }) }
    query.status?.let { key -> label(key, filters.statuses)?.let { add(it to { q: ReadSearchQuery -> q.copy(status = null) }) } }
    if (query.sourcesOnly && query.text.isNotBlank()) add(Strings.readSearchSourcesOnly(context) to { q: ReadSearchQuery -> q.copy(sourcesOnly = false) })
}

/**
 * Reading's row under the field, as the shows' row: on a phone it scrolls sideways under the
 * field with Clear all first while anything is set and the filters button pinned at its end; a
 * wide window keeps the filters button as the last chip. Each change searches through [go].
 */
@Composable
internal fun ReadSearchRow(
    query: ReadSearchQuery,
    filters: ReadFilters,
    phone: Boolean,
    rowScroll: LazyListState,
    onFilters: () -> Unit,
    go: (ReadSearchQuery) -> Unit,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val words = readRowState(context, query, filters, phone)
    fun change(edit: (ReadSearchQuery) -> ReadSearchQuery) = go(edit(store.reading.search.value.query))
    // A pill keeps its place by its key; on a phone the others make room as its width changes.
    fun LazyItemScope.slot(): Modifier =
        if (phone) Modifier.animateItem(fadeInSpec = Motion.standard(), placementSpec = Motion.standard(), fadeOutSpec = Motion.quick()).padding(end = 8.dp) else Modifier
    // AniList's filters say nothing to the sources: dimmed while only they are asked.
    val typed = query.text.isNotBlank()
    val on = !query.sourcesOnly || !typed
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
                            ) { change { it.clearedFilters() } }
                            Spacer(Modifier.width(8.dp))
                        }
                    }
                }
            }
            item(key = "sort") {
                val current = query.sort ?: filters.sorts.firstOrNull()
                MenuChip(words.sort.label, selected = words.sort.set, enabled = on, phone = phone, describe = words.sort.spoken, modifier = slot()) { close ->
                    // With words AniList orders by match unless told otherwise: Relevance comes first.
                    if (typed) {
                        Choice(SearchWords.sort(context, SearchSort.RELEVANCE), query.sort == null) {
                            close()
                            change { it.copy(sort = null) }
                        }
                    }
                    for (sort in filters.sorts) {
                        Choice(sortWords(context, sort), (typed && sort == query.sort) || (!typed && sort == current)) {
                            close()
                            change { it.copy(sort = sort) }
                        }
                    }
                }
            }
            item(key = "genres") {
                val set = query.genres.size + query.excludeGenres.size
                MenuChip(words.genres.label, selected = words.genres.set, enabled = on, phone = phone, describe = words.genres.spoken, modifier = slot()) { _ ->
                    if (set > 0) Choice(stringResource(R.string.filter_any_genre), false) { change { it.copy(genres = emptyList(), excludeGenres = emptyList()) } }
                    for (genre in filters.genres) {
                        Choice(genre, genre in query.genres) {
                            change { now ->
                                if (genre in now.genres) now.copy(genres = now.genres - genre) else now.copy(genres = now.genres + genre, excludeGenres = now.excludeGenres - genre)
                            }
                        }
                    }
                }
            }
            item(key = "started") {
                MenuChip(words.started.label, selected = words.started.set, enabled = on, phone = phone, describe = words.started.spoken, modifier = slot()) { close ->
                    for (choice in filters.started) {
                        Choice(choice.label.text(context), (query.started ?: STARTED_ANY) == choice.key) {
                            close()
                            change { it.copy(started = choice.key.takeIf { key -> key != STARTED_ANY }) }
                        }
                    }
                }
            }
            item(key = "origin") {
                MenuChip(words.origin.label, selected = words.origin.set, enabled = on, phone = phone, describe = words.origin.spoken, modifier = slot()) { _ ->
                    if (words.origin.set) Choice(Strings.readFilterAnyOrigin(context), false) { change { it.copy(origins = emptyList()) } }
                    for (choice in filters.origins) {
                        Choice(choice.label.text(context), choice.key in query.origins) { change { it.copy(origins = it.origins.toggling(choice.key)) } }
                    }
                }
            }
            item(key = "format") {
                MenuChip(words.format.label, selected = words.format.set, enabled = on, phone = phone, describe = words.format.spoken, modifier = slot()) { _ ->
                    if (words.format.set) Choice(stringResource(R.string.filter_any_format), false) { change { it.copy(formats = emptyList()) } }
                    // One-shots are part of Manga, so the menu has no choice of their own.
                    for (choice in filters.formats.filter { it.key != FORMAT_ONE_SHOT }) {
                        Choice(choice.label.text(context), choice.key in query.formats) { change { it.copy(formats = it.formats.togglingFormat(choice.key)) } }
                    }
                }
            }
            item(key = "status") {
                MenuChip(words.status.label, selected = words.status.set, enabled = on, phone = phone, describe = words.status.spoken, modifier = slot()) { close ->
                    if (words.status.set) {
                        Choice(stringResource(R.string.filter_any_status), false) {
                            close()
                            change { it.copy(status = null) }
                        }
                    }
                    for (choice in filters.statuses) {
                        Choice(choice.label.text(context), query.status == choice.key) {
                            close()
                            change { it.copy(status = choice.key) }
                        }
                    }
                }
            }
            // Your sources only needs words to ask by, so it joins the row once there are some.
            if (typed) {
                item(key = "sources-only") {
                    Box(slot()) {
                        SearchChip(Strings.readSearchSourcesOnly(context), selected = query.sourcesOnly, phone = phone) { change { it.copy(sourcesOnly = !it.sourcesOnly) } }
                    }
                }
            }
            if (!phone) {
                item(key = "filters") {
                    SearchChip(stringResource(R.string.filters_button), selected = false, describe = stringResource(R.string.filters_button), onClick = onFilters)
                }
            }
        }
        // The row names every filter the sheet holds, so the button counts none.
        if (phone) FiltersPill(0, stringResource(R.string.filters_button), onFilters)
    }
}

/**
 * Every reading filter in the shows' filters sheet, live as theirs: Your sources only first while
 * there are words, then Genres to include or leave out, Started, Origin, Format and Status.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ReadFiltersSheet(onDismiss: () -> Unit, go: (ReadSearchQuery) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val search by store.reading.search.collectAsStateWithLifecycle()
    val query = search.query
    val filters = store.reading.filters
    // On a phone a chosen chip takes the neutral fill, as the shows' sheet has it.
    val phone = !wideWindow()
    fun change(edit: (ReadSearchQuery) -> ReadSearchQuery) = go(edit(store.reading.search.value.query))
    FiltersSheetFrame(onDismiss, canReset = query.clearedFilters() != query, onReset = { change { it.clearedFilters() } }) {
        if (query.text.isNotBlank()) {
            FilterSwitch(Strings.readSearchSourcesOnly(context), query.sourcesOnly) { on -> change { it.copy(sourcesOnly = on) } }
        }

        FilterHeading(Strings.readFilterGenres(context))
        FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (genre in filters.genres) {
                val pick = when (genre) {
                    in query.genres -> Pick.INCLUDE
                    in query.excludeGenres -> Pick.EXCLUDE
                    else -> Pick.OFF
                }
                PickChip(genre, pick) { change { it.cyclingGenre(genre) } }
            }
        }
        FilterNote(stringResource(R.string.filter_hint_touch))

        FilterHeading(Strings.readSearchStarted(context))
        FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (choice in filters.started.filter { it.key != STARTED_ANY }) {
                SearchChip(choice.label.text(context), query.started == choice.key, phone = phone) {
                    change { it.copy(started = if (it.started == choice.key) null else choice.key) }
                }
            }
        }

        FilterHeading(Strings.readFilterOrigin(context))
        FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (choice in filters.origins) {
                SearchChip(choice.label.text(context), choice.key in query.origins, phone = phone) { change { it.copy(origins = it.origins.toggling(choice.key)) } }
            }
        }

        FilterHeading(Strings.readFilterFormat(context))
        FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (choice in filters.formats.filter { it.key != FORMAT_ONE_SHOT }) {
                SearchChip(choice.label.text(context), choice.key in query.formats, phone = phone) { change { it.copy(formats = it.formats.togglingFormat(choice.key)) } }
            }
        }

        FilterHeading(Strings.readFilterStatus(context))
        FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for (choice in filters.statuses) {
                SearchChip(choice.label.text(context), query.status == choice.key, phone = phone) {
                    change { it.copy(status = if (it.status == choice.key) null else choice.key) }
                }
            }
        }

        FilterNote(stringResource(R.string.filter_live_note))
    }
}
