// Page files to bitmaps: raw greys mapped row by row whatever their stride, originals through the
// platform's decoder, a file gone from the cache said as missing, and the pages named to keep kept
// whatever the cache's size.

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
