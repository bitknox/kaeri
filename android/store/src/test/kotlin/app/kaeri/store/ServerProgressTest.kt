// Did my server get it: the page line per chapter and each server row's second line, on virtual
// time. The waiting rule (nothing for 600 ms, then Sending for at least 0.3 s), results that come
// before ChapterRead, Try again and its answer, the give-up after a day, offline then back online.

package app.kaeri.store

import app.kaeri.engine.model.BookServerKind
import app.kaeri.engine.model.BookServerRow
import app.kaeri.engine.model.BookSourceCommand
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.WriteBackProblem
import app.kaeri.engine.model.WriteBackResult
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ServerProgressTest {
    private val work = "kavita:books/tidebell"
    private val chapter = "kavita:books/tidebell/12"
    private val other = "kavita:books/tidebell/13"
    private val host = "books.home.example"
    private val server = "books"

    private fun row(
        pending: Int = 0,
        sending: Boolean = false,
        savedAt: Long? = null,
        problem: WriteBackProblem? = null,
        problemAt: Long? = null,
        receives: Boolean = true,
    ) = BookServerRow(
        id = server,
        kind = BookServerKind.KAVITA,
        name = host,
        address = "https://$host",
        series = 212u,
        reportProgress = receives,
        off = false,
        signedOut = false,
        libraries = 2u,
        empty = false,
        rechecksAt = null,
        keyLast4 = null,
        progressPending = pending.toUInt(),
        progressSending = sending,
        progressSavedAt = savedAt,
        progressProblem = problem,
        progressProblemAt = problemAt,
    )

    private class Sent : (Command) -> Unit {
        val commands = mutableListOf<Command>()
        override fun invoke(command: Command) {
            commands += command
        }
    }

    private fun TestScope.lines(sent: Sent = Sent(), quiet: Boolean = false) =
        ServerLines(backgroundScope, send = sent, hostFor = { if (it == work) host else null }, quiet = { quiet })

    private fun TestScope.line(lines: ServerLines): ServerLine? {
        runCurrent()
        return lines.lines.value[chapter]
    }

    private val retry = Command.BookSources(BookSourceCommand.RetryProgress(server))

    // The waiting rule on the page.

    @Test
    fun an_answer_inside_600_ms_shows_saved_with_no_sending() = runTest {
        val lines = lines()
        lines.chapterRead(work, chapter)
        advanceTimeBy(599)
        assertNull(line(lines))
        lines.sent(chapter, server, host, WriteBackResult.Saved)
        assertEquals(ServerLine.Saved(host), line(lines))
    }

    @Test
    fun sending_shows_at_600_ms_and_holds_300_ms_before_the_answer() = runTest {
        val lines = lines()
        lines.chapterRead(work, chapter)
        advanceTimeBy(599)
        assertNull(line(lines))
        advanceTimeBy(2)
        assertEquals(ServerLine.Sending(host), line(lines))
        // The answer lands 50 ms after Sending showed: it waits out the hold.
        advanceTimeBy(49)
        lines.sent(chapter, server, host, WriteBackResult.Saved)
        assertEquals(ServerLine.Sending(host), line(lines))
        advanceTimeBy(248)
        assertEquals(ServerLine.Sending(host), line(lines))
        advanceTimeBy(3)
        assertEquals(ServerLine.Saved(host), line(lines))
    }

    @Test
    fun an_answer_after_the_hold_shows_at_once() = runTest {
        val lines = lines()
        lines.chapterRead(work, chapter)
        advanceTimeBy(2_000)
        assertEquals(ServerLine.Sending(host), line(lines))
        lines.sent(chapter, server, host, WriteBackResult.NotSaved(WriteBackProblem.REFUSED_KEY))
        assertEquals(ServerLine.Failed(host, WhyNot.KEY), line(lines))
    }

    @Test
    fun a_work_from_no_server_has_no_line_and_holds_no_space() = runTest {
        val lines = lines()
        lines.chapterRead("folder:comics/a", "folder:comics/a/1")
        advanceTimeBy(5_000)
        assertTrue(lines.lines.value.isEmpty())
        assertFalse(lines.holds("folder:comics/a", "folder:comics/a/1"))
        assertTrue(lines.holds(work, chapter))
    }

    // Results known before any send come before ChapterRead.

    @Test
    fun offline_known_before_chapter_read_comes_with_the_page() = runTest {
        val lines = lines()
        lines.sent(chapter, server, host, WriteBackResult.Waiting(WriteBackProblem.OFFLINE))
        lines.chapterRead(work, chapter)
        assertEquals(ServerLine.Offline(host), line(lines))
        advanceTimeBy(2_000)
        assertEquals(ServerLine.Offline(host), line(lines))
    }

    @Test
    fun not_set_to_receive_before_chapter_read_never_shows_sending() = runTest {
        val lines = lines()
        lines.sent(chapter, server, host, WriteBackResult.NotSaved(WriteBackProblem.NOT_SET_TO_RECEIVE))
        lines.chapterRead(work, chapter)
        advanceTimeBy(5_000)
        assertNull(line(lines))
    }

    @Test
    fun a_held_chapter_says_why_before_chapter_read() = runTest {
        val lines = lines()
        lines.sent(chapter, server, host, WriteBackResult.NotSaved(WriteBackProblem.SERIES_GONE))
        lines.chapterRead(work, chapter)
        advanceTimeBy(1_000)
        assertEquals(ServerLine.Failed(host, WhyNot.GONE), line(lines))
    }

    @Test
    fun sending_on_close_leaves_the_page_without_a_line() = runTest {
        val lines = lines(quiet = true)
        lines.chapterRead(work, chapter)
        advanceTimeBy(2_000)
        lines.sent(chapter, server, host, WriteBackResult.Saved)
        assertNull(line(lines))
        assertFalse(lines.holds(work, chapter))
    }

    // Each result's words.

    @Test
    fun results_map_to_lines() {
        assertEquals(ServerLine.Saved(host), WriteBackResult.Saved.line(host))
        assertEquals(ServerLine.Offline(host), WriteBackResult.Waiting(WriteBackProblem.OFFLINE).line(host))
        assertEquals(ServerLine.Retrying(host), WriteBackResult.Waiting(WriteBackProblem.NO_ANSWER).line(host))
        assertEquals(ServerLine.Retrying(host), WriteBackResult.Waiting(WriteBackProblem.SERVER_ERROR).line(host))
        assertEquals(ServerLine.Failed(host, WhyNot.KEY), WriteBackResult.NotSaved(WriteBackProblem.REFUSED_KEY).line(host))
        assertEquals(ServerLine.Failed(host, WhyNot.PASSWORD), WriteBackResult.NotSaved(WriteBackProblem.REFUSED_PASSWORD).line(host))
        assertEquals(ServerLine.Failed(host, WhyNot.GONE), WriteBackResult.NotSaved(WriteBackProblem.SERIES_GONE).line(host))
        assertEquals(ServerLine.Failed(host, WhyNot.GAVE_UP), WriteBackResult.NotSaved(WriteBackProblem.NO_ANSWER_FOR_A_DAY).line(host))
        assertNull(WriteBackResult.NotSaved(WriteBackProblem.NOT_SET_TO_RECEIVE).line(host))
    }

    // Try again on the line, answered on the row.

    @Test
    fun try_again_on_the_line_sends_the_server_s_queue_and_takes_the_row_s_answer() = runTest {
        val sent = Sent()
        val lines = lines(sent)
        lines.rows(listOf(row(pending = 3, problem = WriteBackProblem.REFUSED_KEY, problemAt = 100)))
        lines.sent(chapter, server, host, WriteBackResult.NotSaved(WriteBackProblem.REFUSED_KEY))
        lines.retry(chapter)
        assertEquals(listOf<Command>(retry), sent.commands)
        // The engine publishes the row at once with the old facts: nothing changes yet.
        lines.rows(listOf(row(pending = 3, problem = WriteBackProblem.REFUSED_KEY, problemAt = 100)))
        assertEquals(ServerLine.Failed(host, WhyNot.KEY), line(lines))
        lines.rows(listOf(row(pending = 3, sending = true, problem = WriteBackProblem.REFUSED_KEY, problemAt = 100)))
        advanceTimeBy(601)
        assertEquals(ServerLine.Sending(host), line(lines))
        lines.rows(listOf(row(pending = 0, savedAt = 200, problem = null)))
        advanceTimeBy(298)
        assertEquals(ServerLine.Sending(host), line(lines))
        advanceTimeBy(2)
        assertEquals(ServerLine.Saved(host), line(lines))
    }

    @Test
    fun try_again_refused_again_in_the_same_second_still_answers() = runTest {
        val lines = lines()
        lines.rows(listOf(row(pending = 3, problem = WriteBackProblem.REFUSED_KEY, problemAt = 100)))
        lines.sent(chapter, server, host, WriteBackResult.NotSaved(WriteBackProblem.REFUSED_KEY))
        lines.retry(chapter)
        lines.rows(listOf(row(pending = 3, sending = true, problem = WriteBackProblem.REFUSED_KEY, problemAt = 100)))
        lines.rows(listOf(row(pending = 3, problem = WriteBackProblem.REFUSED_KEY, problemAt = 100)))
        advanceTimeBy(100)
        assertEquals(ServerLine.Failed(host, WhyNot.KEY), line(lines))
        // No Sending was ever shown.
        advanceTimeBy(2_000)
        assertEquals(ServerLine.Failed(host, WhyNot.KEY), line(lines))
    }

    @Test
    fun try_again_twice_sends_once() = runTest {
        val sent = Sent()
        val lines = lines(sent)
        lines.rows(listOf(row(pending = 1, problem = WriteBackProblem.NO_ANSWER, problemAt = 100)))
        lines.sent(chapter, server, host, WriteBackResult.Waiting(WriteBackProblem.NO_ANSWER))
        lines.retry(chapter)
        lines.retry(chapter)
        assertEquals(1, sent.commands.size)
    }

    @Test
    fun saved_has_no_try_again() = runTest {
        val sent = Sent()
        val lines = lines(sent)
        lines.sent(chapter, server, host, WriteBackResult.Saved)
        lines.retry(chapter)
        assertTrue(sent.commands.isEmpty())
    }

    // Retries on their own, and the give-up after a day.

    @Test
    fun didn_t_answer_becomes_not_saved_after_a_day_of_tries() = runTest {
        val lines = lines()
        lines.chapterRead(work, chapter)
        advanceTimeBy(700)
        lines.sent(chapter, server, host, WriteBackResult.Waiting(WriteBackProblem.NO_ANSWER))
        advanceTimeBy(300)
        assertEquals(ServerLine.Retrying(host), line(lines))
        // A try in the backoff: still waiting, quietly.
        lines.rows(listOf(row(pending = 1, sending = true, problem = WriteBackProblem.NO_ANSWER, problemAt = 100)))
        lines.rows(listOf(row(pending = 1, problem = WriteBackProblem.NO_ANSWER, problemAt = 130)))
        assertEquals(ServerLine.Retrying(host), line(lines))
        lines.rows(listOf(row(pending = 1, problem = WriteBackProblem.NO_ANSWER_FOR_A_DAY, problemAt = 86_500)))
        assertEquals(ServerLine.Failed(host, WhyNot.GAVE_UP), line(lines))
    }

    @Test
    fun a_retry_that_passes_on_its_own_saves_the_line() = runTest {
        val lines = lines()
        lines.sent(chapter, server, host, WriteBackResult.Waiting(WriteBackProblem.SERVER_ERROR))
        lines.rows(listOf(row(pending = 0, savedAt = 400)))
        assertEquals(ServerLine.Saved(host), line(lines))
    }

    // Offline, then back online.

    @Test
    fun back_online_a_quick_send_crossfades_to_saved_without_sending() = runTest {
        val lines = lines()
        lines.sent(chapter, server, host, WriteBackResult.Waiting(WriteBackProblem.OFFLINE))
        lines.chapterRead(work, chapter)
        lines.rows(listOf(row(pending = 3, problem = WriteBackProblem.OFFLINE)))
        assertEquals(ServerLine.Offline(host), line(lines))
        // Back online: the queue sends at once.
        lines.rows(listOf(row(pending = 3, sending = true)))
        advanceTimeBy(400)
        assertEquals(ServerLine.Offline(host), line(lines))
        lines.rows(listOf(row(pending = 0, savedAt = 500)))
        assertEquals(ServerLine.Saved(host), line(lines))
    }

    @Test
    fun back_online_a_slow_send_shows_sending_first() = runTest {
        val lines = lines()
        lines.sent(chapter, server, host, WriteBackResult.Waiting(WriteBackProblem.OFFLINE))
        lines.rows(listOf(row(pending = 3)))
        advanceTimeBy(601)
        assertEquals(ServerLine.Sending(host), line(lines))
        lines.rows(listOf(row(pending = 1, sending = true)))
        lines.rows(listOf(row(pending = 0, savedAt = 900)))
        advanceTimeBy(300)
        assertEquals(ServerLine.Saved(host), line(lines))
    }

    @Test
    fun a_saved_line_stays_saved_whatever_the_row_says_later() = runTest {
        val lines = lines()
        lines.sent(chapter, server, host, WriteBackResult.Saved)
        lines.rows(listOf(row(pending = 2, problem = WriteBackProblem.OFFLINE)))
        assertEquals(ServerLine.Saved(host), line(lines))
    }

    @Test
    fun closing_the_reader_forgets_every_line_and_wait() = runTest {
        val lines = lines()
        lines.chapterRead(work, chapter)
        lines.sent(other, server, host, WriteBackResult.Saved)
        lines.clear()
        advanceTimeBy(2_000)
        assertTrue(lines.lines.value.isEmpty())
    }

    // The server's row.

    private fun TestScope.rowLine(rows: RowLines): RowLine? {
        runCurrent()
        return rows.lines.value[server]
    }

    @Test
    fun row_lines_map_every_write_back_problem() {
        assertEquals(RowLine.Offline(3), row(pending = 3, problem = WriteBackProblem.OFFLINE).rowFacts())
        assertEquals(RowLine.Retrying(3), row(pending = 3, problem = WriteBackProblem.NO_ANSWER).rowFacts())
        assertEquals(RowLine.Retrying(3), row(pending = 3, problem = WriteBackProblem.SERVER_ERROR).rowFacts())
        assertEquals(RowLine.Failed(3, WhyNot.KEY), row(pending = 3, problem = WriteBackProblem.REFUSED_KEY).rowFacts())
        assertEquals(RowLine.Failed(3, WhyNot.PASSWORD), row(pending = 3, problem = WriteBackProblem.REFUSED_PASSWORD).rowFacts())
        assertEquals(RowLine.Failed(3, WhyNot.GONE), row(pending = 3, problem = WriteBackProblem.SERIES_GONE).rowFacts())
        assertEquals(RowLine.Failed(3, WhyNot.GAVE_UP), row(pending = 3, problem = WriteBackProblem.NO_ANSWER_FOR_A_DAY).rowFacts())
        assertEquals(RowLine.Off, row(problem = WriteBackProblem.NOT_SET_TO_RECEIVE).rowFacts())
        assertEquals(RowLine.Off, row(pending = 2, receives = false).rowFacts())
        assertEquals(RowLine.Saved(120), row(savedAt = 120).rowFacts())
        assertNull(row().rowFacts())
        assertNull(row(pending = 3, sending = true).rowFacts())
        assertEquals(WriteBackProblem.entries.toSet(), WriteBackProblem.entries.filter { row(pending = 1, problem = it).rowFacts() != null }.toSet())
    }

    @Test
    fun a_quick_send_on_the_row_never_shows_sending() = runTest {
        val rows = RowLines(backgroundScope)
        rows.update(listOf(row(savedAt = 100)))
        assertEquals(RowLine.Saved(100), rowLine(rows))
        rows.update(listOf(row(pending = 1, sending = true, savedAt = 100)))
        advanceTimeBy(500)
        assertEquals(RowLine.Saved(100), rowLine(rows))
        rows.update(listOf(row(savedAt = 160)))
        assertEquals(RowLine.Saved(160), rowLine(rows))
    }

    @Test
    fun a_slow_send_on_the_row_counts_its_chapters() = runTest {
        val rows = RowLines(backgroundScope)
        rows.update(listOf(row(pending = 3, sending = true)))
        advanceTimeBy(599)
        assertNull(rowLine(rows))
        advanceTimeBy(2)
        assertEquals(RowLine.Sending(3), rowLine(rows))
        rows.update(listOf(row(pending = 2, sending = true, savedAt = 10)))
        assertEquals(RowLine.Sending(2), rowLine(rows))
        rows.update(listOf(row(savedAt = 20)))
        advanceTimeBy(300)
        assertEquals(RowLine.Saved(20), rowLine(rows))
    }

    @Test
    fun try_again_on_the_row_dims_at_once_then_shows_the_answer_with_the_chevron() = runTest {
        val sent = Sent()
        val rows = RowLines(backgroundScope, sent)
        val failed = row(pending = 3, problem = WriteBackProblem.REFUSED_KEY, problemAt = 100)
        rows.update(listOf(failed))
        assertEquals(RowLine.Failed(3, WhyNot.KEY), rowLine(rows))
        rows.retry(server)
        assertEquals(listOf<Command>(retry), sent.commands)
        assertEquals(setOf(server), rows.retrying.value)
        rows.update(listOf(failed))
        advanceTimeBy(599)
        assertEquals(RowLine.Failed(3, WhyNot.KEY), rowLine(rows))
        advanceTimeBy(2)
        assertEquals(RowLine.Sending(3), rowLine(rows))
        rows.update(listOf(failed.copy(progressSending = true)))
        rows.update(listOf(row(savedAt = 300)))
        assertEquals(setOf(server), rows.retrying.value)
        advanceTimeBy(300)
        assertEquals(RowLine.Saved(300), rowLine(rows))
        assertTrue(rows.retrying.value.isEmpty())
    }

    @Test
    fun try_again_on_the_row_answered_quickly_shows_no_sending() = runTest {
        val rows = RowLines(backgroundScope)
        val waiting = row(pending = 3, problem = WriteBackProblem.NO_ANSWER, problemAt = 100)
        rows.update(listOf(waiting))
        rows.retry(server)
        rows.update(listOf(waiting.copy(progressSending = true)))
        advanceTimeBy(200)
        rows.update(listOf(row(pending = 3, problem = WriteBackProblem.NO_ANSWER, problemAt = 160)))
        assertEquals(RowLine.Retrying(3), rowLine(rows))
        assertTrue(rows.retrying.value.isEmpty())
    }

    @Test
    fun try_again_while_offline_answers_with_the_offline_line() = runTest {
        val rows = RowLines(backgroundScope)
        rows.update(listOf(row(pending = 3, problem = WriteBackProblem.NO_ANSWER, problemAt = 100)))
        rows.retry(server)
        rows.update(listOf(row(pending = 3, problem = WriteBackProblem.OFFLINE)))
        assertEquals(RowLine.Offline(3), rowLine(rows))
        assertTrue(rows.retrying.value.isEmpty())
    }

    @Test
    fun the_row_offline_then_back_online() = runTest {
        val rows = RowLines(backgroundScope)
        rows.update(listOf(row(pending = 3, problem = WriteBackProblem.OFFLINE)))
        assertEquals(RowLine.Offline(3), rowLine(rows))
        // Back online: due at once; the offline line stays through the silent wait.
        rows.update(listOf(row(pending = 3)))
        advanceTimeBy(300)
        assertEquals(RowLine.Offline(3), rowLine(rows))
        rows.update(listOf(row(pending = 3, sending = true)))
        advanceTimeBy(301)
        assertEquals(RowLine.Sending(3), rowLine(rows))
        rows.update(listOf(row(savedAt = 50)))
        advanceTimeBy(300)
        assertEquals(RowLine.Saved(50), rowLine(rows))
    }

    @Test
    fun a_server_set_not_to_receive_says_progress_stays_here() = runTest {
        val rows = RowLines(backgroundScope)
        rows.update(listOf(row(receives = false, problem = WriteBackProblem.NOT_SET_TO_RECEIVE)))
        assertEquals(RowLine.Off, rowLine(rows))
    }

    @Test
    fun a_removed_server_loses_its_line() = runTest {
        val rows = RowLines(backgroundScope)
        rows.update(listOf(row(savedAt = 10)))
        rows.update(emptyList())
        assertNull(rowLine(rows))
    }

    // The waiting rule itself.

    @Test
    fun waiting_times_follow_the_rule() {
        assertEquals(null to 400L, Waiting.times(0, 400))
        assertEquals(null to 600L, Waiting.times(0, 600))
        assertEquals(600L to 900L, Waiting.times(0, 650))
        assertEquals(600L to 1600L, Waiting.times(0, 1600))
    }
}
