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
import app.kaeri.engine.model.ReadSourceJoin
import app.kaeri.engine.model.ReadSourceSearch
import app.kaeri.engine.model.ReadSourceState
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.Strings
import app.kaeri.store.ReadSearchState
import app.kaeri.store.ReadingModel
import app.kaeri.ui.AfterWait
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.wideWindow

/** AniList's part leaving when Your sources only turns on: 200 ms. */
private const val ANILIST_OUT = 200

/** Where a grid item of Search goes: AniList's covers fade out in 200 ms, the rest glide. */
@Composable
private fun LazyGridItemScope.searchMotion(anilist: Boolean = false): Modifier {
    val reduced = rememberSystemReducedMotion()
    return Modifier.animateItem(
        fadeInSpec = Motion.fadeIn(),
        placementSpec = if (reduced) null else Motion.standard(),
        fadeOutSpec = if (anilist) tween(ANILIST_OUT, easing = LinearOutSlowInEasing) else Motion.fadeOut(),
    )
}

/**
 * Search's results with words. [both] adds the count of matches in Shows; [demoOnly] the demo's
 * note. Browsing without words stays in [ReadSearchScreen].
 */
internal fun LazyGridScope.wordsResults(search: ReadSearchState, both: Boolean, demoOnly: Boolean) {
    val query = search.query
    val sources = search.sources
    val sourcesOnly = query.sourcesOnly
    val anilist = when {
        sourcesOnly -> ReadAnilistState.SKIPPED
        search.failure != null && search.tiles.isEmpty() -> ReadAnilistState.FAILED
        search.loading && search.tiles.isEmpty() -> ReadAnilistState.ASKING
        sources?.anilist == ReadAnilistState.FAILED -> ReadAnilistState.FAILED
        else -> ReadAnilistState.ANSWERED
    }
    val groups = sources?.groups.orEmpty()
    val shown = groups.filter { it.shows() }
    val nothing = groups.filter { it.state is ReadSourceState.Nothing }
    // Every part settled with nothing at all: one Nothing found, as before the sources.
    val nothingAnywhere = !search.loading && anilist != ReadAnilistState.ASKING && anilist != ReadAnilistState.FAILED &&
        search.tiles.isEmpty() && shown.isEmpty() && (sources == null || sources.done || groups.isEmpty())

    when {
        nothingAnywhere -> item(span = { GridItemSpan(maxLineSpan) }, key = "nothing", contentType = "quiet") {
            Box(searchMotion()) { NothingAnywhere(search, eitherLine = groups.isNotEmpty()) }
        }
        anilist == ReadAnilistState.FAILED -> item(span = { GridItemSpan(maxLineSpan) }, key = "anilist-failed", contentType = "quiet") {
            Box(searchMotion(anilist = true)) { AnilistFailed(search) }
        }
        anilist == ReadAnilistState.ASKING ->
            items(6, key = { "a-wait-$it" }, contentType = { "placeholder" }) {
                Box(searchMotion(anilist = true)) { AfterWait { CoverPlaceholderCell() } }
            }
        anilist == ReadAnilistState.ANSWERED && search.tiles.isEmpty() ->
            item(span = { GridItemSpan(maxLineSpan) }, key = "anilist-nothing", contentType = "quiet") {
                Box(searchMotion(anilist = true)) { AnilistNothing() }
            }
        anilist == ReadAnilistState.ANSWERED -> {
            val total = search.total
            if (total != null && !search.tiles.all { it.fromDemo }) {
                item(span = { GridItemSpan(maxLineSpan) }, key = "count", contentType = "line") {
                    val context = LocalContext.current
                    // "4 series from AniList", or with more to come, "30 of 212 shown".
                    val shownAll = !search.hasMore || search.tiles.size >= total.toInt()
                    Text(
                        if (shownAll) Strings.readSearchTotal(context, total.toInt()) else Strings.readSearchShown(context, search.tiles.size, total.toInt()),
                        style = MaterialTheme.typography.bodySmall.tabular(),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = searchMotion(anilist = true),
                    )
                }
            }
            val first = search.tiles.take(search.firstPage.coerceAtLeast(1))
            items(first, key = { "a-${it.work}" }, contentType = { "cover" }) { tile ->
                Box(searchMotion(anilist = true)) { ResultCell(tile, sources?.joins) }
            }
        }
        else -> {}
    }

    if (shown.isNotEmpty()) {
        item(span = { GridItemSpan(maxLineSpan) }, key = "from-sources", contentType = "heading") {
            val context = LocalContext.current
            Text(
                Strings.readSearchFromSources(context),
                style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                modifier = searchMotion().padding(top = if (sourcesOnly) 0.dp else 16.dp).semantics { heading() },
            )
        }
        for (group in shown) {
            item(span = { GridItemSpan(maxLineSpan) }, key = "src-${group.sourceId}", contentType = "source") {
                Box(searchMotion()) { SourceGroupRow(group) }
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
                Box(searchMotion(anilist = true)) { MoreFromAnilist(search.loading) }
            }
        } else if (later.isNotEmpty()) {
            items(later, key = { "a-${it.work}" }, contentType = { "cover" }) { tile ->
                Box(searchMotion(anilist = true)) { ResultCell(tile, sources?.joins) }
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
    item(span = { GridItemSpan(maxLineSpan) }, key = "said", contentType = "said") { SearchSaid(search) }
}

/** Whether a source's group takes a row: asking, failed, or with hits that joined nothing. */
private fun ReadSourceGroup.shows(): Boolean = when (state) {
    is ReadSourceState.Nothing -> false
    is ReadSourceState.Hits -> tiles.isNotEmpty()
    else -> true
}

/**
 * A source's kind in words beside its name: Server, Folder or Plugin. The engine names the kind
 * on the group (coming with the contract's next commit); until then the name stands alone.
 */
@Suppress("UNUSED_PARAMETER")
@Composable
internal fun sourceKind(group: ReadSourceGroup): String? = null

/** An AniList result in the grid, with "In Home library" under its caption when a source has it. */
@Composable
private fun ResultCell(tile: WorkTile, joins: List<ReadSourceJoin>?) {
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
            lines = if (wideWindow()) 1 else 2,
            reserveLine = true,
            onLongClick = { menu = true },
            extra = extra,
        )
        ReadTileMenu(tile, menu, current = null) { menu = false }
    }
}

/** A grid cell's placeholder: the cover's plate and its caption's two bars. */
@Composable
private fun CoverPlaceholderCell() {
    app.kaeri.ui.PosterPlaceholder()
}

/** One source's row: its name and kind, then its covers, a failure with Try again, or placeholders. */
@Composable
private fun SourceGroupRow(group: ReadSourceGroup) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val kind = sourceKind(group)
    val asking = group.state is ReadSourceState.Asking
    val reduced = rememberSystemReducedMotion()
    val label = if (asking) Strings.readSearchAsking(context, group.sourceName) else listOfNotNull(group.sourceName, kind).joinToString(", ") // i18n-skip
    Column(Modifier.padding(top = 12.dp)) {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 40.dp)
                .semantics(mergeDescendants = true) {
                    heading()
                    contentDescription = label
                },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(group.sourceName, style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.SemiBold), maxLines = 1, modifier = Modifier.weight(1f, fill = false))
            if (kind != null) Text(kind, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1)
            if (asking) CircularProgressIndicator(Modifier.size(14.dp), strokeWidth = 2.dp, color = colors.onSurfaceVariant)
        }
        // The slot keeps its place while asking; a failure closes it to its height on the spring.
        LandingSlot(
            landed = !asking,
            modifier = Modifier.bleed(16.dp).then(if (reduced) Modifier else Modifier.animateContentSize(Motion.standard())),
            placeholders = { CoverStripPlaceholder(count = if (wideWindow()) 6 else 4) },
        ) {
            when (val state = group.state) {
                is ReadSourceState.Failed -> SourceFailed(group, failureWords(context, state.failure), Modifier.padding(horizontal = 16.dp))
                is ReadSourceState.TimedOut -> SourceFailed(group, Strings.readSearchSourceTimeout(context, state.seconds.toInt()), Modifier.padding(horizontal = 16.dp))
                else -> TileStrip(group.tiles, "read-src-${group.sourceId}", rowKey = group.sourceId, planning = false)
            }
        }
    }
}

/** A source that didn't answer: the glyph, its name, the reason and Try again for it alone. */
@Composable
private fun SourceFailed(group: ReadSourceGroup, reason: String, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    FailureNotice(
        title = Strings.readSearchSourceFailed(context, group.sourceName),
        line = reason,
        action = Strings.actionTryAgain(context),
        onAction = { store.reading.retrySource(group.sourceId) },
        modifier = modifier,
    )
}

/** The vermilion glyph, a sentence and its reason, and a button under them. */
@Composable
internal fun FailureNotice(title: String, line: String?, action: String, onAction: () -> Unit, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    Row(modifier.padding(vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Icon(Icons.Problem, contentDescription = null, tint = colors.error, modifier = Modifier.padding(top = 2.dp).size(20.dp))
        Column(Modifier.weight(1f)) {
            Column(Modifier.semantics(mergeDescendants = true) {}) {
                Text(title, style = MaterialTheme.typography.bodyLarge, color = colors.onSurface)
                if (line != null) Text(line, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
            }
            OutlinedButton(
                onClick = onAction,
                modifier = Modifier.padding(top = 12.dp),
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
    FailureNotice(
        title = Strings.readSearchAnilistFailed(context),
        line = Strings.readSearchAnilistFailedLine(context),
        action = Strings.actionTryAgain(context),
        onAction = { store.reading.runSearch(search.query) },
    )
}

/** AniList found nothing, while the sources are still asked or found something. */
@Composable
private fun AnilistNothing() {
    val context = LocalContext.current
    Column(Modifier.padding(vertical = 8.dp).semantics(mergeDescendants = true) {}) {
        Text(Strings.readSearchAnilistNothing(context), style = MaterialTheme.typography.titleMedium)
        Text(
            Strings.readSearchNothingLine(context),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}

/** Nothing anywhere: Nothing found for the words, and the sources' line when there are any. */
@Composable
private fun NothingAnywhere(search: ReadSearchState, eitherLine: Boolean) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val query = search.query
    val filtered = query.formats.isNotEmpty() || query.origins.isNotEmpty() || query.status != null ||
        query.genres.isNotEmpty() || query.excludeGenres.isNotEmpty() || (query.started != null && query.started != "any") // i18n-skip
    Column {
        PlainEmpty(
            Strings.readSearchNothing(context, query.text),
            if (filtered) Strings.readSearchNothingFiltersLine(context) else Strings.readSearchNothingLine(context),
            inset = 0.dp,
            action = if (filtered) Strings.readSearchClearFilters(context) else null,
            onAction = { store.reading.runSearch(ReadingModel.EMPTY_QUERY.copy(text = query.text, sort = query.sort, sourcesOnly = query.sourcesOnly)) },
        )
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
@Composable
private fun MoreFromAnilist(loading: Boolean) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    Box(Modifier.fillMaxWidth().padding(top = 16.dp), contentAlignment = Alignment.CenterStart) {
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
    }
}

/**
 * The one polite announcement of a search with words, once every part has settled: "4 from
 * AniList, 2 from your sources, Northwind didn't answer." Turning Your sources only on says
 * AniList's results are hidden.
 */
@Composable
private fun SearchSaid(search: ReadSearchState) {
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
