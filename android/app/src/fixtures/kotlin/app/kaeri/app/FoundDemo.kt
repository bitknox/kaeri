// The fixtures flavour's found-skip-times demo, for recording the board's motion prototypes
// (`--es found flow` or `--es found settings`): the engine's part is played by a script that says
// what it would, on the clip the playing scene streams, since the test engine finds nothing in it.
// The player and its screens are the app's own.

package app.kaeri.app

import android.util.Log
import app.kaeri.engine.ffi.foundSkipScene
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.FoundSkipFixture
import app.kaeri.engine.model.PlayerUi
import app.kaeri.engine.model.SkipFindPause
import app.kaeri.engine.model.SkipKind
import app.kaeri.player.PlayerHost
import app.kaeri.store.AppStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

internal object FoundDemo {
    /** Motion A, B and C in one pass: skip, land, the card, its answer, then Adjust's review. */
    fun run(host: PlayerHost, store: AppStore, scope: CoroutineScope, which: String) {
        scope.launch {
            val model = host.player
            if (which == "settings") {
                delay(7_000)
                val view = store.settings.view.value ?: return@launch
                Log.w(KaeriApp.TAG, "KAERI_FOUND settings paused")
                store.apply(listOf(Event.Settings(view.copy(skipFindPaused = SkipFindPause.LOW_POWER))))
                delay(5_000)
                store.apply(listOf(Event.Settings(view.copy(skipFindPaused = null))))
                return@launch
            }
            withTimeoutOrNull(60_000) { while (!model.hasPicture) delay(200) }
            val id = model.playback ?: 1uL
            val skip = foundSkipScene(FoundSkipFixture.SKIP)
            val prompt = foundSkipScene(FoundSkipFixture.PROMPT)
            val review = foundSkipScene(FoundSkipFixture.REVIEW)
            val nudged = foundSkipScene(FoundSkipFixture.NUDGED)
            fun mark(tag: String) = Log.w(KaeriApp.TAG, "KAERI_FOUND $tag ${System.currentTimeMillis()}")
            model.apply(Event.PlayerTimeline(id, skip.marks))
            for (ui in skip.ui) model.apply(Event.PlayerUi(id, ui))
            model.hideControls()
            mark("skip")
            delay(2_500)
            model.apply(Event.PlayerUi(id, PlayerUi.SkipCleared))
            model.seekTo(skip.marks.first { it.kind == SkipKind.OPENING }.end)
            mark("landed")
            delay(3_000)
            model.apply(Event.PlayerSkipPrompt(id, prompt.prompt))
            mark("card")
            delay(5_000)
            model.apply(Event.PlayerSkipPrompt(id, foundSkipScene(FoundSkipFixture.SAVED).prompt))
            mark("saved")
            delay(5_000)
            model.apply(Event.PlayerSkipPrompt(id, null))
            delay(1_500)
            model.apply(Event.PlayerSkipPrompt(id, prompt.prompt))
            mark("card again")
            delay(3_000)
            model.apply(Event.PlayerSkipPrompt(id, null))
            model.apply(Event.PlayerSkipMark(id, review.mark))
            mark("review")
            delay(4_000)
            model.apply(Event.PlayerSkipMark(id, nudged.mark))
            mark("nudged")
            delay(3_000)
            model.apply(Event.PlayerSkipMark(id, null))
            mark("done")
        }
    }
}
