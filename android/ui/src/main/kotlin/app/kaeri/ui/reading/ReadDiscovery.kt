// Read home's discovery rows, as the discovery board draws them on Android: per format, in the
// order the engine picks from the reading mix. Each row has its heading (16 medium), the side text
// in haze beside it and View all at the end; a row on its way holds placeholders at final size
// from 600 ms, at least 0.3 s, and lands in one crossfade over them, View all with it. A row that
// failed leaves with its heading and the gap closes on the standard spring. View all pushes the
// row's search onto Home on the shared axis, with its filters as chips.

package app.kaeri.ui.reading

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.lazy.LazyItemScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridItemScope
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.SwapFade
import app.kaeri.design.atFullSpeed
import app.kaeri.design.rememberStillFade
import app.kaeri.design.rememberSystemReducedMotion
import app.kaeri.design.stillFade
import app.kaeri.design.tabular
import app.kaeri.engine.model.ReadHomeRow
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.R
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
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

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
 * alone. Asked again after it landed, what it showed stays until the new answer comes, or for
 * 600 ms, then crossfades to the placeholders; it never fades through empty. Remove animations
 * keeps the same crossfades. [reserve] keeps the slot's height while waiting; without it the slot
 * is empty until the placeholders show. [waitKey] keeps the wait's start with the row, so a row
 * scrolled away and back does not start its 600 ms over. [onLanding] is told as the content
 * starts to fade in, for what fades in with it.
 */
@Composable
internal fun LandingSlot(
    landed: Boolean,
    modifier: Modifier = Modifier,
    reserve: Boolean = true,
    waitKey: String? = null,
    onLanding: () -> Unit = {},
    placeholders: @Composable () -> Unit,
    content: @Composable BoxScope.() -> Unit,
) {
    val here = remember { landed }
    var phase by remember { mutableStateOf(if (here) Landing.Landed else Landing.Waiting) }
    var shownAt by remember { mutableLongStateOf(0L) }
    val placeholderAlpha = remember { Animatable(0f) }
    val contentAlpha = remember { Animatable(if (here) 1f else 0f) }
    val landing by rememberUpdatedState(onLanding)
    LaunchedEffect(landed) {
        if (!landed) {
            if (phase == Landing.Landed || phase == Landing.Crossfading) phase = Landing.Waiting
            val now = withFrameMillis { it }
            val start = waitKey?.let { RowWaits.since(it, now) } ?: now
            val left = PLACEHOLDERS_AFTER - (now - start)
            if (left > 0) delay(left)
            if (phase != Landing.Waiting) return@LaunchedEffect
            phase = Landing.Placeholders
            shownAt = start + PLACEHOLDERS_AFTER
            if (left <= 0 && contentAlpha.value == 0f) {
                // Scrolled back to a row still on its way: its placeholders are there already.
                placeholderAlpha.snapTo(1f)
            } else {
                // Nothing new in 600 ms: the placeholders crossfade in over what was there.
                atFullSpeed {
                    coroutineScope {
                        launch { placeholderAlpha.animateTo(1f, Motion.fadeIn()) }
                        launch { contentAlpha.animateTo(0f, Motion.fadeOut()) }
                    }
                }
            }
            return@LaunchedEffect
        }
        waitKey?.let(RowWaits::done)
        when (phase) {
            Landing.Landed -> return@LaunchedEffect
            Landing.Waiting -> {
                // Within 600 ms: the content fades in on its own, with no placeholders; an answer
                // over what was kept changes in place.
                phase = Landing.Crossfading
                landing()
                atFullSpeed { contentAlpha.animateTo(1f, tween(LAND_IN, easing = LinearOutSlowInEasing)) }
                phase = Landing.Landed
            }
            Landing.Placeholders, Landing.Crossfading -> {
                val held = withFrameMillis { it } - shownAt
                if (held < PLACEHOLDERS_HELD) delay(PLACEHOLDERS_HELD - held)
                phase = Landing.Crossfading
                landing()
                atFullSpeed {
                    contentAlpha.animateTo(1f, tween(LAND_IN, easing = LinearOutSlowInEasing))
                    placeholderAlpha.animateTo(0f, tween(PLACEHOLDERS_OUT, easing = LinearOutSlowInEasing))
                }
                phase = Landing.Landed
            }
        }
    }
    Box(modifier) {
        val kept = contentAlpha.value > 0f
        val showPlaceholders = phase == Landing.Placeholders || phase == Landing.Crossfading || (reserve && phase == Landing.Waiting && !kept)
        if (showPlaceholders) {
            // Hidden from screen readers: the heading carries the state.
            Box(Modifier.clearAndSetSemantics {}.graphicsLayer { alpha = placeholderAlpha.value }) { placeholders() }
        }
        if (landed || kept || phase == Landing.Crossfading) {
            Box(Modifier.graphicsLayer { alpha = contentAlpha.value }, content = content)
        }
    }
}

/**
 * A row's heading: the title, its side text in haze beside it (under it when the two do not fit,
 * so large text keeps the title whole), and View all at the end in silk. [linkShown] fades the
 * link in with the row's covers; its place is kept meanwhile, so the title does not move.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun DiscoveryHeading(
    title: String,
    side: String?,
    loading: Boolean,
    link: String? = null,
    linkShown: Boolean = true,
    onLink: () -> Unit = {},
) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val loadingWord = Strings.stateLoading(context)
    val linkAlpha = remember { Animatable(if (linkShown) 1f else 0f) }
    LaunchedEffect(linkShown) {
        atFullSpeed { linkAlpha.animateTo(if (linkShown) 1f else 0f, tween(LAND_IN, easing = LinearOutSlowInEasing)) }
    }
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        FlowRow(
            Modifier.weight(1f).padding(vertical = 4.dp).semantics(mergeDescendants = true) {
                heading()
                // While the row is on its way the heading says so; the placeholders are hidden.
                if (loading) contentDescription = "$title, $loadingWord" // i18n-skip
            },
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            itemVerticalAlignment = Alignment.Bottom,
        ) {
            Text(title, style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
            if (side != null) {
                Text(side, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
        if (link != null) {
            TextButton(
                onClick = onLink,
                enabled = linkShown,
                // Named with its row, so a list of links makes sense; not there until the row is.
                modifier = Modifier
                    .graphicsLayer { alpha = linkAlpha.value }
                    .then(if (linkShown) Modifier.semantics { contentDescription = "$link, $title" } else Modifier.clearAndSetSemantics {}), // i18n-skip
                colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface, disabledContentColor = colors.onSurface),
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
 * Each caption is two facts the row picks; a long press opens the cover's menu.
 */
@Composable
internal fun DiscoveryRow(row: ReadHomeRow, onViewAll: (ReadHomeRow) -> Unit) {
    val context = LocalContext.current
    val landed = row.tiles.isNotEmpty()
    if (!landed && !row.pending) return
    val title = row.title.text(context)
    var linkShown by remember { mutableStateOf(landed) }
    Column(Modifier.padding(top = shelfTop())) {
        DiscoveryHeading(
            title = title,
            side = row.side?.text(context),
            loading = !landed,
            link = if (row.viewAll != null) Strings.homeViewAll(context) else null,
            linkShown = linkShown && landed,
            onLink = { onViewAll(row) },
        )
        LandingSlot(landed, waitKey = "row-${row.id}", onLanding = { linkShown = true }, placeholders = { CoverStripPlaceholder() }) {
            val oneFormat = row.format != null
            TileStrip(row.tiles, "read-row-${row.id}", rowKey = row.id) { it.rowCaption(context, row.kind, oneFormat) }
        }
    }
}

/**
 * Covers in a sideways row, 112 dp on a phone and 128 on a tablet, each with its two lines. With
 * [planning] a long press opens the cover's menu (Details and the lists) and screen readers get
 * Add to Planning, as discovery's covers have them; a source's hits have neither.
 */
@Composable
internal fun TileStrip(
    tiles: List<WorkTile>,
    from: String,
    rowKey: String,
    planning: Boolean = true,
    /** The line under each title in place of the tile's own, shown and spoken. */
    caption: ((WorkTile) -> RowCaption)? = null,
) {
    val state = rememberLazyListState()
    LazyRow(
        state = state,
        contentPadding = PaddingValues(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(cardGap()),
    ) {
        items(tiles, key = { "$rowKey-${it.work}" }, contentType = { "cover" }) { tile ->
            val words = caption?.invoke(tile)
            if (planning) {
                var menu by remember { mutableStateOf(false) }
                Box {
                    ReadTile(
                        tile, posterWidth(), from = from, lines = 1, reserveLine = true, planning = true,
                        caption = words?.shown, spoken = words?.spoken, onLongClick = { menu = true },
                    )
                    ReadTileMenu(tile, menu, current = null) { menu = false }
                }
            } else {
                ReadTile(tile, posterWidth(), from = from, lines = 1, reserveLine = true, caption = words?.shown, spoken = words?.spoken)
            }
        }
    }
}

/**
 * A row's placeholders at final size: as many covers' plates as the width shows, with their
 * title and line as bars that grow with the text.
 */
@Composable
internal fun CoverStripPlaceholder() {
    val width = posterWidth()
    val gap = cardGap()
    val titleLines = titleLines()
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        // One more than fits, so the last reaches past the edge as the row's covers do.
        val count = (((maxWidth - 16.dp) + gap) / (width + gap)).toInt() + 1
        Row(
            Modifier.wrapContentWidth(Alignment.Start, unbounded = true).padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(gap),
        ) {
            repeat(count.coerceAtLeast(1)) { Box(Modifier.width(width)) { PosterPlaceholder(titleLines, 1) } }
        }
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

/**
 * View all: the row's exact search pushed onto Home, titled with the row. The page is named with
 * the row's id, so its title and search come back after the app was away. With Remove animations
 * on, the push swaps behind a still of the page, a crossfade rather than a cut.
 */
@Composable
internal fun rememberViewAll(): (ReadHomeRow) -> Unit {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val removed by rememberUpdatedState(app.kaeri.design.rememberAnimationsRemoved())
    val scope = rememberCoroutineScope()
    return remember(store, navigator) {
        { row ->
            row.viewAll?.let { query ->
                store.reading.runViewAll(query)
                val fade = navigator.fade
                if (removed && fade != null) {
                    scope.launch { fade.swap(fullSpeed = true) { navigator.openReadPage(READ_PAGE_VIEW_ALL + row.id) } }
                } else {
                    navigator.openReadPage(READ_PAGE_VIEW_ALL + row.id)
                }
            }
        }
    }
}

// Pages pushed from Read's Home

/** View all's page for a row: `view-all:<row id>`, a row's search with its filters as chips. */
internal const val READ_PAGE_VIEW_ALL = "view-all:" // i18n-skip

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
        name.startsWith(READ_PAGE_VIEW_ALL) -> ViewAllPage(name.removePrefix(READ_PAGE_VIEW_ALL))
        name == READ_PAGE_BROWSE -> EinkBrowsePage()
        name.startsWith(READ_PAGE_ROW) -> EinkRowPage(name.removePrefix(READ_PAGE_ROW))
        name == READ_PAGE_SEARCH -> EinkSearchPage()
    }
}

/**
 * View all: the row's exact search pushed over Home, titled with the row, its set filters as
 * checked chips first, then the order and the filters left to set. The results are three across
 * on a phone; the first are the row's own titles. New chips keep the grid until their answer
 * lands. Back with Remove animations on crossfades, as the push did.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ViewAllPage(rowId: String) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val search by store.reading.viewAll.collectAsStateWithLifecycle()
    val home by store.reading.home.collectAsStateWithLifecycle()
    val row = home?.rows?.firstOrNull { it.id == rowId }
    // Back after the app was away: Home's rows say what the row was, and its search runs again.
    LaunchedEffect(Unit) { if (store.reading.home.value == null) store.reading.loadHome() }
    LaunchedEffect(row?.viewAll) {
        val query = row?.viewAll ?: return@LaunchedEffect
        val now = store.reading.viewAll.value
        if (now.tiles.isEmpty() && !now.loading && now.failure == null) store.reading.runViewAll(query)
    }
    val colors = MaterialTheme.colorScheme
    val removed = app.kaeri.design.rememberAnimationsRemoved()
    val scope = rememberCoroutineScope()
    val back: () -> Unit = {
        val fade = navigator.fade
        if (removed && fade != null) scope.launch { fade.swap(fullSpeed = true) { navigator.back() } } else navigator.back()
    }
    // With Remove animations on there is no predictive back to seek, so back crossfades by itself.
    BackHandler(enabled = removed, onBack = back)
    val gridFade = rememberStillFade()
    val shown = rememberShownSearch(search, gridFade)
    Scaffold(
        containerColor = colors.surface,
        topBar = {
            TopAppBar(
                title = { Text(row?.title?.text(context).orEmpty(), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onClick = back) { Icon(Icons.Back, contentDescription = stringResource(R.string.action_back)) } },
            )
        },
    ) { padding ->
        Column(Modifier.padding(top = padding.calculateTopPadding())) {
            ReadFilterRow(search.query, store.reading.filters, browsing = false, viewAll = true) { store.reading.runViewAll(it, keep = true) }
            ReadGrid(
                PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 24.dp + padding.calculateBottomPadding()),
                Modifier.stillFade(gridFade),
            ) { grid -> browseCells(shown, grid, viewAll = true, lineFor = null) }
        }
    }
}

/**
 * A browsing grid's cells, Search's without words and View all's: [lineFor] the line over the
 * covers, or none. Failed with nothing kept says so with Try again; asked with nothing to show
 * holds placeholders from 600 ms; nothing found says [nothing].
 */
internal fun LazyGridScope.browseCells(
    shown: ShownSearch,
    grid: ReadGridShape,
    viewAll: Boolean,
    lineFor: ((Context) -> String)?,
    nothing: @Composable () -> Unit = { BrowseNothing() },
) {
    val search = shown.search
    val failure = shown.failureShown
    val from = if (viewAll) "read-view-all" else "read-search" // i18n-skip
    when {
        failure != null -> item(span = { GridItemSpan(maxLineSpan) }, key = "failed", contentType = "quiet") {
            val store = LocalAppStore.current
            val context = LocalContext.current
            Box(searchMotion()) {
                PlainEmpty(
                    Strings.readSearchFailed(context),
                    failureWords(context, failure),
                    inset = 0.dp,
                    small = true,
                    action = Strings.actionTryAgain(context),
                    onAction = { if (viewAll) store.reading.runViewAll(search.query, keep = true) else store.reading.runSearch(search.query) },
                )
            }
        }
        search.tiles.isEmpty() && search.loading -> {
            // The line's place is held, so the covers land where their placeholders were.
            if (lineFor != null) item(span = { GridItemSpan(maxLineSpan) }, key = "line", contentType = "line") { LineSpace() }
            if (shown.waited) placeholderCells(grid.columns * 3, grid.list, grid.lines, "b-wait")
        }
        search.tiles.isEmpty() -> item(span = { GridItemSpan(maxLineSpan) }, key = "nothing", contentType = "quiet") {
            Box(searchMotion()) { nothing() }
        }
        else -> {
            if (lineFor != null) {
                item(span = { GridItemSpan(maxLineSpan) }, key = "line", contentType = "line") {
                    val context = LocalContext.current
                    SwapFade(lineFor(context), searchMotion(), animationSpec = tween(LAND_IN, easing = LinearOutSlowInEasing), label = "browse line") { said -> // i18n-skip
                        Text(said, style = MaterialTheme.typography.bodySmall.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            items(search.tiles, key = { "b-${it.work}" }, contentType = { "cover" }) { tile ->
                Box(searchMotion(anilist = true)) { BrowseCell(tile, grid, from) }
            }
            if (search.hasMore) {
                item(span = { GridItemSpan(maxLineSpan) }, key = "more-${search.page}", contentType = "more") {
                    val store = LocalAppStore.current
                    LaunchedEffect(search.page) { if (viewAll) store.reading.viewAllMore() else store.reading.searchMore() }
                    Box(Modifier.fillMaxWidth().heightIn(min = 48.dp), contentAlignment = Alignment.Center) {
                        if (search.loading) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                    }
                }
            }
        }
    }
}

/** An empty line at the height of the browse line, held while the first covers come. */
@Composable
private fun LazyGridItemScope.LineSpace() {
    val height = with(LocalDensity.current) { MaterialTheme.typography.bodySmall.lineHeight.toDp() }
    Box(searchMotion().height(height))
}

/** Nothing found while browsing with filters set. */
@Composable
private fun BrowseNothing() {
    val context = LocalContext.current
    PlainEmpty(Strings.readSearchNothingBrowse(context), Strings.readSearchNothingFiltersLine(context), inset = 0.dp, small = true)
}

/** A cover while browsing, with its long-press menu: Details and the lists, Planning among them. */
@Composable
private fun BrowseCell(tile: WorkTile, grid: ReadGridShape, from: String) {
    var menu by remember { mutableStateOf(false) }
    BoxWithConstraints {
        ReadTile(
            tile, maxWidth, from = from, lines = grid.lines, reserveLine = true, planning = true,
            onLongClick = { menu = true }, list = grid.list,
        )
        ReadTileMenu(tile, menu, current = null) { menu = false }
    }
}
