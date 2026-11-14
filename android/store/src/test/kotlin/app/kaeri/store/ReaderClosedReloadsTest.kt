package app.kaeri.store

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingHomeView
import org.junit.Assert.assertTrue
import org.junit.Assert.assertFalse
import org.junit.Test

/** Closing the reader: Read home's Continue is asked again, so it names where reading got to. */
class ReaderClosedReloadsTest {
    private val sent = mutableListOf<Command>()
    private val model = ReadingModel { sent += it }

    private fun asked() = sent.any { (it as? Command.Reading)?.v1 == ReadingCommand.LoadHome(false) }

    @Test
    fun a_shown_read_home_is_asked_again_when_the_reader_closes() {
        model.apply(ReadingEvent.Home(ReadingHomeView()))
        sent.clear()
        model.readerClosed("local:lantern-stair")
        assertTrue(asked())
    }

    @Test
    fun a_read_home_never_shown_is_left_unasked() {
        model.readerClosed("local:lantern-stair")
        assertFalse(asked())
    }
}
