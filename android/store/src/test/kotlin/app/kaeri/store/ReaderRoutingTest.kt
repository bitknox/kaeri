package app.kaeri.store

import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReadingEvent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

/** What the store hands an open reader session of the reading events. */
class ReaderRoutingTest {
    @Test
    fun a_reader_session_hears_the_chapters_and_the_versions() = runTest {
        val store = AppStore(this)
        val heard = mutableListOf<Event>()
        val stop = store.listenToReading { heard += it }
        val chapters = Event.Reading(ReadingEvent.Chapters(WORK, null, emptyList(), null))
        val versions = Event.Reading(ReadingEvent.Versions(WORK, "12", emptyList()))
        store.apply(chapters)
        store.apply(versions)
        assertEquals(listOf(chapters, versions), heard)
        stop()
        store.apply(chapters)
        assertEquals(2, heard.size)
    }

    private companion object {
        const val WORK = "a:900301"
    }
}
