// Why finding skip times from the audio is paused, in Android's words: Settings and the Skip times
// panel say it, the player says nothing.

package app.kaeri.store

import android.content.Context
import app.kaeri.engine.model.SkipFindPause
import app.kaeri.i18n.Strings

/** "Paused while battery saver is on." or "Paused until 2 GB of storage is free.", `null` when it isn't paused. */
fun skipFindPausedLine(context: Context, pause: SkipFindPause?): String? = when (pause) {
    null -> null
    SkipFindPause.LOW_POWER -> Strings.playerSkipPausedBatterySaver(context)
    SkipFindPause.STORAGE -> Strings.playerSkipPausedStorage(context)
}
