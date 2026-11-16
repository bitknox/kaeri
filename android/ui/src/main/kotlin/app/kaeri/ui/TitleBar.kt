package app.kaeri.ui

import androidx.compose.runtime.Stable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.staticCompositionLocalOf
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
 * scrolled [start], where the head starts to pass under it, and is solid [span] later and for the
 * rest of the page. With animations removed it switches at once, halfway.
 */
internal fun titleBarFill(firstIndex: Int, scrolled: Float, start: Float, span: Float, instant: Boolean): Float {
    val fill = if (firstIndex > 0) 1f else ((scrolled - start) / span).coerceIn(0f, 1f)
    return if (instant) (if (fill >= 0.5f) 1f else 0f) else fill
}

/** The show's name in the bar comes in over the second half of the fill, once the art is covered. */
internal fun titleBarNameAlpha(fill: Float): Float = ((fill - 0.5f) * 2f).coerceIn(0f, 1f)

/**
 * Where the bar starts to fill on each page, by its list: the head with a banner keeps it clear
 * while the art is in view, a compact head starts at once.
 */
@Stable
internal class TitleBarStarts {
    private val starts = mutableStateMapOf<Any, Dp>()

    operator fun get(page: Any): Dp? = starts[page]

    operator fun set(page: Any, start: Dp) {
        if (starts[page] != start) starts[page] = start
    }

    fun remove(page: Any) {
        starts.remove(page)
    }
}

internal val LocalTitleBarStarts = staticCompositionLocalOf<TitleBarStarts?> { null }

/** Where a phone's title header starts below the status bar, over the banner's foot. */
internal val PhoneHeaderTop = 112.dp

/** The tablet's banner below the status bar. */
internal val TabletBannerHeight = 208.dp
