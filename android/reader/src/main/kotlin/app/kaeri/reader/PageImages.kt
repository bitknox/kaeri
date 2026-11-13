// Page files to bitmaps. The engine writes each page to its cache whole, as the book had it or as
// raw pixels ready to map (a grey e-ink page is Gray8), and names the crop to show. A picture too
// big for a canvas to draw, such as a very tall strip image, is decoded smaller. Decoding runs
// off the main thread and the bitmaps are kept in a cache sized from the app's memory. The pages
// the reader has on screen, beside it and held over a turn are kept whatever that size: an e-ink
// panel's raw pages are big enough that the cache may hold only one.

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
import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.sqrt

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

/**
 * Decodes page files, the engine's crop applied, and raw pixels mapped as they are. No bitmap comes
 * out bigger than [budget] bytes: a hardware canvas refuses to draw one past [CANVAS_BYTES] and
 * throws, so a picture that would pass it is decoded smaller, its shape kept.
 */
object PageDecoder {
    /** The most a hardware canvas draws in one bitmap; past it, drawing throws. */
    const val CANVAS_BYTES: Long = 100L * 1024 * 1024

    fun decode(file: PageFile, budget: Long = CANVAS_BYTES): PageDecodeResult = try {
        val source = File(file.path)
        if (!source.isFile) {
            PageDecodeResult.Missing
        } else {
            // Settled before the first picture: the probe's one-pixel copy makes the first native
            // buffer and file calls under the lazy's lock. The JVM test runtime sets those up for
            // one first caller at a time, and pages beside the shown one decode alongside it.
            val rgba = rgbaInMemory
            val page = when (val encoding = file.encoding) {
                is PageEncoding.Original -> PageBitmap(original(source, file, budget), grey = false)
                is PageEncoding.Raw -> raw(source, file, encoding, rgba)
            }
            PageDecodeResult.Decoded(PageBitmap(within(page.bitmap, budget), page.grey))
        }
    } catch (_: FileNotFoundException) {
        PageDecodeResult.Missing
    } catch (e: Exception) {
        PageDecodeResult.Unreadable(e.message ?: e.javaClass.simpleName)
    } catch (e: OutOfMemoryError) {
        PageDecodeResult.Unreadable(e.message ?: "out of memory")
    }

    private fun original(source: File, file: PageFile, budget: Long): Bitmap {
        val crop = file.crop?.let { Rect(it.x.toInt(), it.y.toInt(), (it.x + it.width).toInt(), (it.y + it.height).toInt()) }
        val decoded = runCatching {
            ImageDecoder.decodeBitmap(ImageDecoder.createSource(source)) { decoder, info, _ ->
                // Software pixels: an e-ink panel and a screenshot both read them back.
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                val full = info.size
                val scale = scaleWithin(crop?.width() ?: full.width, crop?.height() ?: full.height, ARGB_BYTES, budget)
                if (scale < 1.0) {
                    // Too big to draw: the decoder scales as it reads, and the crop is in scaled pixels.
                    val width = scaled(full.width, scale)
                    val height = scaled(full.height, scale)
                    decoder.setTargetSize(width, height)
                    if (crop != null) {
                        val left = scaled(crop.left, scale, least = 0).coerceAtMost(width - 1)
                        val top = scaled(crop.top, scale, least = 0).coerceAtMost(height - 1)
                        decoder.crop = Rect(
                            left, top,
                            (left + scaled(crop.width(), scale)).coerceAtMost(width),
                            (top + scaled(crop.height(), scale)).coerceAtMost(height),
                        )
                    }
                } else if (crop != null) {
                    decoder.crop = crop
                }
            }
        }.getOrNull()
        if (decoded != null) return decoded
        // Some decoders refuse a format ImageDecoder does not know; the older path still reads it.
        if (crop != null) {
            @Suppress("DEPRECATION")
            val region = BitmapRegionDecoder.newInstance(source.path, false)
            if (region != null) {
                try {
                    val options = BitmapFactory.Options().apply { inSampleSize = sampleWithin(crop.width(), crop.height(), budget) }
                    region.decodeRegion(crop, options)?.let { return it }
                } finally {
                    region.recycle()
                }
            }
        }
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(source.path, bounds)
        val usable = crop?.takeIf { !it.isEmpty && it.right <= bounds.outWidth && it.bottom <= bounds.outHeight }
        val sample = sampleWithin(usable?.width() ?: bounds.outWidth, usable?.height() ?: bounds.outHeight, budget)
        val whole = BitmapFactory.decodeFile(source.path, BitmapFactory.Options().apply { inSampleSize = sample })
            ?: error("not a picture")
        // The crop names the part to show; this path decodes the whole file, so cut it here.
        if (usable != null) {
            val cut = Rect(usable.left / sample, usable.top / sample, usable.right / sample, usable.bottom / sample)
            cut.intersect(0, 0, whole.width, whole.height)
            if (!cut.isEmpty) return Bitmap.createBitmap(whole, cut.left, cut.top, cut.width(), cut.height())
        }
        return whole
    }

    /**
     * [bitmap] as it is when a canvas draws it, or scaled down to [budget] bytes when it would not,
     * whatever made it: raw pixels, and a decoder that gave more than it was asked for.
     */
    internal fun within(bitmap: Bitmap, budget: Long): Bitmap {
        if (bitmap.byteCount.toLong() <= budget) return bitmap
        val perPixel = (bitmap.byteCount.toLong() / (bitmap.width.toLong() * bitmap.height)).toInt().coerceAtLeast(1)
        val scale = scaleWithin(bitmap.width, bitmap.height, perPixel, budget)
        val smaller = Bitmap.createScaledBitmap(bitmap, scaled(bitmap.width, scale), scaled(bitmap.height, scale), true)
        if (smaller !== bitmap) bitmap.recycle()
        return smaller
    }

    /** The factor that brings a [width] by [height] picture to [budget] bytes, 1 when it fits already. */
    internal fun scaleWithin(width: Int, height: Int, bytesPerPixel: Int, budget: Long): Double {
        val bytes = width.toLong() * height * bytesPerPixel
        return if (bytes <= budget || bytes <= 0) 1.0 else sqrt(budget.toDouble() / bytes)
    }

    /** The smallest power-of-two sample that brings a [width] by [height] picture to [budget] bytes. */
    internal fun sampleWithin(width: Int, height: Int, budget: Long): Int {
        var sample = 1
        while (sample < MAX_SAMPLE && ceil(width / sample.toDouble()) * ceil(height / sample.toDouble()) * ARGB_BYTES > budget) sample *= 2
        return sample
    }

    private fun scaled(length: Int, scale: Double, least: Int = 1): Int = floor(length * scale).toInt().coerceAtLeast(least)

    private const val ARGB_BYTES = 4
    private const val MAX_SAMPLE = 1 shl 16

    private fun raw(source: File, file: PageFile, encoding: PageEncoding.Raw, rgba: Boolean): PageBitmap {
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
            val swap = !grey && !rgba
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
 * so a 2 GB e-reader keeps working; the engine's own cache holds the files. The pages named by
 * [keep] stay besides, whatever that size, until they are no longer named.
 */
class PageImages(maxBytes: Int = defaultBytes()) {
    private val cache = object : LruCache<String, PageBitmap>(maxBytes.coerceAtLeast(1)) {
        override fun sizeOf(key: String, value: PageBitmap): Int = value.bitmap.allocationByteCount
    }
    private val kept = java.util.concurrent.ConcurrentHashMap<String, PageBitmap>()
    @Volatile private var keeping: Set<String> = emptySet()

    fun cached(file: PageFile): PageBitmap? = keyOf(file).let { kept[it] ?: cache.get(it) }

    /**
     * Keeps [files]' pictures, once decoded, whatever the cache's size: the page shown, the pages
     * beside it and a page held on screen over a turn. Pages kept before and not named now go back
     * to the cache alone.
     */
    fun keep(files: Collection<PageFile>) {
        val keys = files.mapTo(HashSet(), ::keyOf)
        if (keys == keeping) return
        keeping = keys
        kept.keys.retainAll(keys)
        for (key in keys) if (!kept.containsKey(key)) cache.get(key)?.let { kept[key] = it }
    }

    /** The page's bitmap, decoded now if it is not kept. Call it off the main thread. */
    fun load(file: PageFile): PageDecodeResult {
        val key = keyOf(file)
        cached(file)?.let { return PageDecodeResult.Decoded(it) }
        val result = PageDecoder.decode(file)
        if (result is PageDecodeResult.Decoded) {
            cache.put(key, result.page)
            if (key in keeping) kept[key] = result.page
        }
        return result
    }

    fun clear() {
        kept.clear()
        cache.evictAll()
    }

    companion object {
        fun defaultBytes(): Int = (Runtime.getRuntime().maxMemory() / 8).coerceAtMost(Int.MAX_VALUE.toLong()).toInt()

        private fun keyOf(file: PageFile): String = file.crop?.let { "${file.path}#${it.x},${it.y},${it.width},${it.height}" } ?: file.path
    }
}
