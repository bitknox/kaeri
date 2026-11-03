package app.kaeri.player

import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.StartStep
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class StartProgressTest {
    private fun start(resume: Double? = null) = StartProgress(1u, "7", since = 0L, resume = resume)

    private fun asking(place: UInt = 1u, total: UInt = 3u) = StartStep.AskingSource("Alpha", place, total, false)

    @Test
    fun aLineStaysSixHundredMillisecondsBeforeTheNextOneReplacesIt() {
        val p = start()
            .heard(StartStep.LookingUpTitle, Msg.StartLookingUpTitle, 0)
            .heard(asking(), Msg.StartGettingSourcesReady, 100)
        assertEquals(Msg.StartLookingUpTitle, p.heldLine(100))
        assertEquals(Msg.StartLookingUpTitle, p.heldLine(599))
        assertEquals(Msg.StartGettingSourcesReady, p.heldLine(600))
    }

    @Test
    fun onlyTheLatestOfSeveralLinesHeardDuringAHoldShows() {
        val p = start()
            .heard(StartStep.LookingUpTitle, Msg.StartLookingUpTitle, 0)
            .heard(asking(), Msg.StartGettingSourcesReady, 100)
            .heard(StartStep.FetchingSubtitles, Msg.StartBuffering, 200)
        assertEquals(Msg.StartLookingUpTitle, p.heldLine(599))
        assertEquals(Msg.StartBuffering, p.heldLine(600))
    }

    @Test
    fun theButtonsArriveAtTheirMoments() {
        val p = start(resume = 606.0).heard(asking(), null, 0)
        assertEquals(listOf(StartButton.Cancel), p.buttons(500, false))
        assertEquals(listOf(StartButton.Cancel, StartButton.FromBeginning), p.buttons(1_000, false))
        assertEquals(listOf(StartButton.Cancel, StartButton.ChooseSource, StartButton.Details), p.buttons(3_000, false))
        assertEquals(StartButton.Skip, p.buttons(8_000, false).last())
    }

    @Test
    fun playFromBeginningGoesOnceAsked() {
        val p = start(resume = 606.0).fromTheBeginning()
        assertFalse(p.resuming(1_500))
        assertFalse(StartButton.FromBeginning in p.buttons(1_500, false))
    }

    @Test
    fun skipNeedsAnotherSourceToMoveOnTo() {
        val last = start().heard(asking(place = 3u, total = 3u), null, 0)
        assertFalse(StartButton.Skip in last.buttons(9_000, false))
        val first = start().heard(asking(place = 1u, total = 3u), null, 0)
        assertTrue(StartButton.Skip in first.buttons(9_000, false))
    }

    @Test
    fun offlineOffersOnlyCancelAndDownloads() {
        assertEquals(listOf(StartButton.Cancel, StartButton.ShowDownloads), start().buttons(30_000, true))
    }

    @Test
    fun theDeterminateLineIsOnlyForTheBufferingPercent() {
        val p = start()
            .heard(StartStep.Buffering(42u), Msg.StartBuffering, 0)
        assertEquals(42, p.determinate(100))
        assertNull(start().heard(asking(), Msg.StartGettingSourcesReady, 0).determinate(100))
    }
}
