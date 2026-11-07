package app.kaeri.store

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.ExportFile
import app.kaeri.engine.model.ExportFormat
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Send to e-reader's model: a send outlives its sheet, and only Cancel send stops it. */
class ExportModelTest {
    private val sent = mutableListOf<Command>()
    private val model = ExportModel { sent += it; 7u }

    @Test
    fun a_send_held_for_wifi_goes_on_until_it_is_cancelled() {
        val job = requireNotNull(model.prepare(WORK, listOf("c1", "c2"), "kobo", ExportFormat.KEPUB))
        model.apply(ReadingEvent.ExportWaiting(job, WORK, "kobo", Msg.Verbatim("Waiting for Wi-Fi")))

        // Nothing the sheet does on its way out reaches the engine: the send is still held.
        assertTrue(model.jobs.value[job] is ExportJob.Waiting)
        assertTrue(sent.none { (it as? Command.Reading)?.v1 is ReadingCommand.CancelPrepare })

        // It goes on by itself: the next progress and the files land on the same job.
        model.apply(ReadingEvent.ExportProgress(job, 1u, 2u, 1u, 2u))
        assertEquals(ExportJob.Preparing(job, WORK, "kobo", 1u, 2u, 1u, 2u), model.jobs.value[job])
        model.apply(ReadingEvent.ExportReady(job, listOf(ExportFile("/x/a.kepub.epub", "a.kepub.epub", 10u, null))))
        val ready = model.jobs.value[job] as ExportJob.Ready
        assertEquals(listOf("c1", "c2"), ready.chapters)

        // Shared once however often the share sheet opens, and kept for Share again until put away.
        model.shared(job)
        model.shared(job)
        assertEquals(1, sent.count { (it as? Command.Reading)?.v1 is ReadingCommand.Shared })
        assertTrue(model.jobs.value[job] is ExportJob.Ready)
        model.forget(job)
        assertTrue(model.jobs.value.isEmpty())
    }

    @Test
    fun cancel_send_stops_a_held_send() {
        val job = requireNotNull(model.prepare(WORK, listOf("c1"), "kindle", ExportFormat.FIXED_EPUB))
        model.apply(ReadingEvent.ExportWaiting(job, WORK, "kindle", Msg.Verbatim("Waiting for Wi-Fi")))
        model.cancel(job)
        assertTrue(sent.any { (it as? Command.Reading)?.v1 == ReadingCommand.CancelPrepare(job) })
        assertTrue(model.jobs.value.isEmpty())
    }

    @Test
    fun a_failure_names_its_chapter_and_the_last_device_is_kept() {
        val job = requireNotNull(model.prepare(WORK, listOf("c1"), "kindle", ExportFormat.FIXED_EPUB))
        assertEquals("kindle", model.chosen.value)
        assertTrue(sent.any { (it as? Command.Reading)?.v1 == ReadingCommand.SetSendTarget("kindle") })
        model.apply(ReadingEvent.ExportFailed(job, "12", ReadingFailure.Offline))
        assertEquals(ExportJob.Failed(job, WORK, "kindle", ReadingFailure.Offline, "12"), model.jobs.value[job])
    }

    private companion object {
        const val WORK = "a:900301"
    }
}
