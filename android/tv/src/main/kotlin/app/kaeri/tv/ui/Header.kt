// The header Continue's hero and the title page share: the banner bleeding full width from y 0 to
// 200 with no text on it, then the title on the ground at the keyline. With no banner and no
// still safe to show, a compact header: the poster at the keyline beside the title.

package app.kaeri.tv.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.tv.material3.Text
import app.kaeri.design.CoverPlate
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.store.HeroArt

/** The banner's height on the canvas. */
val BannerHeight = 200.dp

/** The banners are decoded 1920 px wide at most, the canvas's width in pixels. */
const val BANNER_PIXELS = 1920

/**
 * The banner, or the poster, and the title, with [below] under the title. With [crossfade], as
 * on a title page whose seasons change in place, another title's art and name crossfade in over
 * these on standard rather than replacing them in a frame; [below] stays as it is.
 */
@Composable
fun HeroHeader(entry: Entry, rows: List<EpisodeRow>?, modifier: Modifier = Modifier, crossfade: Boolean = false, below: @Composable ColumnScope.() -> Unit) {
    val art = HeroArt.of(entry, rows)
    if (art == HeroArt.Compact) {
        Row(modifier.fillMaxWidth().padding(start = Canvas.Keyline, top = Canvas.SafeY + 20.dp, end = Canvas.SafeX)) {
            Swap(entry.coverUrl to entry.coverColor, crossfade) { (cover, color) ->
                ArtImage(
                    cover,
                    CoverPlate.parse(color),
                    pixels(Canvas.PosterHeight),
                    Modifier.size(Canvas.PosterWidth, Canvas.PosterHeight).clip(RoundedCornerShape(8.dp)),
                )
            }
            Spacer(Modifier.width(24.dp))
            Column(Modifier.padding(top = 8.dp)) {
                Swap(entry.title, crossfade) { HeaderTitle(it) }
                below()
            }
        }
        return
    }
    Column(modifier.fillMaxWidth()) {
        Swap(art.url, crossfade, Modifier.fillMaxWidth().height(BannerHeight)) { url ->
            // Waiting for the episode list keeps the ground: never a cover stretched across.
            if (url != null) ArtImage(url, plate = ObiSilk.Ground, maxPixelSize = BANNER_PIXELS, modifier = Modifier.fillMaxSize())
        }
        Column(Modifier.padding(start = Canvas.Keyline, end = Canvas.SafeX, top = 12.dp)) {
            Swap(entry.title, crossfade) { HeaderTitle(it) }
            below()
        }
    }
}

/** [content] for [value]; with [crossfade], a new value crossfades in over the old on standard. */
@Composable
fun <T> Swap(value: T, crossfade: Boolean, modifier: Modifier = Modifier, content: @Composable (T) -> Unit) {
    if (crossfade && !LocalReducedMotion.current) {
        androidx.compose.animation.Crossfade(value, modifier, animationSpec = app.kaeri.design.Motion.standard(), label = "swap") { content(it) } // i18n-skip
    } else {
        Box(modifier) { content(value) }
    }
}

@Composable
private fun HeaderTitle(title: String) {
    // Two lines before the ellipsis: long titles are common, and the second line costs little.
    Text(title, style = TvTitle.page, color = ObiSilk.Silk, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.semantics { heading() })
}
