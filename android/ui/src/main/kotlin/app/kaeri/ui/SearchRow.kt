// The pill row under the search field: what each pill says, and the fade at the row's edges. The
// words are worked out here, apart from the screen, so the row's states can be tested.

package app.kaeri.ui

import android.content.Context
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Immutable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import app.kaeri.engine.model.FilterKey
import app.kaeri.engine.model.SearchQuery
import app.kaeri.i18n.Strings
import app.kaeri.store.SearchWords

/** One pill of the row: its words, whether it has a value, and what a screen reader says. */
@Immutable
internal data class RowPill(
    val label: String,
    val set: Boolean,
    val enabled: Boolean = true,
    val spoken: String,
)

/**
 * The row as the query sets it. Sort, Genres, Year, Season, Format and Status scroll; Clear all
 * comes first, only while anything is set; the filters button is pinned at the end and counts what
 * only the sheet holds ([sheetCount]), since the pills already say their own.
 */
@Immutable
internal data class FilterRowState(
    val clearAll: Boolean,
    val sort: RowPill,
    val genres: RowPill,
    val year: RowPill,
    val season: RowPill,
    val format: RowPill,
    val status: RowPill,
    val sheetCount: Int,
    val sheetSpoken: String,
)

private const val NAMED_FORMATS = 2

/**
 * What each pill shows. On a phone a single genre is named, and up to two formats are joined;
 * past that a pill counts. A wide window keeps its earlier words.
 */
internal fun filterRowState(context: Context, query: SearchQuery, phone: Boolean): FilterRowState {
    fun pill(name: String, value: String?, enabled: Boolean = true, spoken: String? = null) = RowPill(
        label = value ?: name,
        set = value != null,
        enabled = enabled,
        spoken = spoken ?: if (value != null) Strings.searchPillSet(context, name, value) else Strings.searchPillAny(context, name),
    )

    val current = query.sortInEffect()
    // Trending says its period in a browse: "Trending this week".
    val sortWord = if (current == app.kaeri.engine.model.SearchSort.TRENDING && query.periodInEffect() != null) query.sortLabel() else SearchWords.sort(context, current)
    val sort = RowPill(sortWord, set = query.sort != null, spoken = Strings.sortLabel(context, sortWord))

    val pickedGenres = query.genres.size + query.genresExcluded.size
    val genresName = Strings.filterGenres(context)
    val genres = pill(
        genresName,
        when {
            pickedGenres == 0 -> null
            phone && pickedGenres == 1 -> query.active().first { it.key is FilterKey.Genre || it.key is FilterKey.NotGenre }.label
            else -> Strings.searchGenresPicked(context, pickedGenres)
        },
    )

    val yearName = Strings.filterYear(context)
    val year = pill(yearName, query.label(FilterKey.Years))

    val seasonName = Strings.filterSeason(context)
    val seasonValue = query.season?.let { SearchWords.season(context, it) }
    val season = pill(
        seasonName,
        seasonValue,
        enabled = query.singleYear != null || query.season != null,
        spoken = if (seasonValue == null && query.singleYear == null) seasonName + ", " + Strings.filterSeasonPickYear(context) else null,
    )

    val formatName = Strings.filterFormat(context)
    val formats = query.formats
    val format = pill(
        formatName,
        when {
            formats.isEmpty() -> null
            phone && formats.size <= NAMED_FORMATS -> formats.joinToString(", ") { SearchWords.format(context, it) }
            formats.size == 1 -> SearchWords.format(context, formats.first())
            else -> Strings.searchFormatsPicked(context, formats.size)
        },
    )

    val statusName = Strings.filterStatus(context)
    val statuses = query.statuses
    val status = pill(
        statusName,
        when (statuses.size) {
            0 -> null
            1 -> SearchWords.status(context, statuses.first())
            else -> Strings.searchStatusesPicked(context, statuses.size)
        },
    )

    val sheetCount = query.advancedCount().toInt()
    return FilterRowState(
        clearAll = query.active().isNotEmpty(),
        sort = sort,
        genres = genres,
        year = year,
        season = season,
        format = format,
        status = status,
        sheetCount = sheetCount,
        sheetSpoken = if (sheetCount > 0) Strings.searchPillAllSet(context, sheetCount) else Strings.filtersButton(context),
    )
}

/**
 * A fade at whichever edge of a sideways row has more beyond it, [width] wide: the pills thin out
 * where they carry on. The pinned button sits outside the row, so the fade ends where it starts.
 */
internal fun Modifier.edgeFades(state: LazyListState, width: Dp = 24.dp): Modifier = this
    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
    .drawWithContent {
        drawContent()
        val fade = width.toPx().coerceAtMost(size.width / 2)
        val ltr = layoutDirection == LayoutDirection.Ltr
        val before = state.canScrollBackward
        val after = state.canScrollForward
        fun edge(atLeft: Boolean) {
            val x = if (atLeft) 0f else size.width - fade
            val brush = Brush.horizontalGradient(
                0f to (if (atLeft) Color.Transparent else Color.Black),
                1f to (if (atLeft) Color.Black else Color.Transparent),
                startX = x,
                endX = x + fade,
            )
            drawRect(brush, topLeft = Offset(x, 0f), size = Size(fade, size.height), blendMode = BlendMode.DstIn)
        }
        if (before) edge(atLeft = ltr)
        if (after) edge(atLeft = !ltr)
    }
