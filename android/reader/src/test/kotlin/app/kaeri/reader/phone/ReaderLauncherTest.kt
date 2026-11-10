// The open reader's place kept through its activity being made anew, and only then.

package app.kaeri.reader.phone

import app.kaeri.engine.model.ReadPlace
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
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
}
