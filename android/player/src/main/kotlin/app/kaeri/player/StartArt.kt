// The ground the starting screen puts over its art, and the Details switch it remembers.

package app.kaeri.player

import android.content.Context
import android.graphics.Bitmap
import androidx.core.graphics.get
import kotlin.math.pow

/**
 * How bright a piece of art is where the column sits, so the ground can be deeper over bright art:
 * the mean relative luminance (linear sRGB) of the middle 60 % of the width and half of the
 * height, from a copy about 64 px wide. Measured once per piece of art.
 */
object ArtLuminance {
    /** Above this the ground is [DEEP]: at 55 % haze text stops holding 4.5:1. */
    const val BRIGHT = 0.15

    /** The ground over normal art, and over bright art. */
    const val NORMAL_GROUND = 0.55f
    const val DEEP_GROUND = 0.80f

    fun of(art: Bitmap): Double {
        val width = 64
        val height = (art.height * width / art.width.coerceAtLeast(1)).coerceAtLeast(1)
        val small = if (art.width <= width) art else Bitmap.createScaledBitmap(art, width, height, true)
        val x0 = (small.width * 0.2).toInt()
        val x1 = (small.width * 0.8).toInt().coerceAtLeast(x0 + 1)
        val y0 = (small.height * 0.25).toInt()
        val y1 = (small.height * 0.75).toInt().coerceAtLeast(y0 + 1)
        var sum = 0.0
        var n = 0
        for (y in y0 until y1.coerceAtMost(small.height)) {
            for (x in x0 until x1.coerceAtMost(small.width)) {
                val c = small[x, y]
                sum += 0.2126 * linear(c shr 16 and 0xFF) + 0.7152 * linear(c shr 8 and 0xFF) + 0.0722 * linear(c and 0xFF)
                n++
            }
        }
        if (small !== art) small.recycle()
        return if (n == 0) 0.0 else sum / n
    }

    /** The ground's opacity over [art]. */
    fun ground(art: Bitmap): Float = if (of(art) > BRIGHT) DEEP_GROUND else NORMAL_GROUND

    private fun linear(channel: Int): Double {
        val v = channel / 255.0
        return if (v <= 0.04045) v / 12.92 else ((v + 0.055) / 1.055).pow(2.4)
    }
}

/** Whether Details was left open, kept on this device. */
object StartDetailsMemory {
    private const val FILE = "player_starting"
    private const val KEY = "details_open"

    fun open(context: Context): Boolean =
        runCatching { context.getSharedPreferences(FILE, Context.MODE_PRIVATE).getBoolean(KEY, false) }.getOrDefault(false)

    fun set(context: Context, open: Boolean) {
        runCatching { context.getSharedPreferences(FILE, Context.MODE_PRIVATE).edit().putBoolean(KEY, open).apply() }
    }
}
