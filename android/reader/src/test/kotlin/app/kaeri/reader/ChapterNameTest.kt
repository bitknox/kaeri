// A chapter with no number goes by its own name: a book in one file is named for the book, so the
// reader says its title, never "Chapter <title>". Numbered chapters keep their words.

package app.kaeri.reader

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.reader.phone.chapterName
import app.kaeri.reader.phone.chapterWords
import app.kaeri.reader.phone.endFacts
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ChapterNameTest {
    private val context: Context = ApplicationProvider.getApplicationContext()

    private val book = ChapterRow(id = "book", label = "Rooftop Lantern", state = ChapterReadState.Unread, number = null)
    private val twelve = ChapterRow(id = "12", label = "Chapter 12", state = ChapterReadState.Unread, number = "12", title = "The Long Stair")

    @Test
    fun a_chapter_with_no_number_is_named_by_its_own_name() {
        assertEquals("Rooftop Lantern", chapterName(context, book))
        assertEquals("Rooftop Lantern", chapterWords(context, book))
        assertEquals("", endFacts(context, book.copy(title = "Rooftop Lantern"), null))
    }

    @Test
    fun a_numbered_chapter_keeps_its_words() {
        assertEquals("Chapter 12", chapterName(context, twelve))
        assertEquals("Chapter 12 · The Long Stair", chapterWords(context, twelve))
        assertEquals("Chapter 12", endFacts(context, twelve, null))
    }
}
