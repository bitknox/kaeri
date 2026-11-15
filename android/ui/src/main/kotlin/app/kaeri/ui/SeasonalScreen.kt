// Seasonal: this season's titles from AniList's public data, as a poster grid.

package app.kaeri.ui

import app.kaeri.i18n.R
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.store.ListState
import app.kaeri.store.seasonQuery

@Composable
fun SeasonalScreen() {
    val store = LocalAppStore.current
    val state by store.seasonal.state.collectAsStateWithLifecycle()
    val caption by store.seasonal.caption.collectAsStateWithLifecycle()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val navigator = LocalNavigator.current
    LaunchedEffect(Unit) { store.loadSeasonal() }
    ScreenFrame(
        title = stringResource(R.string.nav_seasonal),
        subtitle = caption?.let(::seasonName),
        actions = {
            // The season as a search, to narrow by genre, format or anything else.
            QuietButton(stringResource(R.string.seasonal_filter_in_search)) {
                store.search(seasonQuery())
                navigator.go(Destination.Search)
            }
        },
    ) { padding ->
        val grid = PaddingValues(start = 16.dp, end = 16.dp, top = padding.calculateTopPadding(), bottom = 24.dp)
        val answered = state.takeIf { it is ListState.Loaded || it is ListState.Empty || it is ListState.Failed }
        val offline by store.offline.collectAsStateWithLifecycle()
        Loading(
            answered,
            placeholders = {
                if (offline) {
                    Column(Modifier.padding(top = padding.calculateTopPadding())) { OfflineState() }
                } else {
                    PosterGrid(grid) { items(9) { PosterPlaceholder() } }
                }
            },
        ) { list ->
            Refreshable({ store.loadSeasonal(force = true) }, top = padding.calculateTopPadding()) {
                when (list) {
                    is ListState.Loaded -> PosterGrid(grid) {
                        items(list.ids.mapNotNull { titles[it] }, key = { it.id.toLong() }) { PosterCell(it, itemMotion(), from = "seasonal") }
                    }
                    is ListState.Failed -> Column(Modifier.padding(top = padding.calculateTopPadding()).verticalScroll(rememberScrollState())) {
                        FailedState(stringResource(R.string.seasonal_load_failed), list.reason, retry = { store.loadSeasonal(force = true) })
                    }
                    else -> Column(Modifier.padding(top = padding.calculateTopPadding()).verticalScroll(rememberScrollState())) { Quiet(caption?.let { stringResource(R.string.seasonal_empty_title, seasonName(it)) } ?: stringResource(R.string.seasonal_empty_detail)) }
                }
            }
        }
    }
}

/**
 * Posters three to a row on a phone, 13 apart across and 16 down; as many as fit at 140 dp or
 * more on a tablet.
 */
@Composable
internal fun PosterGrid(
    padding: PaddingValues,
    state: androidx.compose.foundation.lazy.grid.LazyGridState = androidx.compose.foundation.lazy.grid.rememberLazyGridState(),
    content: androidx.compose.foundation.lazy.grid.LazyGridScope.() -> Unit,
) {
    LazyVerticalGrid(
        state = state,
        columns = if (wideWindow()) GridCells.Adaptive(POSTER_CELL_MIN) else GridCells.Fixed(POSTER_COLUMNS),
        contentPadding = padding,
        horizontalArrangement = Arrangement.spacedBy(POSTER_GAP),
        verticalArrangement = Arrangement.spacedBy(16.dp),
        content = content,
    )
}

/** A phone's [PosterGrid]: three across. */
private const val POSTER_COLUMNS = 3

/** A wide window's [PosterGrid] cells: at least this wide, as many as fit. */
private val POSTER_CELL_MIN = 140.dp

/** The gap between a [PosterGrid]'s columns. */
private val POSTER_GAP = 13.dp

/** The widths of a [PosterGrid]'s columns, from the first. */
@Immutable
internal class PosterCells(private val widths: List<Dp>) {
    /** The width of the [index]th of a run of cells that starts a row. */
    fun width(index: Int): Dp = widths[index % widths.size]
}

/**
 * A [PosterGrid] that tells its content each column's width, measured once for the whole grid,
 * where a cell measuring its own would be a second composition in each.
 */
@Composable
internal fun SizedPosterGrid(
    padding: PaddingValues,
    state: androidx.compose.foundation.lazy.grid.LazyGridState = androidx.compose.foundation.lazy.grid.rememberLazyGridState(),
    content: androidx.compose.foundation.lazy.grid.LazyGridScope.(PosterCells) -> Unit,
) {
    BoxWithConstraints {
        val cells = posterCells(constraints.maxWidth, padding)
        PosterGrid(padding, state) { content(cells) }
    }
}

/**
 * The columns of a [PosterGrid] [width] pixels wide with [padding] about it, worked out as the
 * grid works them out: its columns in whole pixels, the first ones a pixel wider where the width
 * doesn't divide.
 */
@Composable
private fun posterCells(width: Int, padding: PaddingValues): PosterCells {
    val density = LocalDensity.current
    val direction = LocalLayoutDirection.current
    val wide = wideWindow()
    return remember(width, padding, density, direction, wide) {
        with(density) {
            val gap = POSTER_GAP.roundToPx()
            val inside = width - padding.calculateStartPadding(direction).roundToPx() - padding.calculateEndPadding(direction).roundToPx()
            val columns = if (wide) maxOf((inside + gap) / (POSTER_CELL_MIN.roundToPx() + gap), 1) else POSTER_COLUMNS
            val cells = inside - gap * (columns - 1)
            PosterCells(List(columns) { (cells / columns + if (it < cells % columns) 1 else 0).toDp() })
        }
    }
}

/**
 * The season, from the caption the engine says above Seasonal: "Fall 2026" out of
 * "fall 2026 · the 40 most popular".
 */
internal fun seasonName(caption: String): String =
    caption.substringBefore('·').trim().split(' ').joinToString(" ") { word ->
        word.replaceFirstChar { it.titlecase() }
    }
