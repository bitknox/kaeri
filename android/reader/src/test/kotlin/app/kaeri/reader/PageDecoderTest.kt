// Page files to bitmaps: raw greys mapped row by row whatever their stride, originals through the
// platform's decoder, a file gone from the cache said as missing, and the pages named to keep kept
// whatever the cache's size. A picture too big for a canvas to draw comes out within the budget,
// its shape kept.

package app.kaeri.reader

import android.graphics.Bitmap
import android.graphics.Color
import app.kaeri.engine.model.PageEncoding
import app.kaeri.engine.model.PageFile
import app.kaeri.engine.model.PixelFormat
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.File

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class PageDecoderTest {
    @get:Rule
    val dir = TemporaryFolder()

    private fun alpha(bitmap: Bitmap, x: Int, y: Int): Int = Color.alpha(bitmap.getPixel(x, y))

    @Test
    fun grey_pages_map_to_alpha_with_padded_rows() {
        val file = ReaderFixtures.grey(dir.root, "1.gray", width = 5, height = 3, stride = 8)
        val page = (PageDecoder.decode(file) as PageDecodeResult.Decoded).page
        assertTrue(page.grey)
        assertEquals(Bitmap.Config.ALPHA_8, page.bitmap.config)
        assertEquals(5, page.width)
        assertEquals(0, alpha(page.bitmap, 0, 2))
        assertEquals(255, alpha(page.bitmap, 4, 1))
        assertEquals(127, alpha(page.bitmap, 2, 0))
    }

    @Test
    fun rgba_pages_keep_their_colours() {
        val bytes = byteArrayOf(255.toByte(), 0, 0, 255.toByte(), 0, 0, 255.toByte(), 255.toByte())
        val path = File(dir.root, "1.rgba").apply { writeBytes(bytes) }.path
        val file = PageFile(path, PageEncoding.Raw(PixelFormat.RGBA8, 8u), 2u, 1u, null, true, 8u)
        val page = (PageDecoder.decode(file) as PageDecodeResult.Decoded).page
        assertEquals(Color.RED, page.bitmap.getPixel(0, 0))
        assertEquals(Color.BLUE, page.bitmap.getPixel(1, 0))
    }

    @Test
    fun originals_decode_with_the_crop() {
        val source = Bitmap.createBitmap(40, 60, Bitmap.Config.ARGB_8888).apply { eraseColor(Color.GREEN) }
        val png = File(dir.root, "1.png")
        png.outputStream().use { source.compress(Bitmap.CompressFormat.PNG, 100, it) }
        val file = PageFile(png.path, PageEncoding.Original("image/png"), 40u, 60u, app.kaeri.engine.model.PixelRect(5u, 10u, 30u, 40u), true, png.length().toULong())
        val page = (PageDecoder.decode(file) as PageDecodeResult.Decoded).page
        assertEquals(30, page.width)
        assertEquals(40, page.height)
        assertEquals(Color.GREEN, page.bitmap.getPixel(3, 3))
    }

    private fun png(name: String, width: Int, height: Int, colour: Int): File {
        val source = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888).apply { eraseColor(colour) }
        return File(dir.root, name).apply { outputStream().use { source.compress(Bitmap.CompressFormat.PNG, 100, it) } }
    }

    private fun original(png: File, width: Int, height: Int, crop: app.kaeri.engine.model.PixelRect? = null) =
        PageFile(png.path, PageEncoding.Original("image/png"), width.toUInt(), height.toUInt(), crop, true, png.length().toULong())

    @Test
    fun a_strip_image_too_big_to_draw_is_decoded_smaller_with_its_shape() {
        // A narrow, very tall strip image, as a long webtoon panel is; the budget stands in for the canvas's.
        val file = original(png("strip.png", 40, 2000, Color.GREEN), 40, 2000)
        val budget = 40L * 2000 * 4 / 4
        val page = (PageDecoder.decode(file, budget) as PageDecodeResult.Decoded).page
        assertTrue("${page.bitmap.byteCount} bytes", page.bitmap.byteCount <= budget)
        assertTrue("${page.width}x${page.height}", page.width in 19..20 && page.height in 999..1000)
        assertEquals(Color.GREEN, page.bitmap.getPixel(page.width / 2, page.height - 1))
    }

    @Test
    fun a_cropped_page_too_big_to_draw_keeps_its_crop_when_decoded_smaller() {
        val file = original(png("crop.png", 40, 60, Color.BLUE), 40, 60, app.kaeri.engine.model.PixelRect(5u, 10u, 30u, 40u))
        val budget = 30L * 40 * 4 / 4
        val page = (PageDecoder.decode(file, budget) as PageDecodeResult.Decoded).page
        assertTrue("${page.bitmap.byteCount} bytes", page.bitmap.byteCount <= budget)
        assertTrue("${page.width}x${page.height}", page.width in 14..15 && page.height in 19..20)
        assertEquals(Color.BLUE, page.bitmap.getPixel(1, 1))
    }

    @Test
    fun raw_pixels_too_big_to_draw_are_scaled_down() {
        val file = ReaderFixtures.grey(dir.root, "big.gray", width = 40, height = 40)
        val page = (PageDecoder.decode(file, budget = 400) as PageDecodeResult.Decoded).page
        assertTrue(page.grey)
        assertTrue("${page.bitmap.byteCount} bytes", page.bitmap.byteCount <= 400)
        assertEquals(20, page.width)
        assertEquals(20, page.height)
    }

    @Test
    fun the_canvas_budget_fits_a_strip_past_it() {
        // 720 by 40000 is about 110 MB as ARGB, past what a hardware canvas draws.
        val scale = PageDecoder.scaleWithin(720, 40_000, 4, PageDecoder.CANVAS_BYTES)
        assertTrue(scale < 1.0)
        val width = kotlin.math.floor(720 * scale).toLong()
        val height = kotlin.math.floor(40_000 * scale).toLong()
        assertTrue(width * height * 4 <= PageDecoder.CANVAS_BYTES)
        assertEquals(1.0, PageDecoder.scaleWithin(720, 10_000, 4, PageDecoder.CANVAS_BYTES), 0.0)
        assertEquals(2, PageDecoder.sampleWithin(720, 40_000, PageDecoder.CANVAS_BYTES))
        assertEquals(1, PageDecoder.sampleWithin(720, 10_000, PageDecoder.CANVAS_BYTES))
    }

    @Test
    fun a_file_gone_from_the_cache_is_missing() {
        val file = ReaderFixtures.grey(dir.root, "gone.gray", 2, 2)
        File(file.path).delete()
        assertEquals(PageDecodeResult.Missing, PageDecoder.decode(file))
        assertEquals(PageDecodeResult.Missing, PageImages().load(file))
    }

    @Test
    fun a_short_file_is_unreadable_not_a_crash() {
        val file = ReaderFixtures.grey(dir.root, "short.gray", 4, 4).copy(height = 40u)
        assertTrue(PageDecoder.decode(file) is PageDecodeResult.Unreadable)
    }

    @Test
    fun decoded_pages_are_kept() {
        val images = PageImages()
        val file = ReaderFixtures.grey(dir.root, "kept.gray", 3, 3)
        val first = (images.load(file) as PageDecodeResult.Decoded).page
        assertTrue(images.cached(file) === first)
    }

    @Test
    fun kept_pages_stay_when_the_cache_is_full_until_no_longer_named() {
        val images = PageImages(maxBytes = 4 * 4)
        val shown = ReaderFixtures.grey(dir.root, "shown.gray", 4, 4)
        val next = ReaderFixtures.grey(dir.root, "next.gray", 4, 4)
        images.keep(listOf(shown, next))
        images.load(shown)
        images.load(next)
        assertTrue(images.cached(shown) != null && images.cached(next) != null)
        images.keep(listOf(next))
        assertEquals(null, images.cached(shown))
        assertTrue(images.cached(next) != null)
    }
}
