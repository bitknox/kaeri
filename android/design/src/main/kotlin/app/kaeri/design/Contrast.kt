// The system's own request for more contrast: Android 14's contrast level, and the older "high
// contrast text" switch. The theme follows it live and changes nothing else.

package app.kaeri.design

import android.app.UiModeManager
import android.content.Context
import android.database.ContentObserver
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext

/** The contrast level from which the app takes its high contrast colours: medium (0.5) and up. */
private const val MEDIUM_CONTRAST = 0.5f

private const val HIGH_TEXT_CONTRAST = "high_text_contrast_enabled"

/** Whether the system asks for high contrast right now: its contrast level, or high contrast text. */
fun systemWantsHighContrast(context: Context): Boolean {
    if (Build.VERSION.SDK_INT >= 34) {
        val modes = context.getSystemService(UiModeManager::class.java)
        if (modes != null && modes.contrast >= MEDIUM_CONTRAST) return true
    }
    return runCatching { Settings.Secure.getInt(context.contentResolver, HIGH_TEXT_CONTRAST, 0) == 1 }.getOrDefault(false)
}

/** Whether the system asks for high contrast, read live. */
@Composable
fun rememberHighContrast(): Boolean {
    val context = LocalContext.current
    var on by remember { mutableStateOf(systemWantsHighContrast(context)) }
    DisposableEffect(context) {
        val refresh = { on = systemWantsHighContrast(context) }
        val observer = object : ContentObserver(Handler(Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) = refresh()
        }
        runCatching {
            context.contentResolver.registerContentObserver(Settings.Secure.getUriFor(HIGH_TEXT_CONTRAST), false, observer)
        }
        val listener = if (Build.VERSION.SDK_INT >= 34) {
            UiModeManager.ContrastChangeListener { refresh() }.also {
                context.getSystemService(UiModeManager::class.java)?.addContrastChangeListener(context.mainExecutor, it)
            }
        } else {
            null
        }
        refresh()
        onDispose {
            runCatching { context.contentResolver.unregisterContentObserver(observer) }
            if (Build.VERSION.SDK_INT >= 34 && listener != null) {
                context.getSystemService(UiModeManager::class.java)?.removeContrastChangeListener(listener)
            }
        }
    }
    return on
}
