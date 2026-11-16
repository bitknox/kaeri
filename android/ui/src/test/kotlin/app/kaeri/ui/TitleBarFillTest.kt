package app.kaeri.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/** The title page's top bar fills with the scroll, and reverses with it. */
class TitleBarFillTest {
    private fun fill(index: Int, scrolled: Float, instant: Boolean = false) =
        titleBarFill(index, scrolled, start = 48f, span = 64f, instant = instant)

    @Test
    fun clear_while_the_head_is_below_the_bar() {
        assertEquals(0f, fill(0, 0f), 0f)
        assertEquals(0f, fill(0, 48f), 0f)
    }

    @Test
    fun fills_with_the_scroll_and_reverses_with_it() {
        assertEquals(0.25f, fill(0, 64f), 0.001f)
        assertEquals(0.5f, fill(0, 80f), 0.001f)
        assertEquals(1f, fill(0, 112f), 0f)
        assertEquals(1f, fill(0, 400f), 0f)
        // Back down: the same place, the same fill.
        assertEquals(0.25f, fill(0, 64f), 0.001f)
    }

    @Test
    fun solid_past_the_head() {
        assertEquals(1f, fill(1, 0f), 0f)
        assertEquals(1f, fill(7, 12f), 0f)
    }

    @Test
    fun switches_at_once_with_animations_removed() {
        assertEquals(0f, fill(0, 64f, instant = true), 0f)
        assertEquals(0f, fill(0, 79f, instant = true), 0f)
        assertEquals(1f, fill(0, 80f, instant = true), 0f)
        assertEquals(1f, fill(3, 0f, instant = true), 0f)
    }

    @Test
    fun the_name_comes_in_over_the_second_half() {
        assertEquals(0f, titleBarNameAlpha(0f), 0f)
        assertEquals(0f, titleBarNameAlpha(0.5f), 0f)
        assertEquals(0.5f, titleBarNameAlpha(0.75f), 0.001f)
        assertEquals(1f, titleBarNameAlpha(1f), 0f)
    }
}
