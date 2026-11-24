// The player panel's model on a scripted engine and a scripted mpv: what its picks send, a switch
// from the pick until it lands or fails, the picture held while it loads, and an episode's picks
// staying its own.

package app.kaeri.player

import android.graphics.Bitmap
import app.kaeri.engine.core.SourceCandidate
import app.kaeri.engine.core.Translation
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.MpvOption
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.PlayerChoice
import app.kaeri.engine.model.PlayerCommand
import app.kaeri.engine.model.PlayerOp
import app.kaeri.engine.model.SourceSwitch
import app.kaeri.engine.model.SwitchTo
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class PlayerPanelTest {
    private val harbor = SourceCandidate(
        id = "h-1080", providerId = "harbor", providerName = "Harbor Mirror", title = "Stardust Wayfarer - 07 (1080p)",
        quality = 1080u, label = "paper-lantern", size = "1.4 GiB", dualAudio = false, dubbed = false, autoPick = true,
    )
    private val tidewater = harbor.copy(id = "t-1080", providerId = "tidewater", providerName = "Tidewater", autoPick = false)
    private val toHarbor = SwitchTo("harbor", "Harbor Mirror", "h-1080", null)
    private val toTidewater = SwitchTo("tidewater", "Tidewater", "t-1080", null)

    /** Episode 7 playing at 600 s, with the panel's releases listed. */
    private fun TestScope.playing(): Rig {
        val rig = Rig(this)
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
        rig.opened()
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.playhead(600.0)
        rig.model.loadPanelReleases()
        rig.model.apply(Event.Sources(900_001u, "7", listOf(harbor, tidewater), emptyList()))
        return rig
    }

    private fun Rig.switch(state: SourceSwitch) {
        model.apply(Event.PlayerSwitch(1u, state))
        settle()
    }

    @Test
    fun aSwitchLandedWhileSlowerSourcesAreAskedPlaysTheReleaseItsSourceListed() = runTest {
        val rig = Rig(this)
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
        rig.opened()
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.playhead(600.0)
        rig.model.loadPanelReleases()
        rig.model.apply(
            Event.SourcesProgress(
                900_001u, "7",
                listOf(
                    app.kaeri.engine.model.SourceListing("harbor", "Harbor Mirror", app.kaeri.engine.model.ListingState.Listed(listOf(harbor))),
                    app.kaeri.engine.model.SourceListing("tidewater", "Tidewater", app.kaeri.engine.model.ListingState.Asking),
                ),
            ),
        )
        // Switched from elsewhere (a remote), so no row's pick is on its way.
        rig.switch(SourceSwitch.Landed(toHarbor, 600.0))
        assertEquals(harbor, rig.model.playingRelease)
    }

    @Test
    fun aLandedReleaseIsNeverAnotherRowsPick() = runTest {
        val rig = Rig(this)
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
        rig.opened()
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.playhead(600.0)
        rig.model.loadPanelReleases()
        // Tidewater is picked while the engine still lands on Harbor's release, which no list said.
        rig.model.switchTo(tidewater)
        rig.switch(SourceSwitch.Landed(toHarbor, 600.0))
        assertNull(rig.model.playingRelease)
    }

    @Test
    fun thePanelAsksForTheEpisodesReleasesOnce() = runTest {
        val rig = playing()
        assertEquals(1, rig.sent.count { it == Command.LoadSources(900_001u, "7") })
        assertEquals(listOf(harbor, tidewater), rig.model.panelReleases?.candidates)
        rig.model.loadPanelReleases()
        assertEquals(1, rig.sent.count { it is Command.LoadSources })
        rig.model.loadPanelReleases(again = true)
        assertEquals(2, rig.sent.count { it is Command.LoadSources })
    }

    @Test
    fun aPickSendsTheSwitchAndItsRowWaitsSilentlyThenCounts() = runTest {
        val rig = playing()
        val picked = testScheduler.currentTime
        rig.model.switchTo(harbor)
        assertEquals(Command.SwitchSource(900_001u, "7", "harbor", "h-1080"), rig.sent.last())
        val pick = rig.model.panelSwitch!!
        assertEquals(PanelSwitch.Phase.Asked, pick.phase)
        assertTrue(pick.on(SwitchTarget(toHarbor)))
        assertFalse(pick.on(SwitchTarget(toTidewater)))
        assertFalse("silent at first", pick.spinning(picked + 599))
        assertTrue(pick.spinning(picked + 600))
        assertNull(pick.counting(picked + 2_999))
        assertEquals(3, pick.counting(picked + 3_400))

        rig.switch(SourceSwitch.Finding(toHarbor))
        assertEquals(PanelSwitch.Phase.Finding, rig.model.panelSwitch?.phase)
        assertEquals("the wait counts from the pick", picked, rig.model.panelSwitch?.since)
        assertFalse("the episode plays on while it is found", rig.model.holding)
    }

    @Test
    fun aSwitchHoldsThePictureWhileItLoadsAndLetsItGoAsItLands() = runTest {
        val rig = playing()
        val held = Bitmap.createBitmap(4, 4, Bitmap.Config.ARGB_8888)
        rig.model.snapshot = { held }
        rig.model.switchTo(harbor)
        rig.switch(SourceSwitch.Finding(toHarbor))
        rig.switch(SourceSwitch.Loading(toHarbor, 600.0))
        assertTrue(rig.model.holding)
        assertEquals("copied before the load clears it", held, rig.model.heldPicture?.bitmap)

        // The engine's load at the playhead: the seek line stays where it was meanwhile.
        rig.ops(1u, PlayerOp.Load("/media/harbor.mkv", listOf(MpvOption("start", "600"))))
        rig.emit(MpvEvent.of(MpvNative.Event.START_FILE))
        assertEquals(600.0, rig.model.position, 0.0)
        assertFalse(rig.model.hasPicture)
        assertTrue("the starting screen stays away", rig.model.holding)

        rig.emit(MpvEvent.of(MpvNative.Event.FILE_LOADED))
        rig.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        rig.switch(SourceSwitch.Landed(toHarbor, 600.0))
        assertFalse(rig.model.holding)
        assertNull("the row's switch is done", rig.model.panelSwitch)
        assertEquals("Harbor Mirror", rig.model.playingSource)
        assertEquals(harbor, rig.model.playingRelease)
        assertEquals(1, rig.model.landings)
        assertNotNull("crossfading away", rig.model.heldPicture)
        advanceTimeBy(PlayerModel.HELD_FADE + 1)
        rig.settle()
        assertNull(rig.model.heldPicture)
    }

    @Test
    fun aFailureStaysOnItsRowAndTryAgainPicksItAgain() = runTest {
        val rig = playing()
        rig.model.switchTo(tidewater)
        rig.switch(SourceSwitch.Finding(toTidewater))
        val text = Msg.PlayerPanelSwitchFailed("Tidewater", "Fixture Source")
        rig.switch(SourceSwitch.Failed(toTidewater, 600.0, text, null))
        assertEquals(text, rig.model.panelSwitch?.failed)
        assertFalse(rig.model.panelSwitch!!.spinning(testScheduler.currentTime + 10_000))
        assertNull(rig.model.panelSwitch!!.counting(testScheduler.currentTime + 10_000))
        assertNull("nothing new plays", rig.model.playingRelease)
        assertFalse(rig.model.holding)

        val sent = rig.sent.size
        rig.model.retrySwitch()
        assertEquals(Command.SwitchSource(900_001u, "7", "tidewater", "t-1080"), rig.sent.drop(sent).single())
        assertEquals(PanelSwitch.Phase.Asked, rig.model.panelSwitch?.phase)
    }

    @Test
    fun aFailureAfterTheLoadLetsTheHeldPictureGo() = runTest {
        val rig = playing()
        rig.model.snapshot = { Bitmap.createBitmap(4, 4, Bitmap.Config.ARGB_8888) }
        rig.model.switchTo(harbor)
        rig.switch(SourceSwitch.Loading(toHarbor, 600.0))
        assertTrue(rig.model.holding)
        rig.switch(SourceSwitch.Failed(toHarbor, 600.0, Msg.PlayerPanelSwitchFailed("Harbor Mirror", "Fixture Source"), null))
        assertFalse(rig.model.holding)
        advanceTimeBy(PlayerModel.HELD_FADE + 1)
        rig.settle()
        assertNull(rig.model.heldPicture)
    }

    @Test
    fun aSwitchThePanelDidNotPickGetsARowOfItsOwn() = runTest {
        val rig = playing()
        rig.switch(SourceSwitch.Finding(toTidewater))
        assertTrue(rig.model.panelSwitch!!.on(SwitchTarget(toTidewater)))
        assertEquals(PanelSwitch.Phase.Finding, rig.model.panelSwitch?.phase)
    }

    @Test
    fun anotherEpisodeDropsTheReleasesAndThePicks() = runTest {
        val rig = playing()
        rig.model.switchTo(harbor)
        rig.switch(SourceSwitch.Landed(toHarbor, 600.0))
        assertEquals(harbor, rig.model.playingRelease)
        rig.model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "8", null, 1.0))
        assertNull(rig.model.panelReleases)
        assertNull(rig.model.panelSwitch)
        assertNull("a pick is for its episode only", rig.model.playingRelease)
    }

    private val toDub = SwitchTo("", "", null, null, Translation.DUB)
    private val dubOnQuiet = SwitchTo("quiet", "Quiet Shelf", null, null, Translation.DUB)

    @Test
    fun theOtherVersionFollowsFindingLoadingAndLanded() = runTest {
        val rig = playing()
        rig.model.switchTo(harbor)
        rig.switch(SourceSwitch.Landed(toHarbor, 600.0))
        val picked = testScheduler.currentTime
        rig.model.switchTo(Translation.DUB)
        assertEquals(PlayerCommand.Choose(PlayerChoice.Translation(Translation.DUB)), rig.controls.last())
        assertEquals(PanelSwitch.Phase.Asked, rig.model.versionSwitch?.phase)
        assertFalse("silent at first", rig.model.versionSwitch!!.spinning(picked + 599))
        assertTrue(rig.model.versionSwitch!!.spinning(picked + 600))
        assertNull(rig.model.versionSwitch!!.counting(picked + 2_999))
        assertEquals(3, rig.model.versionSwitch!!.counting(picked + 3_000))

        rig.switch(SourceSwitch.Finding(toDub))
        assertEquals(PanelSwitch.Phase.Finding, rig.model.versionSwitch?.phase)
        assertEquals("the wait counts from the pick", picked, rig.model.versionSwitch?.since)
        assertNull("no source row shows the version's switch", rig.model.panelSwitch)
        rig.switch(SourceSwitch.Loading(dubOnQuiet, 600.0))
        assertTrue("the picture is held while the other version loads", rig.model.holding)

        val asked = rig.sent.size
        rig.switch(SourceSwitch.Landed(dubOnQuiet, 600.0))
        assertNull(rig.model.versionSwitch)
        assertFalse(rig.model.holding)
        assertEquals("Quiet Shelf", rig.model.playingSource)
        assertNull("the release picked was the other version's", rig.model.playingRelease)
        assertEquals("the dub's releases are listed", listOf(Command.LoadSources(900_001u, "7")), rig.sent.drop(asked).filterIsInstance<Command.LoadSources>())
    }

    @Test
    fun aVersionThatFailsSaysSoUnderTheSwitchAndPlaysOn() = runTest {
        val rig = playing()
        rig.model.switchTo(Translation.DUB)
        rig.switch(SourceSwitch.Loading(dubOnQuiet, 600.0))
        val text = Msg.PlayerPanelDubFailed("7")
        rig.switch(SourceSwitch.Failed(dubOnQuiet, 600.0, text, null))
        assertEquals(text, rig.model.versionSwitch?.failed)
        assertFalse(rig.model.versionSwitch!!.spinning(testScheduler.currentTime + 10_000))
        assertFalse(rig.model.holding)
        // Picked again from the switch.
        rig.model.switchTo(Translation.DUB)
        assertEquals(PanelSwitch.Phase.Asked, rig.model.versionSwitch?.phase)
    }

    @Test
    fun oneVersionSwitchAtATime() = runTest {
        val rig = playing()
        rig.model.switchTo(Translation.DUB)
        val sent = rig.controls.size
        rig.model.switchTo(Translation.DUB)
        assertEquals(sent, rig.controls.size)
    }

    @Test
    fun anotherSourceTheEngineMovedToIsWhatPlaysNow() = runTest {
        val rig = playing()
        rig.model.switchTo(harbor)
        rig.switch(SourceSwitch.Landed(toHarbor, 600.0))
        assertEquals(harbor, rig.model.playingRelease)
        // The release's stream broke off and another source took over.
        rig.switch(SourceSwitch.Landed(SwitchTo("quiet", "Quiet Shelf", null, "3"), 700.0))
        assertEquals("Quiet Shelf", rig.model.playingSource)
        assertNull(rig.model.playingRelease)
    }

    @Test
    fun anotherStreamOfTheReleasePlayingKeepsTheRelease() = runTest {
        val rig = playing()
        rig.model.switchTo(harbor)
        rig.switch(SourceSwitch.Landed(toHarbor, 600.0))
        rig.switch(SourceSwitch.Landed(SwitchTo("harbor", "Harbor Mirror", null, "2"), 700.0))
        assertEquals(harbor, rig.model.playingRelease)
    }

    @Test
    fun theEpisodesTabPlaysAndMarks() = runTest {
        val rig = playing()
        rig.model.playEpisode("8", fromStart = false)
        assertEquals(PlayerCommand.Choose(PlayerChoice.Episode("8")), rig.controls.last())
        rig.model.playEpisode("5", fromStart = true)
        assertEquals(Command.Play(900_001u, "5", true), rig.sent.last())
        rig.model.markWatched("9")
        assertEquals(Command.SetWatched(900_001u, listOf("9"), true), rig.sent.last())
    }

    @Test
    fun thePanelOpensOnItsTabAndClosingBringsTheControlsBack() = runTest {
        val rig = playing()
        rig.model.openPanel()
        assertTrue(rig.model.panelOpen)
        assertEquals(PanelTab.Episodes, rig.model.panelTab)
        rig.model.openPanel(PanelTab.Sources)
        assertEquals(PanelTab.Sources, rig.model.panelTab)
        rig.model.closePanel()
        assertFalse(rig.model.panelOpen)
        assertTrue(rig.model.controlsVisible)
    }
}
