// The system's caption style as the player reads it for the engine, which merges and draws it. The
// style is Android's caption settings in Robolectric's settings; nothing reads a device's own.

package app.kaeri.player

import android.provider.Settings
import android.view.accessibility.CaptioningManager
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import app.kaeri.engine.model.CaptionEdge
import app.kaeri.engine.model.SystemCaptionStyle

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class SystemCaptionsTest {
    private val context = RuntimeEnvironment.getApplication()

    private fun put(name: String, value: Int) = Settings.Secure.putInt(context.contentResolver, "accessibility_captioning_$name", value)

    @Test
    fun androidsColoursBecomeTheEnginesRgba() {
        assertEquals(0xFFFF00FFu, SystemCaptions.rgba(0xFFFFFF00.toInt()))
        assertEquals(0x00000080u, SystemCaptions.rgba(0x80000000.toInt()))
    }

    @Test
    fun withCaptionsOffThereIsNoSystemStyle() {
        put("enabled", 0)
        assertNull(SystemCaptions.read(context))
    }

    @Test
    fun withCaptionsOnTheViewersStyleIsRead() {
        put("enabled", 1)
        put("preset", -1)
        put("foreground_color", 0xFFFFFF00.toInt())
        put("background_color", 0)
        put("window_color", 0x80000000.toInt())
        put("edge_type", CaptioningManager.CaptionStyle.EDGE_TYPE_DROP_SHADOW)
        put("edge_color", 0xFF000000.toInt())
        Settings.Secure.putString(context.contentResolver, "accessibility_captioning_typeface", "serif")
        Settings.Secure.putFloat(context.contentResolver, "accessibility_captioning_font_scale", 1.5f)
        assertEquals(
            SystemCaptionStyle(
                fontFamily = "serif", relativeSize = 1.5, textRgba = 0xFFFF00FFu, backgroundRgba = 0u, windowRgba = 0x00000080u,
                edge = CaptionEdge.DROP_SHADOW,
            ),
            SystemCaptions.read(context),
        )
    }
}
