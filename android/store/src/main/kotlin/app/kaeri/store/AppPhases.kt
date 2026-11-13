// What the engine is told of the app's place: E7's AppPhase.

package app.kaeri.store

import app.kaeri.engine.model.AppState
import app.kaeri.engine.model.Command

/**
 * Tells the engine where the app is: the foreground, or the background with whether an episode
 * plays there. In the background it says so again each time that changes, so the engine stops
 * keeping a torrent going for an episode that has paused or ended, and keeps one going again for
 * an episode played on from the notification. While the download job or service runs, the
 * background is one where downloads go on, episodes and chapters both.
 */
class AppPhases(private val send: (Command) -> Unit) {
    private var background = false
    private var playing = false
    private var downloading = false

    private val away get() = if (downloading) AppState.BackgroundDownloads else AppState.Background(playing)

    /** The app came to the foreground. */
    fun foreground() {
        background = false
        send(Command.AppPhase(AppState.Foreground))
    }

    /** The app went to the background, with an episode [playing] there or not. */
    fun background(playing: Boolean) {
        background = true
        this.playing = playing
        send(Command.AppPhase(away))
    }

    /** Whether an episode plays, each time that changes: said again while in the background. */
    fun playing(now: Boolean) {
        if (!background || now == playing) return
        playing = now
        if (!downloading) send(Command.AppPhase(away))
    }

    /** Whether the download job or service runs, each time that changes: said again while in the background. */
    fun downloading(now: Boolean) {
        if (now == downloading) return
        downloading = now
        if (background) send(Command.AppPhase(away))
    }
}
