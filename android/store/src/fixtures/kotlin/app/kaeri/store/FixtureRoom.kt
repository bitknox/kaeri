// The fixtures' room, for the room board's shots and recordings (the `room` extra): the testing
// engine's fake room, with Mika and Ren played by the engine, staged to one of its scenes. No
// relay is public: the fake room never leaves the device.

package app.kaeri.store

import android.util.Log
import app.kaeri.engine.ffi.FfiException
import app.kaeri.engine.ffi.FfiFakeRoom
import app.kaeri.engine.ffi.FfiRoomScene
import app.kaeri.engine.model.Command
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

object FixtureRoom {
    /** The fake room's episode: Stardust Wayfarer, episode 9. */
    private const val ID = 900001u
    private const val EPISODE = "9"

    /** The fake room, held for the run: it stops when let go. */
    private var held: FfiFakeRoom? = null

    private fun log(text: String) = Log.i("kaeri", "KAERI_ROOM $text")

    /** The scene [name] names, as `before-play`, `beforeplay` or `BEFORE_PLAY`. */
    fun named(name: String?): FfiRoomScene? {
        val wanted = name?.filter { it.isLetterOrDigit() }?.lowercase() ?: return null
        return FfiRoomScene.entries.firstOrNull { it.name.filter { c -> c.isLetterOrDigit() }.lowercase() == wanted }
    }

    /** An invite's Join screen, before joining. */
    private fun invited(scene: FfiRoomScene) = scene == FfiRoomScene.INVITED || scene == FfiRoomScene.INVITED_FULL

    /** Scenes with the player open on the room's episode: all but the invites and a room nobody reaches. */
    private fun playing(scene: FfiRoomScene) = !invited(scene) && scene != FfiRoomScene.UNREACHABLE

    /**
     * Bring the room to [scene]; true once it is there. [presented] says whether the player is up
     * with a picture. The engine's stage blocks for a few seconds, so it runs off the main thread.
     */
    suspend fun stage(scene: FfiRoomScene, store: AppStore, presented: () -> Boolean, after: Long? = null): Boolean {
        val engine = withTimeoutOrNull(60_000) {
            while (store.engine == null) delay(50)
            store.engine
        } ?: return false
        // For the morph's recording: the player alone first, then the room joins [after] ms on.
        if (after != null) {
            store.send(Command.Play(ID, EPISODE))
            if (!waited(60, presented)) return false.also { log("roomat: no picture") }
            log("playing, the room in $after ms")
            delay(after)
        }
        val staged = withContext(Dispatchers.IO) {
            try {
                val fake = engine.startFakeRoom().also { held = it }
                fake.stage(scene) && (!invited(scene) || store.followRoomLink(fake.invite("Mika")))
            } catch (e: FfiException) {
                Log.w("kaeri", "the fake room did not start: ${e.message}")
                false
            }
        }
        if (!staged) return false.also { log("scene ${scene.name} did not stage") }
        if (invited(scene)) {
            if (!waited(10) { store.room.invited.value?.glimpse != null }) log("scene ${scene.name}: no glimpse")
        }
        if (playing(scene)) {
            // The room's episode in the player, unless the room already opened it.
            if (!waited(5) { presented() }) {
                val episode = store.room.view.value?.episode
                if (episode != null) store.send(Command.Play(episode.id, episode.episode))
            }
            if (!waited(60, presented)) return false.also { log("scene ${scene.name}: no picture") }
        }
        delay(1_000)
        log("ready ${scene.name.lowercase()}")
        return true
    }

    private suspend fun waited(seconds: Int, condition: () -> Boolean): Boolean =
        withTimeoutOrNull(seconds * 1_000L) {
            while (!condition()) delay(50)
            true
        } ?: false
}
