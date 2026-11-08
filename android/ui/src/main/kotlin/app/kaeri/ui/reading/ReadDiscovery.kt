// Read home's discovery rows, as the discovery board draws them on Android: per format, in the
// order the engine picks from the reading mix. Each row has its heading (16 medium), the side text
// in haze beside it and View all at the end; a row on its way holds placeholders at final size
// from 600 ms, at least 0.3 s, and lands in one crossfade over them. A row that failed leaves with
// its heading and the gap closes on the standard spring. View all pushes the row's search onto
// Home with its filters as chips.

package app.kaeri.ui.reading

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.kaeri.design.Motion
import app.kaeri.design.rememberSystemReducedMotion
import app.kaeri.engine.model.ReadHomeRow
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.Strings
import app.kaeri.store.text
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.PLACEHOLDERS_AFTER
import app.kaeri.ui.PosterPlaceholder
import app.kaeri.ui.cardGap
import app.kaeri.ui.posterWidth
import app.kaeri.ui.shelfTop
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.TopAppBar
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.i18n.R
import app.kaeri.ui.AfterWait
import app.kaeri.ui.PosterGrid
import app.kaeri.ui.itemMotion
import app.kaeri.ui.wideWindow
import kotlinx.coroutines.delay

/** How long placeholders stand once shown, so they never flicker away. */
internal const val PLACEHOLDERS_HELD = 300L

/** The row fading in over its placeholders: 200 ms, ease out. */
private const val LAND_IN = 200

/** The placeholders fading out once the row is opaque: 300 ms. */
private const val PLACEHOLDERS_OUT = 300

/**
 * A lazy list item's motion for discovery rows: in and out on standard, and the gap a row leaves
 * closing on the standard spring, or at once with Remove animations on.
 */
@Composable
internal fun LazyItemScope.rowMotion(): Modifier {
    val reduced = rememberSystemReducedMotion()
    return Modifier.animateItem(
        fadeInSpec = Motion.standard(),
        placementSpec = if (reduced) null else Motion.standard(),
        fadeOutSpec = Motion.standard(),
    )
}

private enum class Landing { Waiting, Placeholders, Crossfading, Landed }

/**
 * A slot that holds [placeholders] until its content lands, as the board's motion table has it:
 * nothing for 600 ms (the slot keeps its height, so nothing under it moves), then the
 * placeholders, which stay at least 0.3 s; then [content] fades in over them in 200 ms and they
 * fade out over 300 ms once it is opaque. Content already here, or here within 600 ms, fades in
 * alone. Remove animations keeps the same crossfade. [reserve] keeps the slot's height while
 * waiting; without it the slot is empty until the placeholders show.
 */
@Composable
internal fun LandingSlot(
    landed: Boolean,
    modifier: Modifier = Modifier,
    reserve: Boolean = true,
    placeholders: @Composable () -> Unit,
    content: @Composable BoxScope.() -> Unit,
) {
    val here = remember { landed }
    var phase by remember { mutableStateOf(if (here) Landing.Landed else Landing.Waiting) }
    var shownAt by remember { mutableLongStateOf(0L) }
    val placeholderAlpha = remember { Animatable(if (here) 0f else 0f) }
    val contentAlpha = remember { Animatable(if (here) 1f else 0f) }
    LaunchedEffect(Unit) {
        if (phase != Landing.Waiting) return@LaunchedEffect
        delay(PLACEHOLDERS_AFTER)
        if (phase == Landing.Waiting) {
            phase = Landing.Placeholders
            shownAt = withFrameMillis { it }
            placeholderAlpha.animateTo(1f, Motion.fadeIn())
        }
    }
    LaunchedEffect(landed) {
        if (!landed) {
            // Asked again (a source's Try again): the placeholders come back in one crossfade.
            if (phase == Landing.Landed || phase == Landing.Crossfading) {
                phase = Landing.Placeholders
                shownAt = withFrameMillis { it }
                placeholderAlpha.snapTo(1f)
                contentAlpha.animateTo(0f, tween(LAND_IN, easing = LinearOutSlowInEasing))
            }
            return@LaunchedEffect
        }
        when (phase) {
            Landing.Landed -> return@LaunchedEffect
            Landing.Waiting -> {
                // Within 600 ms: the content fades in on its own, with no placeholders.
                phase = Landing.Crossfading
                contentAlpha.animateTo(1f, tween(LAND_IN, easing = LinearOutSlowInEasing))
                phase = Landing.Landed
            }
            Landing.Placeholders, Landing.Crossfading -> {
                val held = withFrameMillis { it } - shownAt
                if (held < PLACEHOLDERS_HELD) delay(PLACEHOLDERS_HELD - held)
                phase = Landing.Crossfading
                contentAlpha.animateTo(1f, tween(LAND_IN, easing = LinearOutSlowInEasing))
                placeholderAlpha.animateTo(0f, tween(PLACEHOLDERS_OUT, easing = LinearOutSlowInEasing))
                phase = Landing.Landed
            }
        }
    }
    Box(modifier) {
        val showPlaceholders = phase == Landing.Placeholders || phase == Landing.Crossfading || (reserve && phase == Landing.Waiting)
        if (showPlaceholders) {
            // Hidden from screen readers: the heading carries the state.
            Box(Modifier.clearAndSetSemantics {}.graphicsLayer { alpha = placeholderAlpha.value }) { placeholders() }
        }
        if (landed || phase == Landing.Crossfading) {
            Box(Modifier.graphicsLayer { alpha = contentAlpha.value }, content = content)
        }
    }
}

/** A row's heading: the title, its side text in haze, and View all at the end in silk. */
@Composable
internal fun DiscoveryHeading(
    title: String,
    side: String?,
    loading: Boolean,
    link: String? = null,
    onLink: () -> Unit = {},
) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val loadingWord = Strings.stateLoading(context)
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            Modifier.weight(1f).semantics(mergeDescendants = true) {
                heading()
                // While the row is on its way the heading says so; the placeholders are hidden.
                if (loading) contentDescription = "$title, $loadingWord" // i18n-skip
            },
            verticalAlignment = Alignment.Bottom,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                title,
                style = MaterialTheme.typography.titleMedium,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
            if (side != null) {
                Text(side, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1)
            }
        }
        if (link != null) {
            TextButton(
                onClick = onLink,
                // Named with its row, so a list of links makes sense.
                modifier = Modifier.semantics { contentDescription = "$link, $title" }, // i18n-skip
                colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
            ) {
                Text(link)
                Spacer(Modifier.width(2.dp))
                Icon(Icons.Chevron, contentDescription = null, modifier = Modifier.size(18.dp))
            }
        }
    }
}

/**
 * One discovery row: its heading, then its covers in a sideways row, or the placeholders while it
 * is on its way. A row with nothing and nothing coming draws nothing; the engine leaves it out.
 */
@Composable
internal fun DiscoveryRow(row: ReadHomeRow, onViewAll: (ReadHomeRow) -> Unit) {
    val context = LocalContext.current
    val landed = row.tiles.isNotEmpty()
    if (!landed && !row.pending) return
    val title = row.title.text(context)
    Column(Modifier.padding(top = shelfTop())) {
        DiscoveryHeading(
            title = title,
            side = row.side?.text(context),
            loading = !landed,
            link = if (landed && row.viewAll != null) Strings.homeViewAll(context) else null,
            onLink = { onViewAll(row) },
        )
        LandingSlot(landed, placeholders = { CoverStripPlaceholder() }) {
            TileStrip(row.tiles, "read-row-${row.id}", rowKey = row.id)
        }
    }
}

/** Covers in a sideways row, 112 dp on a phone and 128 on a tablet, each with its two lines. */
@Composable
internal fun TileStrip(tiles: List<WorkTile>, from: String, rowKey: String, planning: Boolean = true) {
    val state = rememberLazyListState()
    LazyRow(
        state = state,
        contentPadding = PaddingValues(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(cardGap()),
    ) {
        items(tiles, key = { "$rowKey-${it.work}" }, contentType = { "cover" }) { tile ->
            ReadTile(tile, posterWidth(), from = from, lines = 2, reserveLine = true, planning = planning)
        }
    }
}

/** A row's placeholders at final size: four covers' plates with their two lines as bars. */
@Composable
internal fun CoverStripPlaceholder(count: Int = 4) {
    Row(
        Modifier.padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(cardGap()),
    ) {
        repeat(count) { Box(Modifier.width(posterWidth())) { PosterPlaceholder() } }
    }
}

/** First run offline, nothing kept: one line in place of the rows, under a rule. */
@Composable
internal fun OfflineRowsLine() {
    val context = LocalContext.current
    Column(Modifier.padding(start = 16.dp, end = 16.dp, top = 24.dp)) {
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
        Text(
            Strings.readHomeOfflineRows(context),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 16.dp),
        )
    }
}

/** View all: the row's exact search pushed onto Home, titled with the row. */
@Composable
internal fun rememberViewAll(): (ReadHomeRow) -> Unit {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    return remember(store, navigator) {
        { row ->
            row.viewAll?.let { query ->
                ReadPlaces.viewAllTitle = row.title.text(context)
                store.reading.runViewAll(query)
                navigator.openReadPage(READ_PAGE_VIEW_ALL)
            }
        }
    }
}

// Pages pushed from Read's Home

/** View all's page: a row's search with its filters as chips. */
internal const val READ_PAGE_VIEW_ALL = "view-all" // i18n-skip

/** E-ink's Browse page: the list of discovery rows. */
internal const val READ_PAGE_BROWSE = "browse" // i18n-skip

/** E-ink's grid of one row: `row:<id>`. */
internal const val READ_PAGE_ROW = "row:" // i18n-skip

/** E-ink's paged Search. */
internal const val READ_PAGE_SEARCH = "search" // i18n-skip

/** A reading page pushed onto the current place's back stack, by its name. */
@Composable
internal fun ReadPage(name: String) {
    when {
        name == READ_PAGE_VIEW_ALL -> ViewAllPage()
        name == READ_PAGE_BROWSE -> EinkBrowsePage()
        name.startsWith(READ_PAGE_ROW) -> EinkRowPage(name.removePrefix(READ_PAGE_ROW))
        name == READ_PAGE_SEARCH -> EinkSearchPage()
    }
}

/**
 * View all: the row's exact search pushed over Home, titled with the row, its filters as chips
 * that open their menus, and the results three across; the first are the row's own titles.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ViewAllPage() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val search by store.reading.viewAll.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    Scaffold(
        containerColor = colors.surface,
        topBar = {
            TopAppBar(
                title = { Text(ReadPlaces.viewAllTitle, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onClick = navigator::back) { Icon(Icons.Back, contentDescription = stringResource(R.string.action_back)) } },
            )
        },
    ) { padding ->
        Column(Modifier.padding(top = padding.calculateTopPadding())) {
            ReadFilterRow(search.query, store.reading.filters, browsing = false, onlySet = true, run = store.reading::runViewAll)
            PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp + padding.calculateBottomPadding())) {
                val failure = search.failure
                when {
                    failure != null && search.tiles.isEmpty() -> item(span = { GridItemSpan(maxLineSpan) }, key = "failed", contentType = "quiet") {
                        PlainEmpty(
                            Strings.readSearchFailed(context),
                            failureWords(context, failure),
                            inset = 0.dp,
                            action = Strings.actionTryAgain(context),
                            onAction = { store.reading.runViewAll(search.query) },
                        )
                    }
                    search.tiles.isEmpty() && search.loading ->
                        items(9, key = { "cell-$it" }, contentType = { "placeholder" }) { Box(itemMotion()) { AfterWait { PosterPlaceholder() } } }
                    search.tiles.isEmpty() -> item(span = { GridItemSpan(maxLineSpan) }, key = "nothing", contentType = "quiet") {
                        PlainEmpty(Strings.readSearchNothingBrowse(context), Strings.readSearchNothingFiltersLine(context), inset = 0.dp)
                    }
                    else -> {
                        items(search.tiles, key = { "v-${it.work}" }, contentType = { "cover" }) { tile ->
                            BoxWithConstraints(itemMotion()) {
                                ReadTile(tile, maxWidth, from = "read-view-all", lines = if (wideWindow()) 1 else 2, reserveLine = true, planning = true)
                            }
                        }
                        if (search.hasMore) {
                            item(span = { GridItemSpan(maxLineSpan) }, key = "more-${search.page}", contentType = "more") {
                                LaunchedEffect(search.page) { store.reading.viewAllMore() }
                                Box(Modifier.fillMaxWidth().heightIn(min = 48.dp), contentAlignment = Alignment.Center) {
                                    if (search.loading) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
