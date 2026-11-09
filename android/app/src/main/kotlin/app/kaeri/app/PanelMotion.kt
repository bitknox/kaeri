package app.kaeri.app

import android.content.ContentResolver
import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.compose.ui.MotionDurationScale
import androidx.compose.ui.platform.compositionContext
import androidx.compose.ui.platform.createLifecycleAwareWindowRecomposer
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner

/**
 * How long the window's animations take: the system's animator scale, as Compose's own reads it,
 * and none at all while [still] is set, on an e-ink panel. Every animation in the window then
 * lands at its end at once, so a crossfade or a spring is one redraw rather than a smear of
 * partial refreshes.
 */
class PanelMotion(private val resolver: ContentResolver) : MotionDurationScale {
    @Volatile
    var still: Boolean = false

    @Volatile
    private var system: Float = read()

    private val observer = object : ContentObserver(Handler(Looper.getMainLooper())) {
        override fun onChange(selfChange: Boolean) {
            system = read()
        }
    }

    override val scaleFactor: Float get() = if (still) 0f else system

    private fun read(): Float =
        runCatching { Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) }.getOrDefault(1f)

    /**
     * Gives [activity]'s window a recomposer that runs with this scale, before its content is set,
     * and follows the system's scale while the activity lives.
     */
    fun install(activity: ComponentActivity) {
        val decor = activity.window.decorView
        decor.compositionContext = decor.createLifecycleAwareWindowRecomposer(this, activity.lifecycle)
        resolver.registerContentObserver(Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, observer)
        activity.lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onDestroy(owner: LifecycleOwner) {
                resolver.unregisterContentObserver(observer)
            }
        })
    }
}
