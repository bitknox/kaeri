package app.kaeri.store

import java.time.Clock
import java.time.Instant
import java.time.ZoneOffset
import java.util.Locale
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import app.kaeri.engine.model.CheckOutcome
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.EpisodeCheck
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.MissingCommand
import app.kaeri.engine.model.MissingPhase
import app.kaeri.engine.model.SourceNote
import app.kaeri.engine.model.SourceOption
import app.kaeri.engine.model.TitleProgress

/** The missing episode's words on Android, and Check again's waiting rule. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class MissingEpisodeTest {
    private val context = RuntimeEnvironment.getApplication()
    private val held = AppClock.clock
    private val aired = 1_794_065_400L // 7 November 2026, 15.30 UTC
    private val now = aired + 25 * 60

    @Before
    fun setUp() {
        Locale.setDefault(Locale.UK)
        AppClock.clock = Clock.fixed(Instant.ofEpochSecond(now), ZoneOffset.UTC)
    }

    @After
    fun tearDown() {
        AppClock.clock = held
    }

    private fun row(n: Int, unsourced: Boolean, airsAt: Long?) = EpisodeRow(
        number = n.toString(), title = null, thumbnail = null, description = null, durationSecs = 1440u,
        watched = 0.0, completed = n <= 5, kind = null, skippable = false, airsAt = airsAt, unsourced = unsourced,
    )

    private val entry = TestEntries.entry(900002u).copy(progress = TitleProgress(done = 5u, next = 6u))
    private fun rows(airedAt: Long? = aired) = (1..5).map { row(it, false, aired - (6 - it) * 7 * 86_400L) } + row(6, true, airedAt)

    @Test
    fun just_aired_names_the_episode_and_the_recheck() {
        val missing = entry.missingIn(rows(), now)!!
        assertEquals(MissingPhase.QUIET, missing.phase)
        assertEquals(5u, missing.previous)
        assertEquals(
            "Episode 6 aired 25 min ago. Your sources don't have it yet. Kaeri looks again every 15 minutes.",
            MissingWords.status(context, missing, null, offline = false, now),
        )
        assertEquals(
            "Episode 6 aired 25 min ago. Your sources still don't have it. Kaeri looks again every 15 minutes.",
            MissingWords.status(context, missing, CheckOutcome.MISSING, offline = false, now),
        )
        assertEquals(
            "Episode 6 aired 25 min ago. Your sources don't have it yet. Kaeri looks for it when you're back online.",
            MissingWords.status(context, missing, null, offline = true, now),
        )
        assertEquals("Aired 25 min ago · Not in your sources yet", MissingWords.unsourcedLine(context, aired, now))
        assertTrue(MissingWords.checkHint(context, missing)!!.startsWith("Kaeri also looks by itself every 15 minutes for 6 hours"))
    }

    @Test
    fun the_words_change_with_time() {
        val later = aired + 2 * 86_400L
        val after = entry.missingIn(rows(), later)!!
        assertEquals(MissingPhase.AFTER, after.phase)
        assertEquals("Episode 6 aired on 7 Nov. Your sources still don't have it.", MissingWords.status(context, after, null, offline = true, later))
        assertNull(MissingWords.checkHint(context, after))
        assertEquals("Aired 7 Nov 2026 · Not in your sources yet", MissingWords.unsourcedLine(context, aired, later))
        val week = aired + 8 * 86_400L
        val long = entry.missingIn(rows(), week)!!
        assertEquals(MissingPhase.LONG_AGO, long.phase)
        assertEquals("Episode 6 aired on 7 Nov. None of your sources has it.", MissingWords.status(context, long, null, offline = false, week))
        assertEquals("Aired 7 Nov 2026 · No source has this episode", MissingWords.unsourcedLine(context, aired, week))
        val undated = entry.missingIn(rows(airedAt = null), now)!!
        assertEquals("No source has this episode", MissingWords.status(context, undated, null, offline = false, now))
    }

    @Test
    fun each_source_says_what_it_has() {
        fun option(first: UInt?, last: UInt?, lacking: List<UInt>, covered: UInt = 5u, note: SourceNote? = null) =
            SourceOption("fixture", "Fixture", covered, 6u, note = note, first = first, last = last, lacking = lacking, lackingSince = aired)
        assertEquals("Episodes 1 to 5. Not 6 yet", MissingWords.coverage(context, option(1u, 5u, listOf(6u)), now))
        assertEquals("Episodes 1 to 4. Not 5 or 6 yet", MissingWords.coverage(context, option(1u, 4u, listOf(5u, 6u), 4u), now))
        assertEquals("Episodes 1 to 6", MissingWords.coverage(context, option(1u, 6u, emptyList(), 6u), now))
        assertNull(MissingWords.coverage(context, option(null, null, emptyList(), 0u, SourceNote.Resting), now))
    }

    @Test
    fun check_again_is_sent_and_its_answers_kept() {
        val sent = mutableListOf<Command>()
        val model = MissingModel { sent += it }
        model.checkAgain(900002u, 6u)
        assertEquals(listOf<Command>(Command.Missing(MissingCommand.CheckAgain(900002u, 6u))), sent)
        model.apply(EpisodeCheck(900002u, 6u, CheckOutcome.MISSING, true))
        assertEquals(CheckOutcome.MISSING, model.outcome(900002u, 6u))
        // Offline asks nothing, and the words stay.
        model.apply(EpisodeCheck(900002u, 6u, CheckOutcome.OFFLINE, true))
        assertEquals(CheckOutcome.MISSING, model.outcome(900002u, 6u))
        assertNull(model.outcome(900002u, 7u))
    }

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test
    fun checking_shows_after_600_ms_and_stays_300() {
        val scope = TestScope(StandardTestDispatcher())
        val quick = CheckWait(scope)
        assertTrue(quick.press())
        assertFalse(quick.press())
        scope.advanceTimeBy(500)
        quick.answered()
        scope.advanceTimeBy(1_000)
        assertFalse(quick.checking.value)

        val slow = CheckWait(scope)
        slow.press()
        scope.advanceTimeBy(601)
        scope.runCurrent()
        assertTrue(slow.checking.value)
        slow.answered()
        scope.advanceTimeBy(100)
        scope.runCurrent()
        assertTrue(slow.checking.value)
        scope.advanceTimeBy(250)
        scope.runCurrent()
        assertFalse(slow.checking.value)
    }
}
