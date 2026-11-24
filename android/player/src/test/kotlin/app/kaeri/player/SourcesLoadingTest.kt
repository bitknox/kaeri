// The Sources tab while its sources answer: every row from the engine's first progress event, each
// line silent for 600 ms, then Asking, then the seconds from 3 s, then its answer in the starting
// screen's row words; and the rows always drawn whole from the last event.

package app.kaeri.player

import app.kaeri.engine.core.SourceCandidate
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ListingMiss
import app.kaeri.engine.model.ListingState
import app.kaeri.engine.model.MissedListing
import app.kaeri.engine.model.SourceHold
import app.kaeri.engine.model.SourceListing
import app.kaeri.engine.model.SourceOption
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class SourcesLoadingTest {
    private val context = RuntimeEnvironment.getApplication()

    private fun release(provider: String, n: Int) = SourceCandidate(
        id = "$provider-r$n", providerId = provider, providerName = "", title = "Episode 07 [1080p]", // i18n-skip
        quality = 1080u, label = null, size = null, dualAudio = false, dubbed = false, autoPick = n == 1,
    )

    private fun row(id: String, name: String, state: ListingState) = SourceListing(id, name, state)

    /** The scene's rows as the tab first hears them: the playing source, five asked, one switched off. */
    private val first = listOf(
        row("fixture", "Fixture Source", ListingState.Asking), // i18n-skip
        row("home", "Home server", ListingState.Asking), // i18n-skip
        row("folder", "Media folder", ListingState.Asking), // i18n-skip
        row("index", "Example index", ListingState.Asking), // i18n-skip
        row("second", "Second example", ListingState.Asking), // i18n-skip
        row("archive", "Archive example", ListingState.Asking), // i18n-skip
        row("old", "Old example", ListingState.Off), // i18n-skip
    )

    /** Episode 7 playing, the Sources tab just asked, and the engine's first rows said at once. */
    private fun TestScope.asked(): Rig {
        val rig = Rig(this)
        rig.model.apply(Event.PlaybackStaged(900_001u, "Hoshikuzu no Tabibito", "7", null, 1.0)) // i18n-skip
        rig.model.loadPanelReleases()
        rig.model.apply(Event.SourcesProgress(900_001u, "7", first))
        return rig
    }

    private fun Rig.say(rows: List<SourceListing>) = model.apply(Event.SourcesProgress(900_001u, "7", rows))

    private fun List<SourceListing>.with(id: String, state: ListingState) = map { if (it.providerId == id) it.copy(state = state) else it }

    @Test
    fun everyRowIsThereFromTheFirstEventInTheOrderSetInSources() = runTest {
        val rig = asked()
        assertEquals(1, rig.sent.count { it == Command.LoadSources(900_001u, "7") })
        val releases = rig.model.panelReleases!!
        assertEquals(
            listOf("Home server", "Media folder", "Example index", "Second example", "Archive example", "Old example"), // i18n-skip
            releases.others("fixture", null).map { it.name },
        )
        // Before the sources are known, what plays is left out by the name it goes by.
        assertEquals(6, releases.others(null, "Fixture Source").size) // i18n-skip
        assertTrue(releases.busy)
    }

    @Test
    fun aLineIsSilentFor600MsThenAsksThenCountsFrom3s() = runTest {
        val rig = asked()
        val asked = testScheduler.currentTime
        val releases = rig.model.panelReleases!!
        assertEquals(ListingLine.Silent, releases.line("second", asked + 599))
        assertEquals(ListingLine.Asking, releases.line("second", asked + 600))
        assertEquals(ListingLine.Asking, releases.line("second", asked + 2_999))
        assertEquals(ListingLine.Counting, releases.line("second", asked + 3_000))
        assertEquals(3, releases.seconds("second", asked + 3_000))
        assertEquals(4, releases.seconds("second", asked + 4_200))
        assertEquals(12, releases.seconds("second", asked + 12_000))
        // Switched off says so at once, with no wait.
        assertEquals(ListingLine.Answered, releases.line("old", asked))
    }

    @Test
    fun anAnswerInsideTheSilenceNeverAsksAndOneLaterReplacesTheWait() = runTest {
        val rig = asked()
        val asked = testScheduler.currentTime
        advanceTimeBy(450)
        rig.say(first.with("home", ListingState.Listed(listOf(release("home", 1)))))
        assertEquals(ListingLine.Answered, rig.model.panelReleases!!.line("home", asked + 450))
        advanceTimeBy(6_200 - 450)
        val failed = first
            .with("home", ListingState.Listed(listOf(release("home", 1))))
            .with("archive", ListingState.Missed(ListingMiss.Failed))
        rig.say(failed)
        val releases = rig.model.panelReleases!!
        assertEquals(ListingLine.Answered, releases.line("archive", asked + 6_200))
        // The seconds still count from when the slow one first said Asking, not from the last event.
        assertEquals(12, releases.seconds("second", asked + 12_000))
    }

    @Test
    fun theRowsAreAlwaysTheLastEventsWholeAndTheReleasesComeFromThem() = runTest {
        val rig = asked()
        val index = (1..5).map { release("index", it) }
        rig.say(first.with("index", ListingState.Listed(index)))
        assertEquals(index, rig.model.panelReleases!!.of("index"))
        // A later event that says another row is drawn whole: nothing is merged into it.
        rig.say(first.with("folder", ListingState.Missed(ListingMiss.Ambiguous)))
        assertEquals(emptyList<SourceCandidate>(), rig.model.panelReleases!!.of("index"))
        // Another episode's rows are not this one's.
        rig.model.apply(Event.SourcesProgress(900_001u, "8", first.with("index", ListingState.Listed(index))))
        assertEquals(emptyList<SourceCandidate>(), rig.model.panelReleases!!.of("index"))
    }

    @Test
    fun theTabIsDoneOnceNoRowIsAsked() = runTest {
        val rig = asked()
        val done = first.map { if (it.state is ListingState.Asking) it.copy(state = ListingState.Missed(ListingMiss.TimedOut)) else it }
        rig.say(done)
        assertFalse(rig.model.panelReleases!!.busy)
        rig.model.apply(Event.Sources(900_001u, "7", emptyList(), emptyList()))
        // The final answer keeps the rows the last progress said.
        assertEquals(done, rig.model.panelReleases!!.rows)
    }

    @Test
    fun eachAnswerIsSaidInTheStartingScreensRowWords() {
        fun said(state: ListingState, option: SourceOption? = null) =
            PanelWords.listing(context, row("s", "Source", state), option, "7") // i18n-skip
        assertNull(said(ListingState.Asking))
        assertEquals("Didn't answer", said(ListingState.Missed(ListingMiss.Failed))) // i18n-skip
        assertEquals("Took too long", said(ListingState.Missed(ListingMiss.TimedOut))) // i18n-skip
        assertEquals("Doesn't have episode 7", said(ListingState.Missed(ListingMiss.NoEpisode))) // i18n-skip
        assertEquals("Doesn't have this show", said(ListingState.Missed(ListingMiss.NoMatch))) // i18n-skip
        assertEquals("Couldn't match this title", said(ListingState.Missed(ListingMiss.Ambiguous))) // i18n-skip
        assertEquals("Switched off", said(ListingState.Off)) // i18n-skip
        assertEquals(
            "Not asked · Torrents are off on cellular", // i18n-skip
            said(ListingState.Missed(ListingMiss.HeldBack(SourceHold.CELLULAR))),
        )
        val option = SourceOption(providerId = "s", name = "Source", covered = 9u, of = 9u, pinned = false, note = null, releases = emptyList()) // i18n-skip
        assertEquals(PanelWords.source(context, option), said(ListingState.Listed(listOf(release("s", 1))), option))
    }

    @Test
    fun aSourceThatCouldNotMatchTheTitleSaysSoOnTheStartingScreen() {
        val missed = MissedListing("Media folder", ListingMiss.Ambiguous) // i18n-skip
        assertEquals("Media folder couldn't match this title", missedText(context, missed, "7")) // i18n-skip
        // It answered: none of the sources lists the episode, rather than none that answered.
        assertEquals(
            noneListsText(context, listOf(MissedListing("Brook", ListingMiss.NoEpisode)), "7"), // i18n-skip
            noneListsText(context, listOf(missed), "7"),
        )
    }
}
