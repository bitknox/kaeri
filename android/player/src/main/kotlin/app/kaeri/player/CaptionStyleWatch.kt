// The system's caption style, followed for the player: a change to it gives the mpv playing the
// new look. The viewer's own style comes from the engine's settings.

package app.kaeri.player

import android.content.Context
import android.view.accessibility.CaptioningManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.launch
import app.kaeri.engine.model.SubtitleStyle

/** Keeps [player] on the engine's subtitle style [styles], and on the style set in Accessibility, Caption preferences, for as long as it is held. */
class CaptionStyleWatch(context: Context, private val player: PlayerModel, scope: CoroutineScope, styles: Flow<SubtitleStyle?>) {
    private val system = object : CaptioningManager.CaptioningChangeListener() {
        override fun onEnabledChanged(enabled: Boolean) = player.subtitleStyleChanged()
        override fun onUserStyleChanged(userStyle: CaptioningManager.CaptionStyle) = player.subtitleStyleChanged()
        override fun onFontScaleChanged(fontScale: Float) = player.subtitleStyleChanged()
    }

    init {
        scope.launch { styles.collect { player.subtitleStyle = it } }
        context.getSystemService(CaptioningManager::class.java)?.addCaptioningChangeListener(system)
    }
}
