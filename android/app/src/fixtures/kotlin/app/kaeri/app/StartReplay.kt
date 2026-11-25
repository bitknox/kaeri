// The fixtures flavour's replay of a made-up start on the real player screen: what the `start`
// extra asks for. Not in the app flavour, so no release build carries it.

package app.kaeri.app

import android.app.Activity
import android.content.Context
import android.content.res.Configuration
import android.content.Intent
import android.util.Log
import app.kaeri.engine.ffi.startingFixture
import app.kaeri.engine.model.FixtureEvent
import app.kaeri.engine.model.StartingFixture
import app.kaeri.player.PlayerModel
import app.kaeri.player.PlayerTime
import app.kaeri.player.StartDetailsMemory
import app.kaeri.store.AppStore
import kotlinx.coroutines.CompletableDeferred
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
 *
 * `--es stillat <seconds>` makes it a still: the replay runs on the engine's still clock (turned
 * on at nought at launch, see Flavour.launch), holding it at each event's moment before applying
 * it and then at <seconds>, at once in real time, so the screen shows that moment however slow the
 * device. Events after the moment are left out. The log says `KAERI_STILL_HELD` once it is there.
 * Without it the replay sleeps on the wall clock, as recordings need.
 */
internal object StartReplay {
    private var begun = false

    /** Done once a still replay holds its moment; complete at once when no still is asked for. */
    private val held = CompletableDeferred<Boolean>()

    /** The moment `stillat` names, in milliseconds, or null without one (or one that is not seconds). */
    fun stillMillis(intent: Intent): Long? =
        intent.getStringExtra("stillat")?.trim()?.toDoubleOrNull()?.takeIf { it.isFinite() && it >= 0 }?.let { (it * 1000).toLong() }

    /** True once a still replay asked for holds its moment, or at once when none was. */
    suspend fun held(): Boolean = held.await()

    fun from(context: Context, store: AppStore, player: PlayerModel, intent: Intent) {
        val name = intent.getStringExtra("start")
        if (name == null) {
            held.complete(true)
            return
        }
        if (begun) return
        begun = true
        val state = StartingFixture.entries.firstOrNull { it.name.lowercase().replace('_', '-') == name }
        if (state == null) {
            Log.w(KaeriApp.TAG, "start takes one of ${StartingFixture.entries.joinToString { it.name.lowercase().replace('_', '-') }}, not $name")
            held.complete(false)
            return
        }
        val still = stillMillis(intent)
        if (intent.getStringExtra("details") == "1") StartDetailsMemory.set(context, true)
        val wait = intent.getStringExtra("startdelay")?.toLongOrNull() ?: 0L
        MainScope().launch {
            // The first fixture's art is there before the screen is.
            withTimeoutOrNull(30_000) { store.titles.entries.first { 900001u in it } }
            val events = startingFixture(state)
            if (still != null) {
                held.complete(replayStill(context, player, events, still))
                return@launch
            }
            held.complete(true)
            delay(wait)
            val began = System.currentTimeMillis()
            for (fixture in events) {
                val at = began + fixture.atMs.toLong()
                delay((at - System.currentTimeMillis()).coerceAtLeast(0))
                player.apply(fixture.event)
            }
        }
    }

    /** The events due by [moment], each on the still clock at its own, then the clock held at [moment]. */
    private suspend fun replayStill(context: Context, player: PlayerModel, events: List<FixtureEvent>, moment: Long): Boolean {
        val began = PlayerTime.still() ?: 0L
        var there = true
        for (fixture in events) {
            val at = fixture.atMs.toLong()
            if (at > moment) break
            there = PlayerTime.reach(began + at) && there
            player.apply(fixture.event)
        }
        there = PlayerTime.reach(began + moment) && there
        // The player's screen is up, and a phone has turned to landscape for it, which a slow
        // emulator takes longer over than the shots' settle: then the moment counts as held.
        there = (withTimeoutOrNull(20_000) { while (!player.isPresented || !turned(context)) delay(50); true } ?: false) && there
        // The starting screen reads the player's clock, which its ticker moves every 250 ms (not
        // poke(), which is a tap and would bring the controls up).
        delay(300)
        Log.i(KaeriApp.TAG, if (there) "KAERI_STILL_HELD at $moment ms" else "the still clock did not reach $moment ms")
        return there
    }

    /**
     * Whether the window is as the player's screen leaves it: a phone's turns to landscape once the
     * player's page has come in (PlayerScreen); a larger screen stays as it is.
     */
    private fun turned(context: Context): Boolean {
        val configuration = (context as? Activity)?.resources?.configuration ?: return true
        return configuration.smallestScreenWidthDp >= 600 || configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
    }
}
