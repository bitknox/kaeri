// What reading's grids share: one cover size per width with the rows (112 on a phone, three
// across; 128 on a tablet, as many as fit), a one-column list at the largest text, and what
// Search and View all draw while new results are asked: the last answer, kept until the new one
// lands, with placeholders only where nothing shows after 600 ms, held at least 0.3 s.

package app.kaeri.ui.reading

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyGridScope
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import app.kaeri.design.StillFade
import app.kaeri.design.rememberAnimationsRemoved
import app.kaeri.engine.model.ReadSourceState
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.store.ReadSearchState
import app.kaeri.ui.PLACEHOLDERS_AFTER
import app.kaeri.ui.cardGap
import app.kaeri.ui.posterWidth
import app.kaeri.ui.wideWindow
import kotlinx.coroutines.delay

/** How a reading grid lays out: its columns, whether it is a list, and the lines under a title. */
@Immutable
internal data class ReadGridShape(val columns: Int, val list: Boolean, val lines: Int)

/**
 * Reading's grid of covers. On a phone three across with 13 between; wider, the rows' 128 dp
 * covers with their 16 dp gaps, as many as fit, so Search's covers and a row's are one size. At
 * the largest text it is a one-column list.
 */
@Composable
internal fun ReadGrid(
    padding: PaddingValues,
    modifier: Modifier = Modifier,
    state: LazyGridState = rememberLazyGridState(),
    /** Search's grid: the shows' Search columns and gaps, cells at least 140 wide with 13 between. */
    search: Boolean = false,
    content: LazyGridScope.(ReadGridShape) -> Unit,
) {
    val list = listInsteadOfGrid()
    val wide = wideWindow()
    val cover = if (search) SEARCH_CELL else posterWidth()
    val gap = if (wide && !search) cardGap() else 13.dp
    BoxWithConstraints(modifier) {
        val inner = maxWidth - padding.calculateStartPadding(LayoutDirection.Ltr) - padding.calculateEndPadding(LayoutDirection.Ltr)
        val columns = when {
            list -> 1
            !wide -> 3
            else -> ((inner + gap) / (cover + gap)).toInt().coerceAtLeast(1)
        }
        val shape = ReadGridShape(columns, list, lines = if (wide || list) 1 else 2)
        LazyVerticalGrid(
            state = state,
            columns = when {
                list || !wide -> GridCells.Fixed(columns)
                search -> GridCells.Adaptive(cover)
                else -> GridCells.FixedSize(cover)
            },
            contentPadding = padding,
            horizontalArrangement = Arrangement.spacedBy(gap),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) { content(shape) }
    }
}

/** The narrowest cell of Search's grid on a wide window, as the shows' Search has it. */
private val SEARCH_CELL = 140.dp

/** What a search grid draws: [search] as held, and whether 600 ms have passed with it still busy. */
@Immutable
internal data class ShownSearch(val search: ReadSearchState, val waited: Boolean)

/** Something of the search is still on its way: AniList's page, or a source. */
private val ReadSearchState.busy: Boolean
    get() = loading || staleSources.isNotEmpty() || sources?.groups?.any { it.state is ReadSourceState.Asking } == true

/**
 * AniList's part is asked with no covers to show, so its place holds placeholders once 600 ms
 * pass. A failure kept from before (Try again) stays until then.
 */
internal val ReadSearchState.empty: Boolean
    get() = loading && tiles.isEmpty() && !query.sourcesOnly

/** A failure to draw: one kept while it is asked again gives way to placeholders at 600 ms. */
internal val ShownSearch.failureShown: ReadingFailure?
    get() = search.failure?.takeIf { search.tiles.isEmpty() && !(waited && search.loading) }

/** Whether moving from this to [next] changes what the grid shows, so Remove animations fades it. */
private fun ReadSearchState.drawsOtherThan(next: ReadSearchState): Boolean =
    tiles != next.tiles || failure != next.failure || query.sourcesOnly != next.query.sourcesOnly ||
        sources?.groups != next.sources?.groups || (loading && !next.loading)

/**
 * The search a grid draws. New words or filters keep the last answer on screen (the store marks
 * it stale) until theirs lands; only where nothing shows do placeholders come, from 600 ms, and
 * once there they stay at least 0.3 s before the answer replaces them. With Remove animations on,
 * each change swaps behind [fade]'s still, so the grid crossfades rather than cuts.
 */
@Composable
internal fun rememberShownSearch(search: ReadSearchState, fade: StillFade): ShownSearch {
    var shown by remember { mutableStateOf(search) }
    var waited by remember { mutableStateOf(false) }
    var placeholdersAt by remember { mutableLongStateOf(-1L) }
    val removed by rememberUpdatedState(rememberAnimationsRemoved())
    val latest by rememberUpdatedState(search)
    val busy = search.busy
    LaunchedEffect(busy) {
        if (!busy) {
            waited = false
            return@LaunchedEffect
        }
        delay(PLACEHOLDERS_AFTER)
        waited = true
        if (shown.empty) placeholdersAt = withFrameMillis { it }
    }
    LaunchedEffect(Unit) {
        snapshotFlow { latest }.collect { next ->
            if (next === shown) return@collect
            if (placeholdersAt >= 0 && !next.empty) {
                // Placeholders once shown stand 0.3 s, so they never flicker away.
                val held = withFrameMillis { it } - placeholdersAt
                if (held < PLACEHOLDERS_HELD) delay(PLACEHOLDERS_HELD - held)
                placeholdersAt = -1L
            }
            val now = latest
            if (removed && shown.drawsOtherThan(now)) fade.swap(fullSpeed = true) { shown = now } else shown = now
            if (waited && shown.empty && placeholdersAt < 0) placeholdersAt = withFrameMillis { it }
        }
    }
    return ShownSearch(shown, waited)
}

/**
 * When each discovery row started waiting, by its id, so a row scrolled away and back keeps its
 * 600 ms rather than starting it over. Read and written on the main thread only.
 */
internal object RowWaits {
    private val starts = HashMap<String, Long>()

    fun since(key: String, now: Long): Long = starts.getOrPut(key) { now }

    fun done(key: String) {
        starts.remove(key)
    }
}
