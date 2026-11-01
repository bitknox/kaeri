// The player as the app's frame shows it: up while the model is, over the navigation bar.

package app.kaeri.player.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import app.kaeri.engine.core.AnilistId
import app.kaeri.player.PlayerModel
import app.kaeri.ui.PlayerSlot

/** [model]'s screen in the frame's player slot: the frame navigates to it while it is up. */
@Stable
class PlayerFrame(
    private val model: PlayerModel,
    /** Play on TV from the player, while the app has it: `null` where it has not. */
    private val playOnTv: () -> (() -> Unit)? = { null },
) : PlayerSlot {
    override val presented: Boolean get() = model.isPresented

    override val title: AnilistId? get() = model.titleId

    @Composable
    override fun Content() {
        val navigator = app.kaeri.ui.LocalNavigator.current
        PlayerScreen(model, playOnTv = playOnTv(), openSubtitleSettings = { navigator.openSettings(app.kaeri.ui.SettingsPane.Playback) })
    }
}
