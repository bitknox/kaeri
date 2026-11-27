package app.kaeri.player.ui

import app.kaeri.player.PlayerModel.Corner
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CornerShownTest {
    @Test
    fun skipHidesUnderTheControls() {
        assertTrue(cornerShown(Corner.Skip, chromeVisible = false, assisted = false))
        assertFalse(cornerShown(Corner.Skip, chromeVisible = true, assisted = false))
    }

    @Test
    fun skipStaysWithTheControlsWhileAScreenReaderRuns() {
        // The controls never hide while assisted, so the card is the only way to skip.
        assertTrue(cornerShown(Corner.Skip, chromeVisible = true, assisted = true))
    }

    @Test
    fun otherCardsStayWithTheControls() {
        assertTrue(cornerShown(Corner.UpNext, chromeVisible = true, assisted = false))
        assertTrue(cornerShown(Corner.Undo, chromeVisible = true, assisted = false))
        assertFalse(cornerShown(null, chromeVisible = false, assisted = true))
    }
}
