// The surface the engine lays leaves out for: the reader's own size in pixels, the density, and
// whether there is room for two pages side by side.

package app.kaeri.reader

import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntSize
import app.kaeri.engine.model.ReaderSurface

/** How wide a landscape reader must be, in dp, before it shows two pages. */
const val TWO_PAGES_DP = 600f

/** The surface for a reader [size] pixels big. */
fun readerSurface(size: IntSize, density: Density, eink: Boolean, dark: Boolean, twoPagesAllowed: Boolean = true): ReaderSurface {
    val widthDp = size.width / density.density
    val two = twoPagesAllowed && size.width > size.height && widthDp >= TWO_PAGES_DP
    return ReaderSurface(
        widthPx = size.width.coerceAtLeast(1).toUInt(),
        heightPx = size.height.coerceAtLeast(1).toUInt(),
        scale = density.density,
        eink = eink,
        twoPages = two,
        dark = dark,
    )
}

/** Hands the reader's surface to [onSurface] whenever its size changes. */
fun Modifier.readerSurface(density: Density, eink: Boolean, dark: Boolean, onSurface: (ReaderSurface) -> Unit): Modifier =
    onSizeChanged { size -> if (size.width > 0 && size.height > 0) onSurface(readerSurface(size, density, eink, dark)) }
