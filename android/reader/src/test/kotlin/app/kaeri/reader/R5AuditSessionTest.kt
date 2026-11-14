// Round 5 audit: the session's half of closing, checked against what the engine's protocol offers.

package app.kaeri.reader

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import app.kaeri.reader.ReaderFixtures.WORK
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class R5AuditSessionTest {
    private val sent = mutableListOf<ReaderCommand>()
    private val session = ReaderSession { command -> sent += (command as Command.Reader).v1 }

    private fun opened() {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(6)))
    }

    /**
     * Back while the chapter still opens: the engine drops an open still waiting on `Close(0)`
     * (session.rs, ReaderCommand::Close). The session sends nothing, so the engine opens the
     * chapter anyway, fetches its pages and keeps the next chapters ready for a reader that is gone.
     */
    @Test
    fun closing_while_the_chapter_opens_drops_the_open_waiting() {
        session.open(WORK, ReaderFixtures.surface)
        sent.clear()
        session.close()
        assertTrue("sent: $sent", sent.contains(ReaderCommand.Close(0uL)))
    }

    /**
     * A chapter asked for from the one on screen fails: the state is made anew around the failure
     * and forgets the session, which the engine still holds open. Closing the reader then sends no
     * Close, so the engine never flushes the session: progress held for the close (Send progress
     * after: Close) waits for the next open in this process, and is lost if the process goes first.
     */
    @Test
    fun closing_after_another_chapter_failed_closes_the_engine_session() {
        opened()
        session.openChapter("local:lantern-stair/13")
        session.apply(Event.Reading(ReadingEvent.Failed(ReadingAbout.READER, ReadingFailure.Offline)))
        sent.clear()
        session.close()
        assertTrue("sent: $sent", sent.contains(ReaderCommand.Close(SESSION)))
    }

    /**
     * Back while the chapter opens, with the engine's Opened already on its way: Close(0) leaves an
     * open session alone, so the session that lands after the close is closed by its number.
     */
    @Test
    fun an_open_landing_after_the_close_is_closed_too() {
        session.open(WORK, ReaderFixtures.surface)
        session.close()
        sent.clear()
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u)))
        assertEquals(listOf<ReaderCommand>(ReaderCommand.Close(SESSION)), sent)
        // Nothing of the chapter shows on a reader that is gone.
        assertTrue(session.state.value.view == null)
    }

    /** A chapter that failed offline at the first open leaves no session: the close drops nothing more. */
    @Test
    fun closing_after_the_first_open_failed_sends_the_unnamed_close() {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(Event.Reading(ReadingEvent.Failed(ReadingAbout.READER, ReadingFailure.Offline)))
        sent.clear()
        session.close()
        assertEquals(listOf<ReaderCommand>(ReaderCommand.Close(0uL)), sent)
    }
}
