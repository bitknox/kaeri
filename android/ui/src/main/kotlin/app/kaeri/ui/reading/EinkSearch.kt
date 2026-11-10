// Search on e-ink, as the discovery board draws it: the field, Your sources only as a check box
// beside Filters, and a paged list of what was found, with the filters as a page of their own.
// Paper and ink, no motion: a page turn redraws the page.

package app.kaeri.ui.reading

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.material3.Icon
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkLook
import app.kaeri.eink.EinkSizes
import app.kaeri.eink.EinkSolidButton
import app.kaeri.eink.EinkTextButton
import app.kaeri.eink.EinkTheme
import app.kaeri.eink.EinkType
import app.kaeri.eink.PaperWaiting
import app.kaeri.eink.einkClick
import app.kaeri.eink.hairline
import app.kaeri.eink.topRule
import app.kaeri.engine.model.ReadAnilistState
import app.kaeri.engine.model.ReadFilterChoice
import app.kaeri.engine.model.ReadRowKind
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

/** A cover in the e-ink lists. */
private val ListCover = 40.dp

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
            if (typed.trim() != now.text) {
                // Words cleared with nothing else set: back to the start, as the colour Search goes.
                val asked = now.copy(text = typed.trim())
                if (asked.isStart() && !asked.sourcesOnly) store.reading.searchStart() else store.reading.runSearch(asked)
            }
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
                .border(EinkSizes.Rule, EinkColours.Ink, RoundedCornerShape(EinkSizes.SegmentRadius))
                .padding(horizontal = 16.dp, vertical = 2.dp),
            decorator = { field ->
                // The words and the placeholder at the field's size, centred in its 52 dp.
                Box(Modifier.fillMaxWidth().heightIn(min = 48.dp), contentAlignment = Alignment.CenterStart) {
                    if (text.text.isEmpty()) Text(Strings.readSearchPlaceholder(context), style = EinkType.Body.copy(color = EinkColours.Secondary))
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
            // Nothing asked: the start, not an empty search.
            search.query.isStart() -> SearchStart()
            // After 600 ms one word; most searches answer first and cost the panel nothing.
            (search.query.text.isBlank() && search.tiles.isEmpty() && search.loading) || (lines.isEmpty() && search.loading) ->
                PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp)) }
            // Browsing by filters with no words that found nothing says so, not "Nothing found for".
            lines.isEmpty() && search.query.text.isBlank() -> Column(Modifier.padding(top = 16.dp)) {
                Text(Strings.readSearchNothingBrowse(context), style = EinkType.Body)
                Text(Strings.readSearchNothingFiltersLine(context), style = EinkType.Secondary)
            }
            lines.isEmpty() -> Column(Modifier.padding(top = 16.dp)) {
                Text(Strings.readSearchNothing(context, search.query.text), style = EinkType.Body)
                Text(Strings.readSearchNothingLine(context), style = EinkType.Secondary)
            }
            else -> PagedItems(lines, page, { page = it }, Modifier.padding(top = 8.dp)) { line -> FoundLine(line) { opened = opened + it } }
        }
    }
    if (filters) PaperFilters(search.query) { filters = false }
}

/**
 * Search with nothing asked: Home's discovery rows as the colour Search's start shows them, each
 * opening its grid, paged as the Browse list is.
 */
@Composable
private fun SearchStart() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val home by store.reading.home.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { if (store.reading.home.value == null) store.reading.loadHome() }
    var page by rememberSaveable { mutableIntStateOf(0) }
    // Catalogue-wide, as the colour start: Because you finished stays on Home.
    val rows = home?.rows.orEmpty().filter { it.kind != ReadRowKind.BECAUSE && it.tiles.isNotEmpty() }
    when {
        rows.isNotEmpty() -> Column(Modifier.padding(top = 8.dp)) {
            Text(Strings.readHomeBrowse(context), style = EinkType.RowTitle, modifier = Modifier.padding(top = 8.dp, bottom = 4.dp).semantics { heading() })
            PagedItems(rows, page, { page = it }) { row -> BrowseLine(row) { navigator.openReadPage(READ_PAGE_ROW + row.id) } }
        }
        home == null || home?.rows.orEmpty().any { it.pending } ->
            PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp)) }
    }
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
