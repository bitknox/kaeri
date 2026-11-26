// The player as the app's frame shows it: up while the model is, over the navigation bar.

package app.kaeri.player.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ObiSilk
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.RoomCommand
import app.kaeri.player.PlayerModel
import app.kaeri.player.ui.room.RoomBar
import app.kaeri.player.ui.room.RoomInvitedScreen
import app.kaeri.player.ui.room.RoomLeftAlert
import app.kaeri.ui.ArtworkImage
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.PlayerSlot
import app.kaeri.ui.pixels

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

    @Composable
    override fun Bar() {
        if (model.isPresented) return
        val room = LocalAppStore.current.room
        val view = room.view.collectAsStateWithLifecycle().value ?: return
        val art = view.roomEpisode?.bannerUrl ?: view.roomEpisode?.coverUrl
        RoomBar(
            room,
            positionS = view.position ?: 0.0,
            still = { m -> ArtworkImage(art, ObiSilk.Raised, pixels(64.dp), m) },
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
            onOpen = { room.send(RoomCommand.OpenPlayer) },
        )
    }

    @Composable
    override fun Over() {
        val room = LocalAppStore.current.room
        val view by room.view.collectAsStateWithLifecycle()
        val invited by room.invited.collectAsStateWithLifecycle()
        val left by room.left.collectAsStateWithLifecycle()
        // An invite opened with the player closed: the room's Join screen, upright, its video the still.
        if (invited != null && !model.isPresented) {
            val inv = invited!!
            // A TV driven from this phone and in no room: the TV can take the invite, playing from its own sources.
            val tv = LocalAppStore.current.tv
            val card by tv.card.collectAsStateWithLifecycle()
            val drivenTv = card?.card?.takeIf { it.room == null && it.tvName.isNotEmpty() }
            val onTv = drivenTv?.let { c ->
                c.tvName to {
                    tv.send(app.kaeri.engine.ffi.FfiTvCommand.RoomJoin(inv.invite))
                    room.notNow()
                }
            }
            BackHandler { room.notNow() }
            RoomInvitedScreen(room, inv, onClose = { room.notNow() }, joinOnTv = onTv)
        }
        if (view == null) left?.let { why -> RoomLeftAlert(room, why) }
    }
}
