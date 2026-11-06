// The detection bridge's half on the device: what Android says about itself and its screen, sent
// to the engine as DeviceFacts before the first screen. The engine decides from these whether the
// screen is e-ink and which panel it has; nothing here guesses.

package app.kaeri.eink

import android.content.Context
import android.os.Build
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.WindowManager
import android.webkit.WebView
import app.kaeri.engine.model.ScreenFacts
import kotlin.math.roundToInt

object ScreenFactsReader {
    /** This device's facts. [wideColourGamut] overrides the display's answer, for the Fixtures scenes. */
    fun read(context: Context, wideColourGamut: Boolean? = null): ScreenFacts {
        val metrics = context.resources.displayMetrics
        val windows = context.getSystemService(WindowManager::class.java)
        val bounds = guarded { windows?.maximumWindowMetrics?.bounds }
        @Suppress("DEPRECATION")
        val display = guarded { context.display } ?: guarded { windows?.defaultDisplay }
        // The physical density where the display gives one; the bucket otherwise.
        val dpi = metrics.xdpi.takeIf { it > 0f }?.roundToInt() ?: metrics.densityDpi
        return ScreenFacts(
            manufacturer = Build.MANUFACTURER.orEmpty(),
            brand = Build.BRAND.orEmpty(),
            model = Build.MODEL.orEmpty(),
            device = Build.DEVICE.orEmpty(),
            product = Build.PRODUCT.orEmpty(),
            hardware = Build.HARDWARE.orEmpty(),
            sdk = Build.VERSION.SDK_INT.toUInt(),
            widthPx = (bounds?.width() ?: metrics.widthPixels).coerceAtLeast(0).toUInt(),
            heightPx = (bounds?.height() ?: metrics.heightPixels).coerceAtLeast(0).toUInt(),
            dpi = dpi.coerceAtLeast(0).toUInt(),
            refreshHz = display?.refreshRate ?: 0f,
            einkSupport = systemProperty("ro.eink.support"),
            onyxName = systemProperty("ro.onyx.devicename"),
            wideColourGamut = wideColourGamut ?: (guarded { display?.isWideColorGamut } ?: false),
            hooks = RefreshControllers.hooks(context),
            pageKeys = guarded { KeyCharacterMap.deviceHasKeys(intArrayOf(KeyEvent.KEYCODE_PAGE_UP, KeyEvent.KEYCODE_PAGE_DOWN)).any { it } } ?: false,
            webview = webViewMajor(),
        )
    }

    /** A system property, read by reflection, or null when it is unset or cannot be read. */
    fun systemProperty(name: String): String? = guarded {
        val properties = Class.forName("android.os.SystemProperties")
        properties.getMethod("get", String::class.java).invoke(null, name) as? String
    }?.takeIf { it.isNotBlank() }

    /** The system WebView's major version, such as 120. */
    fun webViewMajor(): UInt? = guarded { WebView.getCurrentWebViewPackage()?.versionName }
        ?.substringBefore('.')
        ?.toUIntOrNull()
}
