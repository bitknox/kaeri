package app.kaeri.ui

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** The title page's top bar under the status bar: its buttons' row. */
internal val TitleBarHeight = 64.dp

/** How much of the page passes under the bar while it fills, from clear to solid. */
internal val TitleBarFillSpan = 64.dp

/**
 * How far the title page's top bar has filled, from 0 (clear, the round buttons over the art) to
 * 1 (solid ground with the show's name), read from where the page is scrolled rather than run on
 * a clock, so it reverses as the page comes back down. The bar stays clear until the page has
 * scrolled [start] and is solid [span] later and for the rest of the page. With animations
 * removed it switches at once, at [snap].
 */
internal fun titleBarFill(firstIndex: Int, scrolled: Float, start: Float, span: Float, snap: Float, instant: Boolean): Float {
    if (firstIndex > 0) return 1f
    if (instant) return if (scrolled >= snap) 1f else 0f
    return ((scrolled - start) / span).coerceIn(0f, 1f)
}

/** The show's name in the bar comes in over the second half of the fill, as the art leaves it. */
internal fun titleBarNameAlpha(fill: Float): Float = ((fill - 0.5f) * 2f).coerceIn(0f, 1f)

/**
 * Where the bar fills on one page, in how far its list has scrolled from the top: clear until
 * [start], solid [TitleBarFillSpan] later at [solid], and with animations removed switching at
 * once at [snap]. A head with a banner keeps the bar clear while any of the art shows below it.
 */
@Immutable
internal data class TitleBarFillRange(val start: Dp, val snap: Dp) {
    val solid: Dp get() = start + TitleBarFillSpan

    fun fill(firstIndex: Int, scrolled: Float, density: Density, instant: Boolean): Float = with(density) {
        titleBarFill(firstIndex, scrolled, start.toPx(), TitleBarFillSpan.toPx(), snap.toPx(), instant)
    }

    companion object {
        /**
         * Over a banner [height] tall below the status bar, which goes up by 1 - [parallax] of the
         * scroll (its parallax hands the rest back): solid just as the banner's foot passes under
         * the bar's foot, the status bar and [TitleBarHeight], filling over the last
         * [TitleBarFillSpan] of scroll before it. With animations removed it switches there too,
         * where the banner is gone.
         */
        fun overBanner(height: Dp, parallax: Float = 0f): TitleBarFillRange {
            val gone = (height - TitleBarHeight) / (1f - parallax)
            return TitleBarFillRange(gone - TitleBarFillSpan, gone)
        }

        /**
         * A head with no banner, its header [headerTop] below the status bar: the bar fills as the
         * header passes under it, and with animations removed switches halfway.
         */
        fun compact(headerTop: Dp): TitleBarFillRange {
            val start = headerTop - TitleBarHeight
            return TitleBarFillRange(start, start + TitleBarFillSpan / 2)
        }
    }
}

/** Where the bar fills on each page, by its list: over its banner, or as a compact head's header passes. */
@Stable
internal class TitleBarRanges {
    private val ranges = mutableStateMapOf<Any, TitleBarFillRange>()

    operator fun get(page: Any): TitleBarFillRange? = ranges[page]

    operator fun set(page: Any, range: TitleBarFillRange) {
        if (ranges[page] != range) ranges[page] = range
    }

    fun remove(page: Any) {
        ranges.remove(page)
    }
}

internal val LocalTitleBarRanges = staticCompositionLocalOf<TitleBarRanges?> { null }

/** Where a phone's title header starts below the status bar, over the banner's foot. */
internal val PhoneHeaderTop = 112.dp

/** The phone's banner below the status bar. */
internal val PhoneBannerHeight = 156.dp

/** How much of the scroll the phone's banner hands back as it goes up: half, so it moves at half the page's speed. */
internal const val PhoneBannerParallax = 0.5f

/** The tablet's banner below the status bar. */
internal val TabletBannerHeight = 208.dp
