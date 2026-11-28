package app.kaeri.store

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.PlayerCommand
import app.kaeri.engine.model.PlayerReport
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

/** Play here from a TV's playhead: the seek goes once this play is staged and its file is open. */
class StartAtTest {
    private val loaded = Command.PlayerReport(1uL, PlayerReport.FileLoaded)

    @Test
    fun theSeekWaitsForThePlayToBeStagedAndOpen() {
        val at = StartAt(900_001u, "7", 754.0)
        assertNull("not staged yet", at.after(loaded))
        at.staged(900_001u, "7")
        assertNull(at.after(Command.PlayerReport(1uL, PlayerReport.Closed)))
        assertEquals(Command.Player(PlayerCommand.SeekTo(754.0)), at.after(loaded))
    }

    @Test
    fun anotherPlayStagedGivesThePositionUp() {
        val at = StartAt(900_001u, "7", 754.0)
        assertFalse(at.staged(900_001u, "8"))
        assertNull(at.after(loaded))
    }
}
