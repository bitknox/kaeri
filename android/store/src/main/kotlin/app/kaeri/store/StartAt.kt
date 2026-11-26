package app.kaeri.store

import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.PlayerCommand
import app.kaeri.engine.model.PlayerReport

/**
 * A play that starts from a position the engine's history doesn't have, such as a TV's playhead
 * on Play here: once the engine staged this play and its file is open, the player seeks there.
 */
internal class StartAt(val id: AnilistId, val episode: String, val position: Double) {
    /** The engine has staged this play. */
    var staged = false
        private set

    /** The engine staged a play: this one, or another, which this gives way to. Whether it is still to start. */
    fun staged(id: AnilistId, episode: String): Boolean {
        if (id != this.id || episode != this.episode) return false
        staged = true
        return true
    }

    /** What to send after [command] has gone: the move, once the staged play's file is open. */
    fun after(command: Command): Command? {
        if (!staged || command !is Command.PlayerReport || command.report != PlayerReport.FileLoaded) return null
        return Command.Player(PlayerCommand.SeekTo(position))
    }
}
