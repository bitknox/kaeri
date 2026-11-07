// The platform's image decoder, for the page formats the engine cannot read: AVIF on an e-ink
// screen, where the engine dithers the pages itself.

package app.kaeri.store

import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.os.Build
import app.kaeri.engine.ffi.FfiDecodedImage
import app.kaeri.engine.ffi.FfiPageDecoder
import java.nio.ByteBuffer
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * The app's [FfiPageDecoder]: a file decoded with [ImageDecoder], in software memory so the pixels
 * can be read, and shrunk while it decodes to the size the engine asks for, which keeps a big scan
 * small on a reader with 2 to 4 GB. ImageDecoder reads AVIF from Android 12 (API 31); below that,
 * and for any file it fails on, the answer is null and the engine fails the page as unsupported.
 *
 * The engine calls it on its page worker's thread and gives up after two seconds.
 *
 * @param sdk the Android version, for a test to say it.
 */
class PlatformPageDecoder(private val sdk: Int = Build.VERSION.SDK_INT) : FfiPageDecoder {
    override fun decode(bytes: ByteArray, format: String, maxWidth: UInt, maxHeight: UInt): FfiDecodedImage? {
        if (sdk < Build.VERSION_CODES.S) return null
        return try {
            val source = ImageDecoder.createSource(ByteBuffer.wrap(bytes))
            val bitmap = ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                decoder.isUnpremultipliedRequired = true
                val wide = info.size.width
                val high = info.size.height
                val scale = min(1.0, min(maxWidth.toDouble() / wide, maxHeight.toDouble() / high))
                if (scale < 1.0 && scale > 0.0) {
                    decoder.setTargetSize(max(1, (wide * scale).roundToInt()), max(1, (high * scale).roundToInt()))
                }
            }
            try {
                rgba(bitmap)
            } finally {
                bitmap.recycle()
            }
        } catch (e: Exception) {
            null
        } catch (e: OutOfMemoryError) {
            null
        }
    }

    /**
     * The bitmap's pixels as packed RGBA8 rows. Read through [Bitmap.getPixels], a band of rows at a
     * time, so the byte order does not depend on how the platform lays the memory out.
     */
    private fun rgba(bitmap: Bitmap): FfiDecodedImage {
        val width = bitmap.width
        val height = bitmap.height
        val out = ByteArray(width * height * 4)
        val band = max(1, BAND_PIXELS / max(1, width))
        val argb = IntArray(width * band)
        var y = 0
        while (y < height) {
            val rows = min(band, height - y)
            bitmap.getPixels(argb, 0, width, 0, y, width, rows)
            var at = y * width * 4
            for (i in 0 until width * rows) {
                val p = argb[i]
                out[at] = (p shr 16).toByte()
                out[at + 1] = (p shr 8).toByte()
                out[at + 2] = p.toByte()
                out[at + 3] = (p ushr 24).toByte()
                at += 4
            }
            y += rows
        }
        return FfiDecodedImage(width = width.toUInt(), height = height.toUInt(), rgba = out)
    }

    private companion object {
        const val BAND_PIXELS = 1 shl 20
    }
}
