// Covers and banners: the plate first, and the art fading in over it once it arrives. Wide art
// for cards fills its frame: banner, seen still, the cover cropped or blurred, or a titled plate.

package app.kaeri.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import app.kaeri.design.ArtShape
import app.kaeri.design.CoverPlate
import app.kaeri.design.Motion
import app.kaeri.design.ShowTitle
import app.kaeri.store.AppStore
import app.kaeri.store.Artwork
import app.kaeri.store.HeroArt
import app.kaeri.engine.model.Entry

/** The store the screens read and send through. */
val LocalAppStore = staticCompositionLocalOf<AppStore> { error("no AppStore in this composition") }

/**
 * A cover or banner filling its box: the plate in [plate], and the art from [url] fading in over
 * it when it arrives, never popping in. Asks for it at [maxPixelSize] pixels on its long side.
 * [onPlate] false leaves the plate out, for art fading in over other art.
 */
@Composable
fun ArtworkImage(url: String?, plate: Color?, maxPixelSize: Int, modifier: Modifier = Modifier, softenTo: Int? = null, onPlate: Boolean = true) {
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
        if (onPlate) CoverPlate(plate, Modifier.fillMaxSize())
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

/** The hairline edge of wide art, silk at 8 %, so a light picture does not bleed into a light neighbour. */
internal val ArtEdge = app.kaeri.design.ObiSilk.Silk.copy(alpha = 0.08f)

/**
 * A card's wide art, for Continue, the spotlight, the party lobby and Watch together: the banner,
 * else the still of an episode already seen (the engine sends it with the title), else the cover
 * filling the frame ([CoverFill]), else, with no cover at all, a plate in the cover's colour with
 * the title on it. Every frame stays one wide picture, never a portrait cover beside an empty slab.
 * A still or banner that lands after the cover filled the frame fades in over the cover, never
 * through the plate.
 */
@Composable
fun BannerArt(entry: Entry, modifier: Modifier = Modifier) {
    val art = HeroArt.card(entry)
    val plate = CoverPlate.parse(entry.coverColor)
    var coverShown by remember(entry.id) { mutableStateOf(art is HeroArt.Cover) }
    LaunchedEffect(art) { if (art is HeroArt.Cover) coverShown = true }
    BoxWithConstraints(modifier) {
        val width = maxWidth
        val height = maxHeight
        val cover = entry.coverUrl
        when (art) {
            is HeroArt.Cover -> CoverFill(art.cover, plate, width, height)
            is HeroArt.Banner, is HeroArt.Still -> {
                if (coverShown && cover != null) CoverFill(cover, plate, width, height)
                ArtworkImage(art.url, plate = plate, maxPixelSize = pixels(width), modifier = Modifier.fillMaxSize(), onPlate = !(coverShown && cover != null))
            }
            else -> TitlePlate(entry.showTitle, plate, width, height)
        }
    }
}

/** How a cover fills a wide frame: cropped around its upper third, or blurred behind itself. */
internal enum class CoverFillMode {
    CROP,
    BLUR,
    ;

    companion object {
        /** Frames up to this wide are cards: small enough on screen to enlarge a cover further. */
        val CARD_WIDTH = 300.dp

        /**
         * The most a crop may enlarge the file on a card. A 460 px cover, AniList's largest, fills
         * a 208 dp Continue card at 1.19 times on a 2.625 density phone, 1.36 at 3 and 1.58 at 3.5.
         */
        const val CARD_LIMIT = 1.6f

        /** The most a crop may enlarge the file in a wider frame: the spotlight blurs. */
        const val LIMIT = 1.33f

        /** Where a cover's focus is, a fraction of its height from the top: its upper third. */
        const val FOCUS = 1f / 3f

        fun limit(width: Dp): Float = if (width <= CARD_WIDTH) CARD_LIMIT else LIMIT

        /**
         * The mode for a frame [width] wide, [pixels] on the screen, from a file [source] pixels
         * wide: a crop while the file's size is not known. A cover's 2:3 is narrower than any
         * frame it fills, so its width sets the enlargement.
         */
        fun of(width: Dp, pixels: Int, source: Int?): CoverFillMode =
            if (source == null || source <= 0 || pixels <= source * limit(width)) CROP else BLUR

        /**
         * Where the top of a cover [width] wide sits in a frame [height] high so its focus is at
         * the frame's middle, without showing past either end: zero or less.
         */
        fun cropTop(width: Float, height: Float, focus: Float = FOCUS): Float {
            val tall = width * 3f / 2f
            return minOf(0f, maxOf(height - tall, height / 2f - tall * focus))
        }
    }
}

/**
 * A cover filling a [width] by [height] frame, by [CoverFillMode]: on the plate in [plate] until
 * it lands, then faded in on the fade-in token, already cropped or blurred, so it never moves.
 */
@Composable
internal fun CoverFill(url: String, plate: Color?, width: Dp, height: Dp, modifier: Modifier = Modifier) {
    val store = LocalAppStore.current
    val across = pixels(width)
    // The cover's long side as it lies across the frame.
    val ask = pixels(width * 1.5f)
    val arrived = remember(url) { store.artwork.arrivals(url) }
    val arrivals by arrived.collectAsStateWithLifecycle()
    val held = remember(url) { HeldArt() }
    val art = url.let { arrivals; store.artwork.image(it) }?.also { held.art = it } ?: held.art
    LaunchedEffect(url, ask) { store.artwork.request(url, ask) }
    val shown = remember(url) { Animatable(if (art != null) 1f else 0f) }
    LaunchedEffect(url, art != null) {
        if (art != null) shown.animateTo(1f, Motion.fadeIn())
    }
    Box(modifier.fillMaxSize().clipToBounds()) {
        CoverPlate(plate, Modifier.fillMaxSize())
        if (art != null) {
            val bitmap = remember(art) { art.image.asImageBitmap() }
            Box(Modifier.fillMaxSize().graphicsLayer { alpha = shown.value }) {
                when (CoverFillMode.of(width, across, art.sourceWidth)) {
                    CoverFillMode.CROP -> CoverCrop(bitmap, width, height)
                    CoverFillMode.BLUR -> BlurredCover(art, bitmap, width, height)
                }
            }
        }
    }
}

/** [bitmap] across the frame's width, its upper third at the frame's middle. */
@Composable
private fun CoverCrop(bitmap: ImageBitmap, width: Dp, height: Dp) {
    val top = CoverFillMode.cropTop(width.value, height.value).dp
    Image(bitmap, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.placedAt(0.dp, top, width, width * 1.5f))
}

/**
 * A cover too small to crop sharply: a blurred enlargement of it, darkened to 55 % and a touch
 * more saturated, with the whole cover sharp on it at 86 % of the height, edged in [ArtEdge].
 * Below Android 12, which cannot blur, the enlargement is drawn from a few pixels and spread.
 */
@Composable
private fun BlurredCover(art: Artwork, bitmap: ImageBitmap, width: Dp, height: Dp) {
    val groundWidth = width * 1.24f
    val top = CoverFillMode.cropTop(width.value, height.value).dp - width * 0.12f
    val soft = remember(art) { if (canBlur) bitmap else art.image.softened(COVER_SOFT_PIXELS).asImageBitmap() }
    val darkened = remember { ColorFilter.colorMatrix(darkened()) }
    Image(
        soft,
        contentDescription = null,
        contentScale = ContentScale.Crop,
        colorFilter = darkened,
        modifier = Modifier
            .placedAt(-width * 0.12f, top, groundWidth, groundWidth * 1.5f)
            .let { if (canBlur) it.blur(maxOf(4.dp, width / 18f), BlurredEdgeTreatment.Rectangle) else it },
    )
    val sharpHeight = (height * 0.86f).value.roundToInt().dp
    val sharpWidth = (sharpHeight * (2f / 3f)).value.roundToInt().dp
    val shape = ArtShape.cover(sharpWidth)
    Image(
        bitmap,
        contentDescription = null,
        contentScale = ContentScale.Crop,
        modifier = Modifier
            .placedAt((width - sharpWidth) / 2f, (height - sharpHeight) / 2f, sharpWidth, sharpHeight)
            .clip(shape)
            .border(1.dp, ArtEdge, shape),
    )
}

/** Brightness at 55 % and saturation at 110 %, as the blurred cover is drawn. */
private fun darkened(): ColorMatrix {
    val matrix = ColorMatrix().apply { setToSaturation(1.1f) }
    matrix.timesAssign(
        ColorMatrix(
            floatArrayOf(
                0.55f, 0f, 0f, 0f, 0f,
                0f, 0.55f, 0f, 0f, 0f,
                0f, 0f, 0.55f, 0f, 0f,
                0f, 0f, 0f, 1f, 0f,
            ),
        ),
    )
    return matrix
}

/** How many pixels across the blurred cover is drawn from below Android 12. */
private const val COVER_SOFT_PIXELS = 12

/**
 * No cover at all: the plate in the cover's colour, with [title] in show type at its foot. A frame
 * too small for a line of it keeps the plate alone. The title is said under the card already.
 */
@Composable
private fun TitlePlate(title: String, plate: Color?, width: Dp, height: Dp) {
    Box(Modifier.fillMaxSize().background(CoverPlate.muted(plate))) {
        if (width >= TITLE_PLATE_MIN_WIDTH) {
            Text(
                title,
                style = if (height > 150.dp) ShowTitle.page else ShowTitle.row,
                color = app.kaeri.design.ObiSilk.Silk,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier
                    .align(Alignment.BottomStart)
                    .padding(start = 12.dp, end = 12.dp, bottom = 10.dp)
                    .clearAndSetSemantics {},
            )
        }
    }
}

/** The narrowest frame the plate's title goes on: Watch together's small thumbnails keep the plate alone. */
private val TITLE_PLATE_MIN_WIDTH = 120.dp

/** Lays the content out at [width] by [height], its top left at [x], [y] in the box, whatever the box's own size. */
private fun Modifier.placedAt(x: Dp, y: Dp, width: Dp, height: Dp): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(Constraints.fixed(width.roundToPx().coerceAtLeast(0), height.roundToPx().coerceAtLeast(0)))
    layout(constraints.maxWidth, constraints.maxHeight) { placeable.place(x.roundToPx(), y.roundToPx()) }
}
