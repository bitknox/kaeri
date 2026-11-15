// The starting screen on a roll-on: which starts the model marks as one, and the 600 ms wait and
// 0.6 s hold the screen keeps for them.

package app.kaeri.player

import app.kaeri.engine.model.Ending
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.NextEpisode
import app.kaeri.engine.model.PlayerOp
import app.kaeri.engine.model.PlayerUi
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class RollOnStartTest {
    private fun staged(rig: Rig, episode: String) =
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", episode, null, 1.0))

    private fun upNext(rig: Rig, playback: ULong = 1u) =
        rig.ui(playback, PlayerUi.UpNext(NextEpisode("8", "A Town Without Night", null), true, 42.0, 10u, 3u))

    @Test
    fun aFirstStartShowsTheStartingScreenAtOnce() = runTest {
        val rig = Rig(this)
        staged(rig, "7")
        assertNull(rig.model.rollOnQuietUntil)
    }

    @Test
    fun theCountdownRunningOutMarksTheNextStartAsARollOn() = runTest {
        val rig = Rig(this)
        staged(rig, "7")
        rig.opened()
        upNext(rig)
        rig.playhead(42.0)
        rig.playhead(52.0)
        advanceTimeBy(100)
        val at = rig.model.elapsed()
        staged(rig, "8")
        assertEquals(at + RollOnStart.QUIET, rig.model.rollOnQuietUntil)
    }

    @Test
    fun playNowMarksTheNextStartAsARollOn() = runTest {
        val rig = Rig(this)
        staged(rig, "7")
        rig.opened()
        upNext(rig)
        rig.playhead(42.0)
        rig.model.playNext()
        staged(rig, "8")
        assertEquals(rig.model.elapsed() + RollOnStart.QUIET, rig.model.rollOnQuietUntil)
    }

    @Test
    fun theEngineRollingOnByItselfIsARollOnToo() = runTest {
        val rig = Rig(this)
        staged(rig, "7")
        rig.opened()
        rig.model.apply(Event.PlaybackEnded(true, Ending.Complete("8"), true))
        staged(rig, "8")
        assertEquals(rig.model.elapsed() + RollOnStart.QUIET, rig.model.rollOnQuietUntil)
    }

    @Test
    fun anEpisodeChosenByHandIsNotARollOn() = runTest {
        val rig = Rig(this)
        staged(rig, "7")
        rig.opened()
        rig.model.apply(Event.PlaybackEnded(true, Ending.Complete("8"), true))
        staged(rig, "8")
        // The first frame ends the roll-on; the viewer then picks episode 3 from the panel.
        rig.ops(2u, PlayerOp.Load("/media/8.mkv", emptyList()))
        rig.emit(MpvEvent.of(MpvNative.Event.START_FILE))
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.playhead(0.5)
        assertNull("over at the first frame", rig.model.rollOnQuietUntil)
        staged(rig, "3")
        assertNull(rig.model.rollOnQuietUntil)
    }

    @Test
    fun aRollOnThatFoundNothingLeavesTheNextStartAlone() = runTest {
        val rig = Rig(this)
        staged(rig, "7")
        rig.opened()
        upNext(rig)
        rig.model.playNext()
        rig.model.apply(Event.PlaybackKept)
        staged(rig, "3")
        assertNull(rig.model.rollOnQuietUntil)
    }

    @Test
    fun theStartingScreenWaitsSixHundredMillisecondsOnARollOnOnly() {
        assertEquals(0L, RollOnStart.untilShown(null, failed = false, now = 1_000))
        assertEquals(600L, RollOnStart.untilShown(1_600, failed = false, now = 1_000))
        assertEquals(100L, RollOnStart.untilShown(1_600, failed = false, now = 1_500))
        assertEquals("past it, at once", 0L, RollOnStart.untilShown(1_600, failed = false, now = 1_700))
        assertEquals("a failure says so at once", 0L, RollOnStart.untilShown(1_600, failed = true, now = 1_000))
    }

    @Test
    fun onceShownOnARollOnTheStartingScreenHoldsSixTenthsOfASecond() {
        assertEquals(600L, RollOnStart.stillHeld(shownAt = 1_600, rollOn = true, now = 1_600))
        assertEquals(200L, RollOnStart.stillHeld(shownAt = 1_600, rollOn = true, now = 2_000))
        assertEquals(0L, RollOnStart.stillHeld(shownAt = 1_600, rollOn = true, now = 2_300))
        assertEquals("any other start lifts with the first frame", 0L, RollOnStart.stillHeld(shownAt = 1_600, rollOn = false, now = 1_700))
    }
}
