// A strip plate's words sit in the middle of the part of the image on screen, kept inside the image.

package app.kaeri.reader

import org.junit.Assert.assertEquals
import org.junit.Test

class StripPlateTest {
    @Test
    fun the_words_follow_the_part_of_a_tall_image_on_screen() {
        // A 3000 px image whose top is 1600 px above a 800 px window: the words centre on 2000.
        assertEquals(1980, stripWordsTop(3000, 1600f..2400f, 40))
        // Only its top 20 px on screen: they keep inside the image.
        assertEquals(0, stripWordsTop(3000, 0f..20f, 40))
        // Its foot on screen: they stop at its bottom edge.
        assertEquals(2960, stripWordsTop(3000, 2990f..3000f, 40))
    }

    @Test
    fun an_image_wholly_on_screen_or_off_it_keeps_its_words_in_the_middle() {
        assertEquals(280, stripWordsTop(600, 0f..600f, 40))
        assertEquals(1480, stripWordsTop(3000, null, 40))
        assertEquals(0, stripWordsTop(20, null, 40))
    }
}
