package app.kaeri.ui.reading

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The series button puts "chapter" before a number, never before a chapter's own title. */
class ChapterLabelTest {
    @Test
    fun a_number_takes_the_word_chapter() {
        assertTrue(numbered("12"))
        assertTrue(numbered("10.5"))
        assertTrue(numbered("12-13"))
    }

    @Test
    fun a_title_goes_by_itself() {
        assertFalse(numbered("Oneshot"))
        assertFalse(numbered("The Lantern Festival"))
        assertFalse(numbered("1st Anniversary Extra"))
        assertFalse(numbered(""))
    }
}
