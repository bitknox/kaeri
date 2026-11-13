package app.kaeri.store

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.DownloadPick
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** A download the engine could not start is said as such, never left looking queued. */
class DownloadNotStartedTest {
    private val model = ReadingModel { _: Command -> }

    @Test
    fun a_download_that_could_not_start_is_kept_until_said() {
        val pick = DownloadPick.NextUnread(5u)
        model.download("a:7", pick)
        model.apply(ReadingEvent.Failed(ReadingAbout.DOWNLOADS, ReadingFailure.Offline))
        assertEquals(DownloadNotStarted(pick, ReadingFailure.Offline), model.notStarted.value["a:7"])
        model.notStartedSeen("a:7")
        assertTrue(model.notStarted.value.isEmpty())
    }

    @Test
    fun a_failure_about_downloads_with_no_ask_waiting_names_no_series() {
        model.apply(ReadingEvent.Failed(ReadingAbout.DOWNLOADS, ReadingFailure.Offline))
        assertTrue(model.notStarted.value.isEmpty())
        // An ask answered by a failure is settled: the next failure is not its.
        model.download("a:7", DownloadPick.NextUnread(5u))
        model.apply(ReadingEvent.Failed(ReadingAbout.DOWNLOADS, ReadingFailure.Offline))
        model.notStartedSeen("a:7")
        model.apply(ReadingEvent.Failed(ReadingAbout.DOWNLOADS, ReadingFailure.Offline))
        assertTrue(model.notStarted.value.isEmpty())
    }

    @Test
    fun asking_again_clears_what_was_said() {
        model.download("a:7", DownloadPick.NextUnread(5u))
        model.apply(ReadingEvent.Failed(ReadingAbout.DOWNLOADS, ReadingFailure.Offline))
        model.download("a:7", DownloadPick.NextUnread(5u))
        assertTrue(model.notStarted.value.isEmpty())
    }
}
