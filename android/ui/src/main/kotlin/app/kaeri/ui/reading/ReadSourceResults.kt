// Search with words, as the discovery board redrew it: AniList's results as the browse grid with a
// two-line caption, a joined cover saying which source has it; then From your sources, one
// sideways row of covers per source with its name and kind, each landing in its own slot; the
// sources that found nothing share one line at the end; More from AniList under them. Your
// sources only hides AniList's part so the rows lead.

package app.kaeri.ui.reading

import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridItemScope
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.layout.layout
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import app.kaeri.design.Motion
import app.kaeri.design.rememberSystemReducedMotion
import app.kaeri.design.tabular
import app.kaeri.engine.model.ReadAnilistState
import app.kaeri.engine.model.ReadSourceGroup
import app.kaeri.engine.model.ReadSourceKind
import app.kaeri.engine.model.ReadSourceJoin
import app.kaeri.engine.model.ReadSourceState
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.Strings
import app.kaeri.store.ReadSearchState
import app.kaeri.store.ReadingModel
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore

/** AniList's part leaving when Your sources only turns on: 200 ms. */
private const val ANILIST_OUT = 200

/** Covers fading in over their placeholders: 200 ms, ease out. */
private const val COVERS_IN = 200

/** Placeholders fading out once the covers over them are opaque: 300 ms, after the covers' 200. */
private const val PLACEHOLDERS_OUT = 300

/**
 * Where a grid item of Search goes: AniList's covers fade out in 200 ms, placeholders fade out
 * over 300 ms once the covers over them are in, the rest glide.
 */
@Composable
internal fun LazyGridItemScope.searchMotion(anilist: Boolean = false, placeholder: Boolean = false): Modifier {
    val reduced = rememberSystemReducedMotion()
    return Modifier.animateItem(
        fadeInSpec = if (placeholder) Motion.fadeIn() else tween(COVERS_IN, easing = LinearOutSlowInEasing),
        placementSpec = if (reduced) null else Motion.standard(),
        fadeOutSpec = when {
            placeholder -> tween(PLACEHOLDERS_OUT, delayMillis = COVERS_IN, easing = LinearOutSlowInEasing)
            anilist -> tween(ANILIST_OUT, easing = LinearOutSlowInEasing)
            else -> Motion.fadeOut()
        },
    )
}

/** The grid's placeholders for [count] cells, as covers or, at the largest text, as list rows. */
internal fun LazyGridScope.placeholderCells(count: Int, list: Boolean, lines: Int, prefix: String) {
    items(count, key = { "$prefix-$it" }, contentType = { "placeholder" }) {
        // Hidden from screen readers: the heading or the field says Search is busy.
        Box(searchMotion(placeholder = true).clearAndSetSemantics {}) {
            if (list) ListRowPlaceholder() else app.kaeri.ui.PosterPlaceholder(titleLines(), lines)
        }
    }
}

/**
 * Search's results with words. [both] adds the count of matches in Shows; [demoOnly] the demo's
 * note. What is drawn is [shown]: the answer before new words stays until theirs lands, and
 * placeholders show only where nothing is, from 600 ms ([ShownSearch.waited]). Browsing without
 * words stays in [ReadSearchScreen].
 */
internal fun LazyGridScope.wordsResults(shown: ShownSearch, both: Boolean, demoOnly: Boolean, grid: ReadGridShape) {
    val search = shown.search
    val query = search.query
    val sources = search.sources
    val sourcesOnly = query.sourcesOnly
    val anilist = when {
        sourcesOnly -> ReadAnilistState.SKIPPED
        shown.failureShown != null -> ReadAnilistState.FAILED
        search.loading && search.tiles.isEmpty() -> ReadAnilistState.ASKING
        sources?.anilist == ReadAnilistState.FAILED && !search.stale -> ReadAnilistState.FAILED
        else -> ReadAnilistState.ANSWERED
    }
    // AniList's covers count only while its part shows: hidden by Your sources only, they are not found.
    val anilistTiles = if (sourcesOnly) emptyList() else search.tiles
    val groups = sources?.groups.orEmpty()
    val shownGroups = groups.filter { it.shows() }
    val nothing = groups.filter { it.state is ReadSourceState.Nothing }
    // Every part settled with nothing at all: one Nothing found, as before the sources.
    val nothingAnywhere = !search.loading && anilist != ReadAnilistState.ASKING && anilist != ReadAnilistState.FAILED &&
        anilistTiles.isEmpty() && shownGroups.isEmpty() && (sources == null || sources.done || groups.isEmpty())
    // Kept from the words before and asked for 600 ms: a small ring beside the count says so.
    val asking = shown.waited && search.stale

    when {
        nothingAnywhere -> item(span = { GridItemSpan(maxLineSpan) }, key = "nothing", contentType = "quiet") {
            Box(searchMotion()) { NothingAnywhere(search, eitherLine = groups.isNotEmpty()) }
        }
        anilist == ReadAnilistState.FAILED -> item(span = { GridItemSpan(maxLineSpan) }, key = "anilist-failed", contentType = "quiet") {
            Box(searchMotion(anilist = true)) { AnilistFailed(search) }
        }
        anilist == ReadAnilistState.ASKING -> {
            // The count's line holds its place from the start, so the covers land where their placeholders were.
            countLine(null, asking = false)
            if (shown.waited) placeholderCells(grid.columns * 2, grid.list, 1, "a-wait")
        }
        anilist == ReadAnilistState.ANSWERED && search.tiles.isEmpty() ->
            item(span = { GridItemSpan(maxLineSpan) }, key = "anilist-nothing", contentType = "quiet") {
                Box(searchMotion(anilist = true)) { AnilistNothing() }
            }
        anilist == ReadAnilistState.ANSWERED -> {
            countLine(search, asking)
            val first = search.tiles.take(search.firstPage.coerceAtLeast(1))
            items(first, key = { "a-${it.work}" }, contentType = { "cover" }) { tile ->
                Box(searchMotion(anilist = true)) { ResultCell(tile, sources?.joins, grid) }
            }
        }
        else -> {}
    }

    if (shownGroups.isNotEmpty()) {
        // Its own item, so it stays while the first source's row leaves or is asked again.
        item(span = { GridItemSpan(maxLineSpan) }, key = "src-heading", contentType = "heading") {
            val context = LocalContext.current
            // A heading as the shows' Search start has them; the grid's gap under it is taken back,
            // so it sits over its first row as a rail's heading does.
            Box(searchMotion().pullUp(16.dp)) { app.kaeri.ui.StartHeading(Strings.readSearchFromSources(context)) }
        }
        shownGroups.forEach { group ->
            item(span = { GridItemSpan(maxLineSpan) }, key = "src-${group.sourceId}", contentType = "source") {
                Box(searchMotion()) { SourceGroupRow(group, again = group.sourceId in search.staleSources, waited = shown.waited) }
            }
        }
    }
    if (nothing.isNotEmpty() && !nothingAnywhere) {
        item(span = { GridItemSpan(maxLineSpan) }, key = "src-nothing", contentType = "line") {
            Box(searchMotion()) { SourcesNothingLine(nothing) }
        }
    }

    // More from AniList sits under the sources; its pages come below it.
    if (anilist == ReadAnilistState.ANSWERED && search.tiles.isNotEmpty()) {
        val later = search.tiles.drop(search.firstPage)
        if (later.isEmpty() && search.hasMore) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "more-anilist", contentType = "more") {
                Box(searchMotion(anilist = true)) { MoreFromAnilist(search.loading, search.tiles.size, search.total) }
            }
        } else if (later.isNotEmpty()) {
            items(later, key = { "a-${it.work}" }, contentType = { "cover" }) { tile ->
                Box(searchMotion(anilist = true)) { ResultCell(tile, sources?.joins, grid) }
            }
            if (search.hasMore) {
                item(span = { GridItemSpan(maxLineSpan) }, key = "more-${search.page}", contentType = "more") {
                    val store = LocalAppStore.current
                    LaunchedEffect(search.page) { store.reading.searchMore() }
                    Box(Modifier.fillMaxWidth().heightIn(min = 48.dp), contentAlignment = Alignment.Center) {
                        if (search.loading) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                    }
                }
            }
        }
    }
    if (both && !sourcesOnly) {
        item(span = { GridItemSpan(maxLineSpan) }, key = "other", contentType = "line") { Box(searchMotion()) { OtherScopeRow(search) } }
    }
    if (demoOnly) {
        item(span = { GridItemSpan(maxLineSpan) }, key = "demo", contentType = "line") { Box(searchMotion()) { DemoSearchNote() } }
    }
}

/**
 * The count over AniList's covers: "4 series from AniList" when the engine knows the count
 * exactly, else "Series from AniList", never a number that is not the count. While AniList is
 * asked with nothing shown ([search] null) the line is empty but holds its height; while the
 * covers are kept from the words before, a small ring beside it says they are being asked again.
 */
private fun LazyGridScope.countLine(search: ReadSearchState?, asking: Boolean) {
    item(span = { GridItemSpan(maxLineSpan) }, key = "count", contentType = "line") {
        val context = LocalContext.current
        val total = search?.total
        val words = when {
            search == null || search.tiles.all { it.fromDemo } -> ""
            total != null -> Strings.readSearchTotal(context, total.toInt())
            else -> Strings.readSearchTotalUnknown(context)
        }
        Row(searchMotion(), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            app.kaeri.design.SwapFade(words, animationSpec = tween(COVERS_IN, easing = LinearOutSlowInEasing), label = "count") { said -> // i18n-skip
                // The shows' caption style over the grid.
                Text(
                    said,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            androidx.compose.animation.AnimatedVisibility(asking, enter = androidx.compose.animation.fadeIn(Motion.fadeIn()), exit = androidx.compose.animation.fadeOut(Motion.fadeOut())) {
                CircularProgressIndicator(Modifier.size(12.dp), strokeWidth = 1.5.dp, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** Draws as it is but takes [by] less room below, so what comes next sits closer. */
private fun Modifier.pullUp(by: androidx.compose.ui.unit.Dp): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints)
    layout(placeable.width, (placeable.height - by.roundToPx()).coerceAtLeast(0)) { placeable.place(0, 0) }
}

/** Whether a source's group takes a row: asking, failed, or with hits that joined nothing. */
private fun ReadSourceGroup.shows(): Boolean = when (state) {
    is ReadSourceState.Nothing -> false
    is ReadSourceState.Hits -> tiles.isNotEmpty()
    else -> true
}

/** A source's kind in words beside its name: Server, Folder or Plugin; a built-in one has none. */
@Composable
internal fun sourceKind(group: ReadSourceGroup): String? {
    val context = LocalContext.current
    return when (group.kind) {
        ReadSourceKind.SERVER -> Strings.readSearchSourceKindServer(context)
        ReadSourceKind.FOLDER -> Strings.readSearchSourceKindFolder(context)
        ReadSourceKind.PLUGIN -> Strings.readSearchSourceKindPlugin(context)
        null -> null
    }
}

/** An AniList result in the grid, with "In Home library" under its caption when a source has it. */
@Composable
private fun ResultCell(tile: WorkTile, joins: List<ReadSourceJoin>?, grid: ReadGridShape) {
    val context = LocalContext.current
    val join = joins?.firstOrNull { it.work == tile.work }
    val extra = join?.let {
        if (it.sourceNames.size == 1) Strings.readSearchInSource(context, it.sourceNames.first())
        else Strings.readSearchInSources(context, it.sourceNames.size)
    }
    var menu by remember { mutableStateOf(false) }
    BoxWithConstraints {
        ReadTile(
            tile,
            maxWidth,
            from = "read-search",
            // The board's two-line caption: the title, then "Format · Origin".
            lines = 1,
            caption = tile.anilistLine(context),
            spoken = tile.anilistSpoken(context),
            reserveLine = true,
            onLongClick = { menu = true },
            extra = extra,
            list = grid.list,
            planning = true,
            titleMax = 2,
        )
        ReadTileMenu(tile, menu, current = null) { menu = false }
    }
}

/** A source group shows this many hits, then "Show N more". */
internal const val SOURCE_HITS_SHOWN = 5

/**
 * One source's row: its name and kind, then its covers, a failure with Try again, or placeholders.
 * [again]: what it shows is from the words before, kept while it is asked for these; past 600 ms
 * ([waited]) the ring beside its name says so, and its answer crossfades in over the old one.
 */
@Composable
private fun SourceGroupRow(group: ReadSourceGroup, again: Boolean, waited: Boolean) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val kind = sourceKind(group)
    val asking = group.state is ReadSourceState.Asking
    val busy = asking || again
    val reduced = rememberSystemReducedMotion()
    val kept = remember { arrayOf(group) }
    if (!asking) kept[0] = group
    val answer = kept[0]
    // "Northwind, plugin, searching" while it is asked; "Home library, server" once it answered.
    val label = when {
        busy && kind != null -> Strings.readSearchSourceBusyKind(context, group.sourceName, kind)
        busy -> Strings.readSearchSourceBusy(context, group.sourceName)
        kind != null -> Strings.readSearchSourceNamed(context, group.sourceName, kind)
        else -> group.sourceName
    }
    Column {
        // Headed as the shows' Search rails are: the name, and what it is beside it in haze.
        Row(
            Modifier
                .fillMaxWidth()
                .semantics(mergeDescendants = true) {
                    heading()
                    contentDescription = label
                },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                group.sourceName,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                modifier = Modifier.weight(1f, fill = false).padding(vertical = 12.dp),
            )
            if (kind != null) Text(kind, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1)
            androidx.compose.animation.AnimatedVisibility(asking || (again && waited), enter = androidx.compose.animation.fadeIn(Motion.fadeIn()), exit = androidx.compose.animation.fadeOut(Motion.fadeOut())) {
                CircularProgressIndicator(Modifier.size(14.dp), strokeWidth = 2.dp, color = colors.onSurfaceVariant)
            }
        }
        // The slot keeps its place while asking; a failure closes it to its height on the spring.
        LandingSlot(
            landed = !asking,
            modifier = if (reduced) Modifier else Modifier.animateContentSize(Motion.standard()),
            placeholders = { RailCoversPlaceholder() },
        ) {
            // A new answer over the one kept from the words before: one crossfade in the same slot.
            // Asked again by its Try again, the last answer stays until the slot's placeholders take it.
            app.kaeri.design.SwapFade(answer.state to answer.tiles, animationSpec = tween(COVERS_IN, easing = LinearOutSlowInEasing), label = "source answer") { (state, tiles) -> // i18n-skip
                when (state) {
                    is ReadSourceState.Failed -> SourceFailed(group, failureWords(context, state.failure), null)
                    is ReadSourceState.TimedOut -> SourceFailed(
                        group,
                        Strings.readSearchSourceTimeout(context, state.seconds.toInt()),
                        Strings.readSearchSourceTimeoutSpoken(context, state.seconds.toInt()),
                    )
                    // Source hits: no menu and no Add to Planning, as the board has them.
                    else -> TileStrip(tiles, "read-src-${group.sourceId}", rowKey = group.sourceId, planning = false, cap = SOURCE_HITS_SHOWN, rail = true) { hit ->
                        hit.hitLine(context).let { RowCaption(it, it) }
                    }
                }
            }
        }
    }
}

/** A rail's covers on their way: as many 104 dp placeholders as the width shows, and one more past the edge. */
@Composable
private fun RailCoversPlaceholder() {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val count = ((maxWidth + app.kaeri.ui.RAIL_GAP) / (app.kaeri.ui.RAIL_COVER + app.kaeri.ui.RAIL_GAP)).toInt() + 1
        Row(
            Modifier.wrapContentWidth(Alignment.Start, unbounded = true),
            horizontalArrangement = Arrangement.spacedBy(app.kaeri.ui.RAIL_GAP),
        ) {
            repeat(count.coerceAtLeast(1)) { Box(Modifier.width(app.kaeri.ui.RAIL_COVER)) { app.kaeri.ui.PosterPlaceholder(titleLines(), 1) } }
        }
    }
}

/** A source that didn't answer: the glyph, its name, the reason and Try again for it alone. */
@Composable
private fun SourceFailed(group: ReadSourceGroup, reason: String, spoken: String?, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    FailureNotice(
        title = Strings.readSearchSourceFailed(context, group.sourceName),
        line = reason,
        spokenLine = spoken,
        action = Strings.actionTryAgain(context),
        // Named with its source, so a list of them makes sense: "Try again, Northwind".
        actionSpoken = Strings.readSearchTryAgainSource(context, group.sourceName),
        onAction = { store.reading.retrySource(group.sourceId) },
        modifier = modifier,
    )
}

/** The vermilion glyph, a sentence and its reason, and a button under them. */
@Composable
internal fun FailureNotice(
    title: String,
    line: String?,
    action: String,
    onAction: () -> Unit,
    modifier: Modifier = Modifier,
    /** The reason as said, where it differs from what shows: "No answer in 15 seconds". */
    spokenLine: String? = null,
    /** The button as said, where it needs more than its words. */
    actionSpoken: String? = null,
) {
    val colors = MaterialTheme.colorScheme
    Row(modifier.padding(vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Icon(Icons.ProblemFilled, contentDescription = null, tint = colors.error, modifier = Modifier.padding(top = 2.dp).size(20.dp))
        Column(Modifier.weight(1f)) {
            Column(Modifier.semantics(mergeDescendants = true) {}) {
                Text(title, style = MaterialTheme.typography.bodyLarge, color = colors.onSurface)
                if (line != null) {
                    Text(
                        line,
                        style = MaterialTheme.typography.bodyMedium,
                        color = colors.onSurfaceVariant,
                        modifier = if (spokenLine != null) Modifier.semantics { contentDescription = spokenLine } else Modifier,
                    )
                }
            }
            OutlinedButton(
                onClick = onAction,
                modifier = Modifier.padding(top = 12.dp).then(if (actionSpoken != null) Modifier.semantics { contentDescription = actionSpoken } else Modifier),
                colors = ButtonDefaults.outlinedButtonColors(contentColor = colors.onSurface),
            ) { Text(action) }
        }
    }
}

/** AniList didn't answer: the sources were still asked, and Try again asks it again. */
@Composable
private fun AnilistFailed(search: ReadSearchState) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    // As the shows' Search says a failure: the line, then Try again.
    app.kaeri.ui.SearchFailed(Strings.readSearchAnilistFailedLine(context), onRetry = { store.reading.runSearch(search.query) })
}

/** AniList found nothing, while the sources are still asked or found something. */
@Composable
private fun AnilistNothing() {
    val context = LocalContext.current
    app.kaeri.ui.SearchNothing(Strings.readSearchAnilistNothing(context), onClearFilters = null)
}

/**
 * Nothing anywhere: Nothing found for the words, and the sources' line when there are any. With
 * Your sources only on it says so: Nothing found in your sources, and to try other words.
 */
@Composable
private fun NothingAnywhere(search: ReadSearchState, eitherLine: Boolean) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val query = search.query
    val filtered = query.formats.isNotEmpty() || query.origins.isNotEmpty() || query.status != null ||
        query.genres.isNotEmpty() || query.excludeGenres.isNotEmpty() || (query.started != null && query.started != "any") // i18n-skip
    Column {
        // As the shows' Search says nothing found: the line, then Clear filters when any are set.
        when {
            query.sourcesOnly -> app.kaeri.ui.SearchNothing(Strings.readSearchSourcesOnlyNothing(context), onClearFilters = null)
            else -> app.kaeri.ui.SearchNothing(
                Strings.readSearchNothing(context, query.text),
                onClearFilters = if (filtered) ({ store.reading.runSearch(query.clearedFilters().copy(sourcesOnly = query.sourcesOnly)) }) else null,
            )
        }
        if (eitherLine && !query.sourcesOnly) {
            Text(
                Strings.readSearchSourcesNothing(context),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 8.dp),
            )
        }
    }
}

/** The sources that found nothing, named in one line at the end. */
@Composable
private fun SourcesNothingLine(groups: List<ReadSourceGroup>) {
    val context = LocalContext.current
    val names = joinedNames(context, groups.map { it.sourceName })
    Column(Modifier.padding(top = 16.dp)) {
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
        Text(
            Strings.readSearchSourceNothing(context, names),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 16.dp),
        )
    }
}

/** Names as the language lists them: "A, B and C". */
internal fun joinedNames(context: android.content.Context, names: List<String>): String =
    android.icu.text.ListFormatter.getInstance(context.resources.configuration.locales[0]).format(names)

/** More from AniList: its next page, under the sources. */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun MoreFromAnilist(loading: Boolean, shown: Int, total: UInt?) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    androidx.compose.foundation.layout.FlowRow(
        Modifier.fillMaxWidth().padding(top = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        itemVerticalAlignment = Alignment.CenterVertically,
    ) {
        OutlinedButton(
            onClick = { store.reading.searchMore() },
            enabled = !loading,
            colors = ButtonDefaults.outlinedButtonColors(contentColor = colors.onSurface),
        ) {
            if (loading) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
            } else {
                Text(Strings.readSearchMoreAnilist(context))
            }
        }
        // "30 of 212 shown" once the count is exact, "30 shown" until then.
        Text(
            if (total != null) Strings.readSearchShown(context, shown, total.toInt()) else Strings.readSearchShownSome(context, shown),
            style = MaterialTheme.typography.bodySmall.tabular(),
            color = colors.onSurfaceVariant,
        )
    }
}

/**
 * The one polite announcement of a search with words, once every part has settled: "4 from
 * AniList, 2 from your sources, Northwind didn't answer." Turning Your sources only on says
 * AniList's results are hidden.
 */
@Composable
internal fun SearchSaid(search: ReadSearchState) {
    val context = LocalContext.current
    val sources = search.sources
    val said = when {
        sources == null || !sources.done -> ""
        else -> buildList {
            if (search.query.sourcesOnly) add(Strings.readSearchAnilistHidden(context))
            else if (sources.anilist == ReadAnilistState.FAILED) add(Strings.readSearchAnilistFailed(context))
            else add(Strings.readSearchSaidAnilist(context, search.tiles.size))
            val hits = sources.groups.sumOf { it.tiles.size }
            if (hits > 0) add(Strings.readSearchSaidSources(context, hits))
            sources.groups.filter { it.state is ReadSourceState.Failed || it.state is ReadSourceState.TimedOut }
                .forEach { add(Strings.readSearchSourceFailed(context, it.sourceName)) }
        }.joinToString(", ") // i18n-skip
    }
    // Outside the grid, so it is there to be heard however long the results are.
    Box(
        Modifier.size(1.dp).clearAndSetSemantics {
            if (said.isNotEmpty()) {
                contentDescription = said
                liveRegion = LiveRegionMode.Polite
            }
        },
    )
}

/**
 * A sideways row inside the padded grid reaching the screen's edges, so its covers scroll out
 * past the edge as Home's do, while its first cover lines up with the grid.
 */
internal fun Modifier.bleed(by: androidx.compose.ui.unit.Dp): Modifier = layout { measurable, constraints ->
    val extra = (by * 2).roundToPx()
    val width = constraints.maxWidth + extra
    val placeable = measurable.measure(constraints.copy(minWidth = width, maxWidth = width))
    layout(constraints.maxWidth, placeable.height) { placeable.place(-extra / 2, 0) }
}
