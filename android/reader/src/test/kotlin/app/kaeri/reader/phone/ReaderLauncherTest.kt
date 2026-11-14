// The open reader's place kept through its activity being made anew, and only then; made anew
// for a change of font size or language, the open session itself is taken on, once.

package app.kaeri.reader.phone

import app.kaeri.engine.model.ReadPlace
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test

class ReaderLauncherTest {
    private val request = ReaderRequest(work = "local:lantern-stair", chapter = "local:lantern-stair/12", from = "cover")
    private val place = ReadPlace("local:lantern-stair", "local:lantern-stair/14", 9u, null, null)

    @After
    fun closed() = ReaderLauncher.close()

    @Test
    fun a_reader_made_anew_opens_where_the_last_one_was() {
        ReaderLauncher.open(request)
        ReaderLauncher.keep(request, place)
        assertEquals(place, ReaderLauncher.keptPlace(request))
    }

    @Test
    fun a_closed_reader_keeps_nothing() {
        ReaderLauncher.open(request)
        ReaderLauncher.close()
        ReaderLauncher.keep(request, place)
        assertNull(ReaderLauncher.keptPlace(request))
    }

    @Test
    fun a_new_open_starts_where_it_asks() {
        ReaderLauncher.open(request)
        ReaderLauncher.keep(request, place)
        val other = request.copy(chapter = "local:lantern-stair/2")
        ReaderLauncher.open(other)
        assertNull(ReaderLauncher.keptPlace(other))
        ReaderLauncher.open(request)
        assertNull(ReaderLauncher.keptPlace(request))
    }

    @Test
    fun the_open_session_is_taken_on_once_by_the_next_reader() {
        ReaderLauncher.open(request)
        val session = ReaderSession {}
        var released = 0
        ReaderLauncher.retain(request, session, PageImages()) { released++ }
        assertNull(ReaderLauncher.takeRetained(request.copy(chapter = "local:lantern-stair/2")))
        assertSame(session, ReaderLauncher.takeRetained(request)?.session)
        assertNull(ReaderLauncher.takeRetained(request))
        assertEquals(0, released)
    }

    @Test
    fun a_session_no_reader_takes_on_is_closed() {
        ReaderLauncher.open(request)
        var released = 0
        ReaderLauncher.retain(request, ReaderSession {}, PageImages()) { released++ }
        ReaderLauncher.close()
        assertEquals(1, released)
        // Retained after the reader closed, it is closed at once.
        ReaderLauncher.retain(request, ReaderSession {}, PageImages()) { released++ }
        assertEquals(2, released)
    }
}
