// Page files to bitmaps. The engine writes each page to its cache whole, as the book had it or as
// raw pixels ready to map (a grey e-ink page is Gray8), and names the crop to show. Decoding runs
// off the main thread and the bitmaps are kept in a cache sized from the app's memory.

package app.kaeri.reader

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.BitmapRegionDecoder
import android.graphics.ImageDecoder
import android.graphics.Rect
import android.util.LruCache
import app.kaeri.engine.model.PageEncoding
import app.kaeri.engine.model.PageFile
import app.kaeri.engine.model.PixelFormat
import java.io.File
import java.io.FileNotFoundException
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.channels.FileChannel

/** A decoded page. [grey] pages are alpha-only: draw them white over black. */
class PageBitmap(val bitmap: Bitmap, val grey: Boolean) {
    val width: Int get() = bitmap.width
    val height: Int get() = bitmap.height
}

/** Why a page file gave no bitmap. */
sealed interface PageDecodeResult {
    data class Decoded(val page: PageBitmap) : PageDecodeResult

    /** The file is gone from the cache: ask the engine for it again. */
    data object Missing : PageDecodeResult

    /** The file is there but could not be read as a picture. */
    data class Unreadable(val reason: String) : PageDecodeResult
}

/** Decodes page files, the engine's crop applied, and raw pixels mapped as they are. */
object PageDecoder {
    fun decode(file: PageFile): PageDecodeResult = try {
        val source = File(file.path)
        if (!source.isFile) {
            PageDecodeResult.Missing
        } else {
            when (val encoding = file.encoding) {
                is PageEncoding.Original -> PageDecodeResult.Decoded(PageBitmap(original(source, file), grey = false))
                is PageEncoding.Raw -> PageDecodeResult.Decoded(raw(source, file, encoding))
            }
        }
    } catch (_: FileNotFoundException) {
        PageDecodeResult.Missing
    } catch (e: Exception) {
        PageDecodeResult.Unreadable(e.message ?: e.javaClass.simpleName)
    } catch (e: OutOfMemoryError) {
        PageDecodeResult.Unreadable(e.message ?: "out of memory")
    }

    private fun original(source: File, file: PageFile): Bitmap {
        val crop = file.crop?.let { Rect(it.x.toInt(), it.y.toInt(), (it.x + it.width).toInt(), (it.y + it.height).toInt()) }
        val decoded = runCatching {
            ImageDecoder.decodeBitmap(ImageDecoder.createSource(source)) { decoder, _, _ ->
                // Software pixels: an e-ink panel and a screenshot both read them back.
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                if (crop != null) decoder.crop = crop
            }
        }.getOrNull()
        if (decoded != null) return decoded
        // Some decoders refuse a format ImageDecoder does not know; the older path still reads it.
        if (crop != null) {
            @Suppress("DEPRECATION")
            val region = BitmapRegionDecoder.newInstance(source.path, false)
            if (region != null) {
                try {
                    region.decodeRegion(crop, null)?.let { return it }
                } finally {
                    region.recycle()
                }
            }
        }
        val whole = BitmapFactory.decodeFile(source.path) ?: error("not a picture")
        // The crop names the part to show; this path decodes the whole file, so cut it here.
        if (crop != null && crop.right <= whole.width && crop.bottom <= whole.height && !crop.isEmpty) {
            return Bitmap.createBitmap(whole, crop.left, crop.top, crop.width(), crop.height())
        }
        return whole
    }

    private fun raw(source: File, file: PageFile, encoding: PageEncoding.Raw): PageBitmap {
        val width = file.width.toInt()
        val height = file.height.toInt()
        val grey = encoding.format == PixelFormat.GRAY8
        val pixel = if (grey) 1 else 4
        val stride = encoding.stride.toInt().takeIf { it >= width * pixel } ?: (width * pixel)
        val bitmap = Bitmap.createBitmap(width, height, if (grey) Bitmap.Config.ALPHA_8 else Bitmap.Config.ARGB_8888)
        RandomAccessFile(source, "r").use { input ->
            val mapped = input.channel.map(FileChannel.MapMode.READ_ONLY, 0, input.length())
            check(mapped.capacity().toLong() >= stride.toLong() * (height - 1) + width * pixel) { "the page file is short" }
            val rowBytes = bitmap.rowBytes
            // RGBA in memory is what ARGB_8888 holds on Android; a host whose order differs swaps.
            val swap = !grey && !rgbaInMemory
            val packed = if (!swap && stride == rowBytes && mapped.capacity() >= rowBytes * height) {
                mapped.limit(rowBytes * height)
                mapped
            } else {
                // Rows padded differently from the bitmap's: copy them one by one.
                val out = ByteBuffer.allocateDirect(rowBytes * height)
                val row = ByteArray(width * pixel)
                for (y in 0 until height) {
                    mapped.position(y * stride)
                    mapped.get(row)
                    if (swap) {
                        for (i in row.indices step 4) {
                            val r = row[i]
                            row[i] = row[i + 2]
                            row[i + 2] = r
                        }
                    }
                    out.position(y * rowBytes)
                    out.put(row)
                }
                out.rewind()
                out
            }
            bitmap.copyPixelsFromBuffer(packed)
        }
        return PageBitmap(bitmap, grey)
    }

    /** Whether ARGB_8888 holds red first in memory, as it does on every Android device. */
    private val rgbaInMemory: Boolean by lazy {
        runCatching {
            val probe = Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888)
            probe.copyPixelsFromBuffer(ByteBuffer.wrap(byteArrayOf(-1, 0, 0, -1)))
            android.graphics.Color.red(probe.getPixel(0, 0)) == 255
        }.getOrDefault(true)
    }
}

/**
 * The decoded pages kept in memory, by file and crop. Sized to an eighth of what the app may use,
 * so a 2 GB e-reader keeps working; the engine's own cache holds the files.
 */
class PageImages(maxBytes: Int = defaultBytes()) {
    private val cache = object : LruCache<String, PageBitmap>(maxBytes.coerceAtLeast(1)) {
        override fun sizeOf(key: String, value: PageBitmap): Int = value.bitmap.allocationByteCount
    }

    fun cached(file: PageFile): PageBitmap? = cache.get(keyOf(file))

    /** The page's bitmap, decoded now if it is not kept. Call it off the main thread. */
    fun load(file: PageFile): PageDecodeResult {
        cache.get(keyOf(file))?.let { return PageDecodeResult.Decoded(it) }
        val result = PageDecoder.decode(file)
        if (result is PageDecodeResult.Decoded) cache.put(keyOf(file), result.page)
        return result
    }

    fun clear() = cache.evictAll()

    companion object {
        fun defaultBytes(): Int = (Runtime.getRuntime().maxMemory() / 8).coerceAtMost(Int.MAX_VALUE.toLong()).toInt()

        private fun keyOf(file: PageFile): String = file.crop?.let { "${file.path}#${it.x},${it.y},${it.width},${it.height}" } ?: file.path
    }
}
