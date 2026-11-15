package app.kaeri.store

import org.junit.Assert.assertEquals
import org.junit.Test
import app.kaeri.engine.model.AppState
import app.kaeri.engine.model.Command

class AppPhasesTest {
    private val said = mutableListOf<Command>()
    private val phases = AppPhases { said += it }

    private fun background(playing: Boolean) = Command.AppPhase(AppState.Background(playing))

    @Test
    fun playback_that_stops_in_the_background_is_said_so_the_engine_lets_its_torrent_go() {
        phases.background(playing = true)
        // Paused from the notification, or the episode ended.
        phases.playing(false)
        assertEquals(listOf(background(true), background(false)), said)
    }

    @Test
    fun playback_that_starts_again_in_the_background_is_said_too() {
        phases.background(playing = true)
        phases.playing(false)
        phases.playing(true)
        assertEquals(listOf(background(true), background(false), background(true)), said)
    }

    @Test
    fun nothing_changed_and_the_foreground_say_nothing_more() {
        phases.background(playing = false)
        phases.playing(false)
        phases.foreground()
        // In the foreground the engine is not told of playback; going back is said as itself.
        phases.playing(true)
        phases.playing(false)
        phases.background(playing = false)
        assertEquals(listOf(background(false), Command.AppPhase(AppState.Foreground), background(false)), said)
    }

    private val downloads = Command.AppPhase(AppState.BackgroundDownloads)

    @Test
    fun the_download_job_running_in_the_background_lets_the_downloads_go_on() {
        // Chapters queued in front start the job; then the app leaves.
        phases.downloading(true)
        phases.background(playing = false)
        assertEquals(listOf(downloads), said)
        // Playback changing meanwhile changes nothing: downloads and what plays go on.
        phases.playing(true)
        assertEquals(listOf(downloads), said)
        // The job ends: what waits in the background waits again, as playback has it now.
        phases.downloading(false)
        assertEquals(listOf(downloads, background(true)), said)
    }

    @Test
    fun the_job_starting_or_ending_while_away_is_said_and_in_front_is_not() {
        phases.background(playing = false)
        // Started again by the system after the app left.
        phases.downloading(true)
        phases.downloading(true)
        phases.foreground()
        phases.downloading(false)
        assertEquals(listOf(background(false), downloads, Command.AppPhase(AppState.Foreground)), said)
    }
}
