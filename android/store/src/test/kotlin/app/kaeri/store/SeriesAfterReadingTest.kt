package app.kaeri.store

import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReadPlace
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderView
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReadingEvent
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The series page under the reader asks for itself again once a chapter is read or the reader
 * closes: the engine sends nothing for either, and the main button would go on offering the
 * chapter just read.
 */
class SeriesAfterReadingTest {
    private val sent = mutableListOf<Command>()
    private val model = ReadingModel { sent += it }

    private val view = ReaderView(
        session = 1uL,
        work = WORK,
        title = "The Lantern Ferry",
        medium = Medium.MANGA,
        chapter = ChapterRow(id = "1", label = "1", state = ChapterReadState.Unread),
        mode = ReaderMode.PAGED,
        direction = ReadDirection.RIGHT_TO_LEFT,
        start = ReadPlace(work = WORK, chapter = "1"),
    )

    private fun asks() = sent.count { it is Command.Reading && it.v1 == ReadingCommand.LoadSeries(WORK) }

    private fun pageOpen() = model.observe(Event.Reading(ReadingEvent.Chapters(WORK, null, emptyList(), null)))

    @Test
    fun a_chapter_read_asks_for_the_series_again() {
        pageOpen()
        model.observe(Event.Reader(ReaderEvent.Opened(view)))
        assertEquals(0, asks())
        model.observe(Event.Reader(ReaderEvent.ChapterRead(WORK, "1", emptyList())))
        assertEquals(1, asks())
    }

    @Test
    fun closing_the_reader_asks_for_the_series_again_once() {
        pageOpen()
        model.observe(Event.Reader(ReaderEvent.Opened(view)))
        model.observe(Event.Reader(ReaderEvent.Closed(1uL)))
        assertEquals(1, asks())
        model.observe(Event.Reader(ReaderEvent.Closed(1uL)))
        assertEquals(1, asks())
    }

    @Test
    fun a_book_with_no_page_open_asks_for_nothing() {
        model.observe(Event.Reader(ReaderEvent.Opened(view)))
        model.observe(Event.Reader(ReaderEvent.Closed(1uL)))
        assertEquals(0, asks())
    }

    private companion object {
        const val WORK = "a:900301"
    }
}
