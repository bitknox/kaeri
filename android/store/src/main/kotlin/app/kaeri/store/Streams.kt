// Reading a stream up to a limit, on every Android the apps run on: InputStream.readNBytes is
// Android 13's.

package app.kaeri.store

import java.io.ByteArrayOutputStream
import java.io.InputStream

/**
 * Up to [limit] bytes from this stream, fewer only where it ends first, as Java's
 * `readNBytes(limit)` reads them. A caller that reads `limit + 1` learns whether the stream was
 * longer than it takes.
 */
fun InputStream.readAtMost(limit: Int): ByteArray {
    require(limit >= 0) { "limit is negative" } // i18n-skip
    val out = ByteArrayOutputStream(minOf(limit, CHUNK))
    val buffer = ByteArray(minOf(limit, CHUNK).coerceAtLeast(1))
    var left = limit
    while (left > 0) {
        val read = read(buffer, 0, minOf(left, buffer.size))
        if (read < 0) break
        out.write(buffer, 0, read)
        left -= read
    }
    return out.toByteArray()
}

private const val CHUNK = 64 * 1024
