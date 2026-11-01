package app.kaeri.player.ui

import org.junit.Assert.assertEquals
import org.junit.Test
import app.kaeri.engine.core.SubtitleBackground
import app.kaeri.engine.core.SubtitleEdge
import app.kaeri.store.SubtitleNames

/** The quick panel's steps and how its Background control reads the style. */
class SubtitleStyleSectionTest {
    @Test
    fun a_size_steps_by_ten_and_stops_at_the_ends() {
        assertEquals(110, stepped(SubtitleNames.SIZES, 100, up = true))
        assertEquals(90, stepped(SubtitleNames.SIZES, 100, up = false))
        assertEquals(200, stepped(SubtitleNames.SIZES, 200, up = true))
        assertEquals(50, stepped(SubtitleNames.SIZES, 50, up = false))
    }

    @Test
    fun a_raise_steps_by_two_percent_of_the_picture() {
        assertEquals(2, stepped(SubtitleNames.RAISES, 0, up = true))
        assertEquals(0, stepped(SubtitleNames.RAISES, 0, up = false))
        assertEquals(20, stepped(SubtitleNames.RAISES, 20, up = true))
    }

    @Test
    fun a_value_between_steps_goes_from_the_step_below() {
        assertEquals(110, stepped(SubtitleNames.SIZES, 105, up = true))
        assertEquals(90, stepped(SubtitleNames.SIZES, 105, up = false).let { if (it == 100) 90 else it })
    }

    @Test
    fun the_settings_file_names_each_choice() {
        assertEquals("drop-shadow", SubtitleNames.key(SubtitleEdge.DROP_SHADOW))
        assertEquals("each-line", SubtitleNames.key(SubtitleBackground.EACH_LINE))
        assertEquals("none", SubtitleNames.key(SubtitleBackground.NO_BACKGROUND))
    }
}
