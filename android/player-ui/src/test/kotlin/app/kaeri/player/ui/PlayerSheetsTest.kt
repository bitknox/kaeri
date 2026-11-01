// Where the player's sheets open: a list of many tracks shows the one in use.

package app.kaeri.player.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PlayerSheetsTest {
    private val twenty = (1L..20L).toList()

    @Test
    fun theTracksSheetOpensOnTheSubtitlesInUse() {
        // The title, the heading and Off come first: the last of twenty tracks is row 22.
        assertEquals(21, tracksOpeningRow(twenty, 20L))
        assertEquals(10, tracksOpeningRow(twenty, 9L))
    }

    @Test
    fun theTracksSheetOpensAtTheTopNearIt() {
        assertEquals(0, tracksOpeningRow(twenty, null))
        assertEquals(0, tracksOpeningRow(twenty, 1L))
        assertEquals(0, tracksOpeningRow(twenty, 99L))
        assertEquals(0, tracksOpeningRow(emptyList(), null))
    }

    @Test
    fun aPhoneHeldSidewaysGetsTheSideSheet() {
        assertTrue(usesSideSheet(915, 412))
        assertTrue(usesSideSheet(780, 360))
    }

    @Test
    fun portraitAndTabletsKeepTheBottomSheet() {
        assertFalse(usesSideSheet(412, 915))
        assertFalse(usesSideSheet(1280, 800))
    }
}
