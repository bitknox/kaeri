package app.kaeri.widgets

import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Android 11's widgets, which the launcher neither clips nor lets colour a progress bar. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class SystemStyleTest {
    @Test
    fun art_at_the_edge_rounds_itself_only_where_nothing_clips_it() {
        assertEquals(16.dp, SystemStyle.unclipped(30))
        assertEquals(0.dp, SystemStyle.unclipped(31))
        assertEquals(0.dp, SystemStyle.unclipped(35))
    }

    @Test
    fun the_drawn_bar_fills_up_to_the_progress_over_its_track() {
        val fill = 0xFFF2A64B.toInt()
        val track = 0xFF727892.toInt()
        val bar = WidgetArt.bar(0.25f, 400, 10, fill, track)
        assertEquals(fill, bar.getPixel(50, 5))
        assertEquals(track, bar.getPixel(200, 5))
        assertEquals(track, bar.getPixel(390, 5))
    }

    @Test
    fun art_rounded_at_the_top_only_keeps_its_bottom_corners_square() {
        val plate = WidgetArt.plate(0xFF222638.toInt(), 100, 60, 16f, topOnly = true)
        assertEquals(0, plate.getPixel(0, 0))
        assertEquals(0xFF222638.toInt(), plate.getPixel(0, 59))
        assertEquals(0xFF222638.toInt(), plate.getPixel(99, 59))
    }
}
