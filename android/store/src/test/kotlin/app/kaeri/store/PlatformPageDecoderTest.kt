package app.kaeri.store

import android.graphics.Bitmap
import android.graphics.Color
import java.io.ByteArrayOutputStream
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The platform's page decoder, on a PNG because Robolectric's native graphics cannot read AVIF
 * (the decoder is the same call for every format). A real AVIF page is checked on the emulator.
 * The picture is 240 by 360: red on the left half, blue on the right.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class PlatformPageDecoderTest {
    private val png: ByteArray = run {
        val bitmap = Bitmap.createBitmap(240, 360, Bitmap.Config.ARGB_8888)
        for (x in 0 until 240) for (y in 0 until 360) bitmap.setPixel(x, y, if (x < 120) Color.RED else Color.BLUE)
        ByteArrayOutputStream().also { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }.toByteArray()
    }

    private fun pixel(rgba: ByteArray, width: Int, x: Int, y: Int): List<Int> =
        (0 until 4).map { rgba[(y * width + x) * 4 + it].toInt() and 0xFF }

    @Test
    fun a_page_decodes_to_packed_rgba_at_its_own_size() {
        val image = PlatformPageDecoder().decode(png, "image/avif", 1000u, 1000u)!!
        assertEquals(240u, image.width)
        assertEquals(360u, image.height)
        assertEquals(240 * 360 * 4, image.rgba.size)
        assertEquals(listOf(255, 0, 0, 255), pixel(image.rgba, 240, 10, 10))
        assertEquals(listOf(0, 0, 255, 255), pixel(image.rgba, 240, 230, 350))
    }

    @Test
    fun a_page_larger_than_asked_for_is_decoded_smaller_with_its_shape_kept() {
        val image = PlatformPageDecoder().decode(png, "image/avif", 120u, 1000u)!!
        assertEquals(120u, image.width)
        assertEquals(180u, image.height)
        assertEquals(120 * 180 * 4, image.rgba.size)
        assertEquals(listOf(255, 0, 0, 255), pixel(image.rgba, 120, 5, 5))
    }

    @Test
    fun below_android_12_there_is_no_answer() {
        assertNull(PlatformPageDecoder(sdk = 30).decode(png, "image/avif", 1000u, 1000u))
    }

    @Test
    fun a_file_that_is_not_an_image_has_no_answer() {
        assertNull(PlatformPageDecoder().decode(ByteArray(64) { 7 }, "image/avif", 1000u, 1000u))
    }
}
