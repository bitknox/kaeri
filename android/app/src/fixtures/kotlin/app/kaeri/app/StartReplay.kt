// The fixtures flavour's replay of a made-up start on the real player screen: what the `start`
// extra asks for. Not in the app flavour, so no release build carries it.

package app.kaeri.app

import android.content.Context
import android.content.Intent
import android.util.Log
import app.kaeri.engine.ffi.startingFixture
import app.kaeri.engine.model.StartingFixture
import app.kaeri.player.PlayerModel
import app.kaeri.player.StartDetailsMemory
import app.kaeri.store.AppStore
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * `--es start <state>` says the engine's made-up start [state] (`slow`, `source-missed`,
 * `offline`, `failed-no-episode` and the other names of `StartingFixture`, lower case with
 * hyphens) to the player as the engine would, on its own clock. `--es details 1` has Details
 * open, and `--es startdelay <ms>` waits before the first event, for a recording that begins
 * first. Once per process.
 */
internal object StartReplay {
    private var begun = false

    fun from(context: Context, store: AppStore, player: PlayerModel, intent: Intent) {
        val name = intent.getStringExtra("start") ?: return
        if (begun) return
        begun = true
        val state = StartingFixture.entries.firstOrNull { it.name.lowercase().replace('_', '-') == name }
        if (state == null) {
            Log.w(KaeriApp.TAG, "start takes one of ${StartingFixture.entries.joinToString { it.name.lowercase().replace('_', '-') }}, not $name")
            return
        }
        if (intent.getStringExtra("details") == "1") StartDetailsMemory.set(context, true)
        val wait = intent.getStringExtra("startdelay")?.toLongOrNull() ?: 0L
        MainScope().launch {
            // The first fixture's art is there before the screen is.
            withTimeoutOrNull(30_000) { store.titles.entries.first { 900001u in it } }
            delay(wait)
            val events = startingFixture(state)
            val began = System.currentTimeMillis()
            for (fixture in events) {
                val at = began + fixture.atMs.toLong()
                delay((at - System.currentTimeMillis()).coerceAtLeast(0))
                player.apply(fixture.event)
            }
        }
    }
}
