// One leaf: the parts the engine placed on it, each page drawn into its rectangle, with the waiting
// plate and the failed slot where a page has not come. The look of those two belongs to the screen
// (a raised plate on a phone, one static line on e-ink), so they are slots with phone defaults.

package app.kaeri.reader

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.robotoFlex
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.LeafPart
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.i18n.Strings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlin.math.roundToInt

/** How long a page may take before its waiting plate shows, and before the plate names it. */
object PageWaits {
    const val PLATE_MS = 600L
    const val WORDS_MS = 4_000L
}

/** What a waiting page shows: the page, and whether it has waited long enough to say so. */
typealias WaitingSlot = @Composable BoxScope.(page: Int, long: Boolean) -> Unit

/** What a failed page shows: the page, why, and the way to try again. */
typealias FailedSlot = @Composable BoxScope.(page: Int, failure: ReadingFailure, retry: () -> Unit) -> Unit

/**
 * Draws [leaf]. [instant] puts a page in at once, as e-ink and Paper mode do; otherwise it fades in
 * over its plate. [onReady] runs after the frame in which every part was drawn or failed, which is
 * when an e-ink screen refreshes.
 */
@Composable
fun LeafView(
    leaf: Leaf,
    state: ReaderState,
    images: PageImages,
    onMissing: (LeafPart) -> Unit,
    onRetry: (page: Int) -> Unit,
    modifier: Modifier = Modifier,
    instant: Boolean = false,
    onReady: () -> Unit = {},
    waiting: WaitingSlot = { page, long -> PageWaiting(page, long) },
    failed: FailedSlot = { page, failure, retry -> PageFailed(page, failure, retry) },
) {
    val ready = remember(leaf) { mutableStateMapOf<Int, Boolean>() }
    BoxWithConstraints(modifier.fillMaxSize()) {
        val width = constraints.maxWidth
        val height = constraints.maxHeight
        leaf.parts.forEachIndexed { index, part ->
            val place = part.place
            val x = (place.x * width).roundToInt()
            val y = (place.y * height).roundToInt()
            val w = (place.width * width).roundToInt().coerceAtLeast(1)
            val h = (place.height * height).roundToInt().coerceAtLeast(1)
            val density = androidx.compose.ui.platform.LocalDensity.current
            Box(
                Modifier
                    .offset { IntOffset(x, y) }
                    .size(with(density) { w.toDp() }, with(density) { h.toDp() }),
            ) {
                PartView(
                    part = part,
                    state = state,
                    images = images,
                    instant = instant,
                    onMissing = onMissing,
                    onReady = { ready[index] = true },
                    waiting = waiting,
                    failed = { page, failure, retry ->
                        LaunchedEffect(Unit) { ready[index] = true }
                        failed(page, failure, retry)
                    },
                    onRetry = onRetry,
                )
            }
        }
    }
    val all = leaf.parts.isNotEmpty() && leaf.parts.indices.all { ready[it] == true }
    LaunchedEffect(all, leaf) {
        if (all) {
            withFrameNanos { }
            onReady()
        }
    }
}

@Composable
private fun BoxScope.PartView(
    part: LeafPart,
    state: ReaderState,
    images: PageImages,
    instant: Boolean,
    onMissing: (LeafPart) -> Unit,
    onReady: () -> Unit,
    waiting: WaitingSlot,
    failed: FailedSlot,
    onRetry: (page: Int) -> Unit,
) {
    val page = part.page.toInt()
    val failure = state.failures[part.page]
    val file = state.file(part)
    if (failure != null && file == null) {
        failed(page, failure) { onRetry(page) }
        return
    }
    val wasCached = remember(file?.path) { file?.let(images::cached) != null }
    val result by produceState(file?.let(images::cached)?.let { PageDecodeResult.Decoded(it) } as PageDecodeResult?, file) {
        if (file != null && value == null) value = withContext(Dispatchers.IO) { images.load(file) }
    }
    when (val now = result) {
        is PageDecodeResult.Decoded -> {
            // A page that was there at once goes in at once; one that came later fades over its plate.
            val alpha = remember(file?.path) { Animatable(if (instant || wasCached) 1f else 0f) }
            LaunchedEffect(now) {
                if (!instant) alpha.animateTo(1f, Motion.fadeIn()) else alpha.snapTo(1f)
            }
            if (alpha.value < 1f) Waited(page, waiting)
            PageCanvas(now.page, rotate = part.rotate, alpha = alpha.value, modifier = Modifier.fillMaxSize())
            LaunchedEffect(now) { onReady() }
        }
        is PageDecodeResult.Missing -> {
            LaunchedEffect(file) { onMissing(part) }
            Waited(page, waiting)
        }
        is PageDecodeResult.Unreadable -> failed(page, ReadingFailure.Other(now.reason)) { onRetry(page) }
        null -> Waited(page, waiting)
    }
}

/** Nothing for the first 600 ms, then the plate, then its words at four seconds. */
@Composable
private fun BoxScope.Waited(page: Int, waiting: WaitingSlot) {
    var shown by remember { mutableStateOf(0) }
    LaunchedEffect(Unit) {
        delay(PageWaits.PLATE_MS)
        shown = 1
        delay(PageWaits.WORDS_MS - PageWaits.PLATE_MS)
        shown = 2
    }
    if (shown > 0) waiting(page, shown > 1)
}

/** A decoded page, fitted into its box; a grey page is drawn white over black. */
@Composable
fun PageCanvas(page: PageBitmap, rotate: Boolean, modifier: Modifier = Modifier, alpha: Float = 1f) {
    val image = remember(page) { page.bitmap.asImageBitmap() }
    Canvas(modifier) {
        val turned = rotate
        val srcW = if (turned) image.height else image.width
        val srcH = if (turned) image.width else image.height
        val scale = minOf(size.width / srcW, size.height / srcH)
        fun DrawScope.draw() {
            // Unrotated, the image's own box centred where the turned page goes.
            val iw = image.width * scale
            val ih = image.height * scale
            val dst = IntOffset((size.width / 2 - iw / 2).roundToInt(), (size.height / 2 - ih / 2).roundToInt())
            val dstSize = IntSize(iw.roundToInt(), ih.roundToInt())
            if (page.grey) {
                drawRect(Color.Black, Offset(dst.x.toFloat(), dst.y.toFloat()), Size(iw, ih), alpha = alpha)
                drawImage(
                    image, dstOffset = dst, dstSize = dstSize, alpha = alpha,
                    colorFilter = ColorFilter.tint(Color.White, BlendMode.SrcIn), filterQuality = FilterQuality.Medium,
                )
            } else {
                drawImage(image, dstOffset = dst, dstSize = dstSize, alpha = alpha, filterQuality = FilterQuality.Medium)
            }
        }
        if (turned) rotate(90f) { draw() } else draw()
    }
}

/** The phone's waiting plate: a raised panel with the page number, and its words at four seconds. */
@Composable
fun BoxScope.PageWaiting(page: Int, long: Boolean) {
    val context = LocalContext.current
    Box(Modifier.fillMaxSize().padding(24.dp).background(ObiSilk.Raised, RoundedCornerShape(12.dp))) {
        Column(Modifier.align(Alignment.Center), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                (page + 1).toString(),
                color = ObiSilk.Haze,
                fontFamily = robotoFlex(FontWeight.Medium, 34.sp),
                fontSize = 34.sp,
            )
            if (long) {
                Spacer(Modifier.height(8.dp))
                Text(Strings.readerPageLoading(context, page + 1), color = ObiSilk.Haze, fontSize = 15.sp)
            }
        }
    }
}

/** The phone's failed page: the glyph, why, Try again, and that a swipe goes on without it. */
@Composable
fun BoxScope.PageFailed(page: Int, failure: ReadingFailure, retry: () -> Unit) {
    val context = LocalContext.current
    Column(
        Modifier.align(Alignment.Center).widthIn(max = 360.dp).padding(28.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        BrokenPageGlyph(ObiSilk.Vermilion, Modifier.size(44.dp))
        Text(
            Strings.readerPageFailed(context, page + 1),
            color = ObiSilk.Silk,
            fontFamily = robotoFlex(FontWeight.SemiBold, 20.sp),
            fontSize = 20.sp,
            textAlign = TextAlign.Center,
        )
        Text(failure.text(context), color = ObiSilk.Haze, fontSize = 15.sp, textAlign = TextAlign.Center)
        Button(
            onClick = retry,
            colors = ButtonDefaults.buttonColors(containerColor = ObiSilk.Amber, contentColor = ObiSilk.Ground),
        ) { Text(Strings.readerTryAgain(context)) }
        Text(Strings.readerPageFailedSwipe(context), color = ObiSilk.Haze, fontSize = 13.sp, textAlign = TextAlign.Center)
    }
}

/** A page with its corner torn: the failed page's mark, in [colour]. */
@Composable
fun BrokenPageGlyph(colour: Color, modifier: Modifier = Modifier) {
    Canvas(modifier) {
        val s = size.minDimension
        val stroke = androidx.compose.ui.graphics.drawscope.Stroke(width = s * 0.07f)
        val path = androidx.compose.ui.graphics.Path().apply {
            moveTo(s * 0.2f, s * 0.08f)
            lineTo(s * 0.62f, s * 0.08f)
            lineTo(s * 0.8f, s * 0.26f)
            lineTo(s * 0.8f, s * 0.5f)
            lineTo(s * 0.66f, s * 0.6f)
            lineTo(s * 0.74f, s * 0.72f)
            lineTo(s * 0.8f, s * 0.92f)
            lineTo(s * 0.2f, s * 0.92f)
            close()
        }
        drawPath(path, colour, style = stroke)
    }
}
