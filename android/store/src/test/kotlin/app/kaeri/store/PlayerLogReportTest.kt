package app.kaeri.store

import app.kaeri.engine.model.DeviceClass
import app.kaeri.engine.model.Ending
import app.kaeri.engine.model.Event
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/** Report a bug's player log as the store sets it up: a playback error's kind, and the summary's flag. */
@OptIn(ExperimentalCoroutinesApi::class)
class PlayerLogReportTest {
    private val main = UnconfinedTestDispatcher()
    private lateinit var store: AppStore
    private val phone = BugReportDevice(appVersion = "0.10.0", osVersion = "16", device = DeviceClass.PHONE, build = "release")

    @Before
    fun setUp() {
        Dispatchers.setMain(main)
        store = AppStore(scope = TestScope(main), decoder = { _, _ -> null }, clock = { 0L })
    }

    @After
    fun tearDown() = Dispatchers.resetMain()

    private fun staged() = store.apply(listOf(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0)))

    @Test
    fun an_alert_after_a_failed_playback_is_a_playback_error_until_the_next_one_starts() {
        assertEquals("message", store.alertProblem("Couldn't connect").kind)
        staged()
        store.apply(listOf(Event.PlaybackEnded(false, Ending.Failed, false)))
        assertEquals(BugReports.PLAYBACK_ERROR, store.alertProblem("Segment 212 failed after 3 tries").kind)
        assertEquals("Segment 212 failed after 3 tries", store.alertProblem("Segment 212 failed after 3 tries").message)
        // Rolling on is not an ending of its own: the failure stands.
        store.apply(listOf(Event.PlaybackEnded(true, Ending.Stopped, true)))
        assertTrue(store.playbackFailed)
        staged()
        assertEquals("message", store.alertProblem("Couldn't connect").kind)
    }

    @Test
    fun a_playback_that_ended_well_is_no_playback_error() {
        staged()
        store.apply(listOf(Event.PlaybackEnded(true, Ending.Stopped, false)))
        assertFalse(store.playbackFailed)
    }

    @Test
    fun the_report_says_the_log_is_attached_only_when_asked() {
        assertFalse(store.bugReport(phone, null, emptyList()).playerLog)
        assertTrue(store.bugReport(phone, null, emptyList(), playerLog = true).playerLog)
        val problem = BugContext(BugReports.PLAYBACK_ERROR, "Segment 212 failed", title = "Stardust Wayfarer")
        val report = store.bugReport(phone, problem, listOf("c1"), playerLog = true)
        assertEquals("playback", report.error?.kind)
        assertEquals(listOf("Stardust Wayfarer"), report.`private`)
        assertEquals(listOf("c1"), report.crashes)
    }
}
