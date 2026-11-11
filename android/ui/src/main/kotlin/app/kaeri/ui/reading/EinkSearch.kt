// Search on e-ink, as the discovery board draws it: the field, Your sources only as a check box
// beside Filters, and a paged list of what was found, with the filters as a page of their own.
// Paper and ink, no motion: a page turn redraws the page.

package app.kaeri.ui.reading

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.sp
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.ReadSourceJoin
import app.kaeri.store.ReadingModel
import app.kaeri.store.AirTime
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
import androidx.compose.foundation.layout.width
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
import app.kaeri.store.skipsAnilist
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
    // A large panel's Search in: one source, or every source (null). The words go to that source
    // alone: it is the query's scope, so the engine asks no other.
    var only by rememberSaveable { mutableStateOf<String?>(null) }
    LaunchedEffect(text) {
        snapshotFlow { text.text.toString() }.drop(1).debounce(TYPING_REST_EINK).collect { typed ->
            val now = store.reading.search.value.query
            if (typed.trim() != now.text) {
                // Words cleared with nothing else set: back to the start, as the colour Search goes.
                val asked = now.copy(text = typed.trim(), sourceScope = only?.let { listOf(it) })
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
                Row(Modifier.fillMaxWidth().heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                        if (text.text.isEmpty()) Text(Strings.readSearchPlaceholder(context), style = EinkType.Body.copy(color = EinkColours.Secondary))
                        field()
                    }
                    // Clear at the field's end once there are words, as the large panel board draws it.
                    if (einkLarge && text.text.isNotEmpty()) {
                        Text(
                            Strings.actionClear(context),
                            style = EinkType.Label,
                            modifier = Modifier.einkClick { text.setTextAndPlaceCursorAtEnd("") }.padding(start = 12.dp),
                        )
                    }
                }
            },
        )
        Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            PaperCheck(Strings.readSearchSourcesOnly(context), search.query.sourcesOnly, Modifier.weight(1f)) {
                store.reading.runSearch(search.query.copy(sourcesOnly = !search.query.sourcesOnly))
            }
            EinkTextButton(Strings.readFilterTitle(context), { filters = true }, underline = true)
        }
        val large = einkLarge
        val lines = if (large) emptyList() else foundLines(context, search, opened)
        when {
            // Nothing asked: the start, not an empty search.
            search.query.isStart() -> if (large) WideStart(only, search.query.sourcesOnly) { only = it } else SearchStart()
            // A large panel: the results in columns under each place's name (WideResults below).
            large -> WideResults(search, only)
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

// The large panel (EinkSeries.kt's [einkLarge]), as the e-ink large panel board draws Search: the
// start lists where Search looks, AniList's genres on the grid's four columns and the formats;
// the results are rows of 60 × 90 covers in two columns upright and three on its side, your
// library first, then AniList's, then each source under its own name with its count. Nothing is
// ranked across places and nothing is suggested.

/** A result row's cover on the large panel. */
private val WideCover = 60.dp

/** A result row is its three lines and this, 110 dp at the least: the cover and 10 above and under it. */
private val WideRowPad = 22.dp
private val WideRowLeast = 110.dp

/** One line of the large results: a place's heading, a run of rows across the columns, or a note. */
private sealed interface Wide {
    data class Head(val text: String, val count: String?) : Wide
    data class Cells(val cells: List<WideCell>, val first: Boolean) : Wide
    data class Line(val line: Found) : Wide
    /** A heading with the line under it, so a page never ends on a heading. */
    data class Together(val lines: List<Wide>) : Wide
}

/** A result: its tile, its facts and where it is from. */
private data class WideCell(val tile: WorkTile, val facts: String, val from: String)

/** Each heading joined to the line after it, as one item to page. */
private fun List<Wide>.headsWithNext(): List<Wide> {
    val out = mutableListOf<Wide>()
    var head: Wide? = null
    for (line in this) {
        val held = head
        when {
            line is Wide.Head -> {
                held?.let { out += it }
                head = line
            }
            held != null -> {
                out += Wide.Together(listOf(held, line))
                head = null
            }
            else -> out += line
        }
    }
    head?.let { out += it }
    return out
}

/** The status among a tile's facts: "finished", "releasing". */
private fun WorkTile.statusFact(): Msg? = facts.firstOrNull {
    when ((it as? Msg.Reading)?.v1) {
        ReadingMsg.ReadingStatusReleasing, ReadingMsg.ReadingStatusFinished, ReadingMsg.ReadingStatusHiatus,
        ReadingMsg.ReadingStatusCancelled, ReadingMsg.ReadingStatusNotYet -> true
        else -> false
    }
}

/** How long a tile's series is among its facts: "34 chapters", "5 volumes". */
private fun WorkTile.lengthFact(): Msg? =
    facts.firstOrNull { (it as? Msg.Reading)?.v1 is ReadingMsg.ReadingChapters || (it as? Msg.Reading)?.v1 is ReadingMsg.ReadingVolumes }

/**
 * A result's facts in the board's order, kind, length, status: "Manga · 34 chapters · Releasing".
 * A source's hit says only what the source does: its format and how much it holds.
 */
private fun WorkTile.wideFacts(context: android.content.Context, hit: Boolean): String {
    val parts = if (hit) {
        listOfNotNull(formatLabel?.takeIf { it.isNotBlank() }?.replaceFirstChar { it.titlecase(AirTime.locale) }, amount?.text(context))
    } else {
        val length = lengthFact()
        val status = statusFact()
        val kind = facts.firstOrNull { it != length && it != status }
        listOfNotNull(kind?.text(context), length?.text(context), status?.text(context))
    }
    return parts.joinToString(" · ") // i18n-skip
}

/** "In Home library" for an AniList result a source has too; else AniList's name. */
private fun anilistFrom(context: android.content.Context, joined: ReadSourceJoin?): String = when {
    joined == null -> "AniList" // i18n-skip: a product name
    joined.sourceNames.size == 1 -> Strings.readSearchInSource(context, joined.sourceNames.first())
    else -> Strings.readSearchInSources(context, joined.sourceNames.size)
}

/** Rows of [cells], [columns] across, left to right and then down, as the board lays them. */
private fun MutableList<Wide>.addCells(cells: List<WideCell>, columns: Int) {
    cells.chunked(columns).forEachIndexed { index, run -> add(Wide.Cells(run, first = index == 0)) }
}

/** The results as lines: your library, AniList, then each source, every place under its own name. */
private fun wideLines(
    context: android.content.Context,
    search: ReadSearchState,
    library: List<Pair<WorkTile, LibrarySegmentRead>>,
    only: String?,
    columns: Int,
): List<Wide> = buildList {
    val sources = search.sources
    val query = search.query
    if (library.isNotEmpty()) {
        add(Wide.Head(Strings.readSearchGroupLibrary(context), Strings.readSearchGroupCount(context, library.size)))
        addCells(
            library.map { (tile, segment) ->
                WideCell(tile, tile.wideFacts(context, hit = false), listOfNotNull(listWords(context, segment), tile.next?.text(context)).joinToString(" · ")) // i18n-skip
            },
            columns,
        )
    }
    // A source picked under Search in leaves AniList out, as Your sources only does.
    if (!query.skipsAnilist) {
        val failed = (search.failure != null && search.tiles.isEmpty()) || sources?.anilist == ReadAnilistState.FAILED
        val count = (search.total?.toInt() ?: search.tiles.size).takeIf { search.tiles.isNotEmpty() }
        add(Wide.Head(Strings.readSearchGroupAnilist(context), count?.let { Strings.readSearchGroupCount(context, it) }))
        when {
            failed -> add(Wide.Line(Found.AnilistFailed(query)))
            search.tiles.isEmpty() && !search.loading -> add(Wide.Line(Found.Note(Strings.readSearchAnilistNothing(context))))
            search.tiles.isEmpty() -> add(Wide.Line(Found.Asking("AniList"))) // i18n-skip: a product name
            else -> addCells(
                search.tiles.map { tile ->
                    WideCell(tile, tile.wideFacts(context, hit = false), anilistFrom(context, sources?.joins?.firstOrNull { it.work == tile.work }))
                },
                columns,
            )
        }
        if (search.hasMore) add(Wide.Line(Found.More(search.tiles.size, search.total)))
    }
    val groups = sources?.groups.orEmpty().filter { only == null || it.sourceId == only }
    for (group in groups.filter { it.state !is ReadSourceState.Nothing && !(it.state is ReadSourceState.Hits && it.tiles.isEmpty()) }) {
        val hits = group.tiles.size.takeIf { group.state is ReadSourceState.Hits }
        add(Wide.Head(Strings.readSearchGroupSource(context, group.sourceName), hits?.let { Strings.readSearchGroupCount(context, it) }))
        when (val state = group.state) {
            is ReadSourceState.Asking -> add(Wide.Line(Found.Asking(group.sourceName)))
            is ReadSourceState.Failed -> add(Wide.Line(Found.Failed(group, failureWords(context, state.failure))))
            is ReadSourceState.TimedOut -> add(Wide.Line(Found.Failed(group, Strings.readSearchSourceTimeout(context, state.seconds.toInt()))))
            else -> addCells(group.tiles.map { WideCell(it, it.wideFacts(context, hit = true), group.sourceName) }, columns)
        }
    }
    val nothing = groups.filter { it.state is ReadSourceState.Nothing }
    if (nothing.isNotEmpty()) add(Wide.Line(Found.Note(Strings.readSearchSourceNothing(context, joinedNames(context, nothing.map { it.sourceName })), ruled = true)))
}.headsWithNext()

/** How many list columns the page holds, from the panel's width less its gutters. */
@Composable
private fun wideColumns(): Int = paperListColumns(LocalConfiguration.current.screenWidthDp.dp - Side * 2)

/**
 * The large panel's results. Your library's part is the series in your lists whose title has the
 * words, or that a place below found: the engine's search does not look in the library itself.
 */
@Composable
private fun WideResults(search: ReadSearchState, only: String?) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val lists by store.reading.lists.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { READ_LISTS.filter { it !in store.reading.lists.value }.forEach { store.reading.loadList(it) } }
    var page by rememberSaveable(search.query) { mutableIntStateOf(0) }
    val columns = wideColumns()
    val words = search.query.text.trim()
    val found = buildSet {
        search.tiles.forEach { add(it.work) }
        search.sources?.groups.orEmpty().forEach { group -> group.tiles.forEach { add(it.work) } }
    }
    val library = READ_LISTS.flatMap { segment ->
        lists[segment].orEmpty().filter { (words.isNotEmpty() && it.title.contains(words, ignoreCase = true)) || it.work in found }.map { it to segment }
    }.distinctBy { it.first.work }
    val lines = wideLines(context, search, library, only, columns)
    val rowHeight = wideRowHeight()
    val groups = search.sources?.groups.orEmpty()
    val failed = search.failure != null || search.sources?.anilist == ReadAnilistState.FAILED ||
        groups.any { it.state is ReadSourceState.Failed || it.state is ReadSourceState.TimedOut }
    val nothing = !failed && library.isEmpty() && search.tiles.isEmpty() && groups.none { it.tiles.isNotEmpty() }
    when {
        nothing && search.loading -> PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp)) }
        nothing && groups.none { it.state is ReadSourceState.Asking } -> Column(Modifier.padding(top = 16.dp)) {
            if (words.isEmpty()) {
                Text(Strings.readSearchNothingBrowse(context), style = EinkType.Body)
                Text(Strings.readSearchNothingFiltersLine(context), style = EinkType.Secondary)
            } else {
                Text(Strings.readSearchNothing(context, words), style = EinkType.Body)
                Text(Strings.readSearchNothingLine(context), style = EinkType.Secondary)
            }
        }
        else -> PagedItems(lines, page, { page = it }, Modifier.padding(top = 4.dp)) { line -> WideLine(line, columns, rowHeight) }
    }
}

/** A result row's height at the text size in use: the title at 18, two lines at 15 and 22 dp. */
@Composable
private fun wideRowHeight(): Dp {
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val probe = "Ag" // i18n-skip: a measuring probe
    val lines = with(density) {
        measurer.measure(probe, WideTitle).size.height.toDp() + measurer.measure(probe, EinkType.Secondary).size.height.toDp() * 2
    }
    return maxOf(WideRowLeast, lines + WideRowPad)
}

/** A result's title: 18 in SemiBold, as the board's rows. */
private val WideTitle = EinkType.Label.copy(fontSize = 18.sp)

@Composable
private fun WideLine(line: Wide, columns: Int, rowHeight: Dp) {
    when (line) {
        is Wide.Head -> Row(Modifier.fillMaxWidth().heightIn(min = 52.dp).padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(line.text, style = EinkType.RowTitle, modifier = Modifier.weight(1f).semantics { heading() })
            line.count?.let { Text(it, style = EinkType.Secondary, modifier = Modifier.padding(start = 12.dp)) }
        }
        is Wide.Cells -> Row(Modifier.fillMaxWidth().height(rowHeight)) {
            for (column in 0 until columns) {
                val cell = line.cells.getOrNull(column)
                // No rule beside an empty cell: the gap stays, so the columns keep their width.
                if (column > 0) {
                    if (cell == null) Spacer(Modifier.width(PaperListGap)) else ColumnRule()
                }
                Box(Modifier.weight(1f).fillMaxHeight()) {
                    if (cell != null) WideResult(cell, ruled = !line.first)
                }
            }
        }
        is Wide.Line -> FoundLine(line.line) {}
        is Wide.Together -> Column { line.lines.forEach { WideLine(it, columns, rowHeight) } }
    }
}

/** A result on the large panel: the 60 × 90 cover, the title, its facts, where it is from, a chevron. */
@Composable
private fun WideResult(cell: WideCell, ruled: Boolean) {
    val navigator = LocalNavigator.current
    val tile = cell.tile
    Row(
        Modifier
            .fillMaxSize()
            .topHairline(ruled)
            .einkClick { navigator.openSeries(tile.work, "eink-search") }
            .semantics(mergeDescendants = true) {},
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ReadCover(tile.cover, WideCover, Modifier.border(EinkSizes.Hairline, EinkColours.Ink), shape = RoundedCornerShape(0.dp), title = tile.title)
        Column(Modifier.weight(1f).padding(start = 16.dp)) {
            Text(tile.title, style = WideTitle, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (cell.facts.isNotEmpty()) Text(cell.facts, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(cell.from, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Text("›", style = EinkType.Button, modifier = Modifier.padding(start = 12.dp)) // i18n-skip
    }
}

/** One of the start's pieces, paged as one item each. */
private sealed interface StartPart {
    data class Head(val text: String, val side: String? = null) : StartPart
    data class Places(val places: List<Pair<String?, String>>) : StartPart
    data class Genres(val genres: List<String>) : StartPart
    data class Formats(val formats: List<ReadFilterChoice>) : StartPart
    data class Note(val text: String) : StartPart
    data class Together(val parts: List<StartPart>) : StartPart
}

/** Between the start's chips, across and down. */
private val ChipGap = 24.dp

/** A genre's column: four on the 10.3 inch panel upright, as the board's grid. */
private val GenreLeast = 200.dp

/**
 * The large panel's start: Search in, naming only the sources you added; AniList's genres on the
 * grid's four columns; the formats; and a line on where Search looks, AniList left out with Your
 * sources only. Kaeri keeps no recent reading searches, so the board's Recent searches is not drawn.
 */
@Composable
private fun WideStart(only: String?, sourcesOnly: Boolean, onOnly: (String?) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val sources by store.bookSources.view.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { if (store.bookSources.view.value == null) store.bookSources.load() }
    var page by rememberSaveable { mutableIntStateOf(0) }
    val view = sources
    // Folders are one source to search; then each server and plugin that is on, in your order.
    val places = buildList {
        if (view != null) {
            if (view.folders.any { !it.removed }) add(READ_FOLDERS to Strings.readSourceFolders(context))
            view.servers.filter { !it.off }.forEach { add(it.id to it.name) }
            view.plugins.filter { !it.off }.forEach { add(it.id to it.name) }
        }
    }.sortedBy { (id, _) -> view?.order?.indexOf(id)?.takeIf { it >= 0 } ?: Int.MAX_VALUE }
    val filters = store.reading.filters
    val columns = ((LocalConfiguration.current.screenWidthDp.dp - Side * 2 + ChipGap) / (GenreLeast + ChipGap)).toInt().coerceAtLeast(2)
    val parts = buildList {
        if (places.isNotEmpty()) {
            add(StartPart.Together(listOf(StartPart.Head(Strings.readSearchIn(context)), StartPart.Places(listOf<Pair<String?, String>>(null to Strings.readSearchInAll(context)) + places))))
        }
        filters.genres.chunked(columns).forEachIndexed { index, run ->
            add(if (index == 0) StartPart.Together(listOf(StartPart.Head(Strings.readFilterGenres(context), Strings.readSearchGenresFrom(context)), StartPart.Genres(run))) else StartPart.Genres(run))
        }
        if (filters.formats.isNotEmpty()) add(StartPart.Together(listOf(StartPart.Head(Strings.readSearchFormats(context)), StartPart.Formats(filters.formats))))
        // Your sources only leaves AniList out, and the line says so.
        add(StartPart.Note(if (sourcesOnly) Strings.readSearchStartNoteOwn(context) else Strings.readSearchStartNote(context)))
    }
    PagedItems(parts, page, { page = it }, Modifier.padding(top = 4.dp)) { part -> StartLine(part, columns, only, onOnly) }
}

/** The Folders source's id, as the engine names it. */
private const val READ_FOLDERS = "folders"

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun StartLine(part: StartPart, columns: Int, only: String?, onOnly: (String?) -> Unit) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    when (part) {
        is StartPart.Head -> Row(Modifier.fillMaxWidth().heightIn(min = 52.dp).padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(part.text, style = EinkType.RowTitle, modifier = Modifier.weight(1f).semantics { heading() })
            part.side?.let { Text(it, style = EinkType.Secondary, modifier = Modifier.padding(start = 12.dp)) }
        }
        is StartPart.Places -> androidx.compose.foundation.layout.FlowRow(
            Modifier.fillMaxWidth().padding(bottom = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            part.places.forEach { (id, name) -> StartChip(name, chosen = id == only, height = 48.dp) { onOnly(id) } }
        }
        is StartPart.Genres -> Row(Modifier.fillMaxWidth().padding(bottom = 12.dp), horizontalArrangement = Arrangement.spacedBy(ChipGap)) {
            for (column in 0 until columns) {
                val genre = part.genres.getOrNull(column)
                Box(Modifier.weight(1f)) {
                    if (genre != null) {
                        StartChip(genre, chosen = false, height = 52.dp, wide = true) {
                            store.reading.runSearch(ReadingModel.EMPTY_QUERY.copy(genres = listOf(genre)))
                        }
                    }
                }
            }
        }
        is StartPart.Formats -> androidx.compose.foundation.layout.FlowRow(
            Modifier.fillMaxWidth().padding(bottom = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            part.formats.forEach { format ->
                StartChip(format.label.text(context), chosen = false, height = 48.dp) {
                    store.reading.runSearch(ReadingModel.EMPTY_QUERY.copy(formats = listOf(format.key)))
                }
            }
        }
        is StartPart.Note -> Text(part.text, style = EinkType.Secondary, modifier = Modifier.padding(top = 8.dp))
        is StartPart.Together -> Column { part.parts.forEach { StartLine(it, columns, only, onOnly) } }
    }
}

/** A chip on paper: a 2 dp ink border, 10 dp corners, light grey when chosen. */
@Composable
private fun StartChip(text: String, chosen: Boolean, height: Dp, wide: Boolean = false, onClick: () -> Unit) {
    Box(
        Modifier
            .then(if (wide) Modifier.fillMaxWidth() else Modifier)
            .heightIn(min = height)
            .clip(RoundedCornerShape(EinkSizes.SegmentRadius))
            .background(if (chosen) EinkColours.Selected else EinkColours.Paper)
            .border(EinkSizes.Rule, EinkColours.Ink, RoundedCornerShape(EinkSizes.SegmentRadius))
            .einkClick(onClick = onClick)
            .semantics { selected = chosen }
            .padding(horizontal = if (wide) 18.dp else 18.dp),
        contentAlignment = if (wide) Alignment.CenterStart else Alignment.Center,
    ) {
        Text(text, style = EinkType.Label.copy(fontSize = 16.sp), maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}
