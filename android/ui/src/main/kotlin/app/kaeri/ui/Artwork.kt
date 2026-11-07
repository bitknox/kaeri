// Covers and banners: the plate first, and the art fading in over it once it arrives.

package app.kaeri.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import app.kaeri.design.ArtShape
import app.kaeri.design.CoverPlate
import app.kaeri.design.Motion
import app.kaeri.store.AppStore
import app.kaeri.store.Artwork
import app.kaeri.engine.model.Entry

/** The store the screens read and send through. */
val LocalAppStore = staticCompositionLocalOf<AppStore> { error("no AppStore in this composition") }

/**
 * A cover or banner filling its box: the plate in [plate], and the art from [url] fading in over
 * it when it arrives, never popping in. Asks for it at [maxPixelSize] pixels on its long side.
 */
@Composable
fun ArtworkImage(url: String?, plate: Color?, maxPixelSize: Int, modifier: Modifier = Modifier, softenTo: Int? = null) {
    val store = LocalAppStore.current
    // This URL's arrivals alone: another cover landing leaves this one be.
    val arrived = remember(url) { url?.let(store.artwork::arrivals) ?: NONE_ARRIVED }
    val arrivals by arrived.collectAsStateWithLifecycle()
    // Art on screen is held while it is: the store's budget letting it go never drops it to its plate.
    val held = remember(url) { HeldArt() }
    val art = url?.let { arrivals; store.artwork.image(it) }?.also { held.art = it } ?: held.art
    LaunchedEffect(url, maxPixelSize) {
        if (url != null) store.artwork.request(url, maxPixelSize)
    }
    // Art already here when the box first draws shows at once; art that arrives fades in.
    val shown = remember(url) { Animatable(if (art != null) 1f else 0f) }
    LaunchedEffect(url, art != null) {
        if (art != null) shown.animateTo(1f, Motion.fadeIn())
    }
    Box(modifier) {
        CoverPlate(plate, Modifier.fillMaxSize())
        if (art != null) {
            val bitmap = remember(art, softenTo) { (softenTo?.let(art.image::softened) ?: art.image).asImageBitmap() }
            Image(
                bitmap,
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxSize().graphicsLayer { alpha = shown.value },
            )
        }
    }
}

/**
 * A blur where Compose has none, below Android 12: [this] drawn [across] pixels wide, which the
 * box's smooth scaling spreads into soft colour.
 */
internal fun android.graphics.Bitmap.softened(across: Int): android.graphics.Bitmap {
    val w = across.coerceIn(1, width)
    return android.graphics.Bitmap.createScaledBitmap(this, w, (height.toLong() * w / width).toInt().coerceAtLeast(1), true)
}

/**
 * Whether Compose can blur, from Android 12 (RenderEffect); below, a blur draws nothing and
 * art softens with [softened] instead.
 */
@get:androidx.annotation.ChecksSdkIntAtLeast(api = android.os.Build.VERSION_CODES.S)
internal val canBlur: Boolean get() = android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S

/** The art a box last showed for its URL. */
private class HeldArt { var art: Artwork? = null }

/** What a box with no URL observes: nothing ever lands. */
private val NONE_ARRIVED: StateFlow<Int> = MutableStateFlow(0)

/** The pixels a box [dp] wide or high needs. */
@Composable
fun pixels(dp: Dp): Int = with(LocalDensity.current) { dp.toPx().roundToInt() }

/** A title's poster at [width], rounded by its width. */
@Composable
fun CoverArt(entry: Entry, width: Dp, modifier: Modifier = Modifier) {
    ArtworkImage(
        url = entry.coverUrl,
        plate = null,
        maxPixelSize = pixels(width * 1.5f),
        modifier = modifier.clip(ArtShape.cover(width)),
    )
}

/**
 * A card's wide art: the banner, else the cover at its own aspect against the leading edge of a
 * plate, never a portrait cover stretched across a banner's frame.
 */
@Composable
fun BannerArt(entry: Entry, modifier: Modifier = Modifier) {
    BoxWithConstraints(modifier) {
        val banner = entry.bannerUrl
        val height = maxHeight
        if (banner != null) {
            ArtworkImage(banner, plate = null, maxPixelSize = pixels(maxWidth), modifier = Modifier.fillMaxSize())
        } else {
            val inset = height * 0.06f
            Box(Modifier.fillMaxSize()) {
                CoverPlate(null, Modifier.fillMaxSize())
                ArtworkImage(
                    entry.coverUrl,
                    plate = null,
                    maxPixelSize = pixels(height),
                    modifier = Modifier
                        .align(Alignment.CenterStart)
                        .padding(start = inset)
                        .fillMaxHeight(fraction = 1f - 0.12f)
                        .aspectRatio(2f / 3f)
                        .clip(ArtShape.cover(height * (2f / 3f))),
                )
            }
        }
    }
}
