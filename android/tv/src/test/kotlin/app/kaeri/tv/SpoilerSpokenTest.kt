package app.kaeri.tv

import app.kaeri.tv.ui.spokenBlock
import org.junit.Assert.assertEquals
import org.junit.Test

/** What TalkBack gets of a TV comment with an unopened spoiler: the word, never the hidden text. */
class SpoilerSpokenTest {
    @Test
    fun hidden_spans_read_as_the_word() {
        val spans = listOf(false to "The ending: ", true to "the lighthouse keeper was ", true to "the narrator", false to ". Loved it.")
        assertEquals("The ending: Spoiler. Loved it.", spokenBlock(spans, "Spoiler"))
    }

    @Test
    fun a_spoiler_between_words_keeps_them_apart() {
        assertEquals("Before Spoiler after", spokenBlock(listOf(false to "Before", true to "secret", false to "after"), "Spoiler"))
        assertEquals("Spoiler", spokenBlock(listOf(true to "all of it"), "Spoiler"))
    }

    @Test
    fun plain_text_is_unchanged() {
        assertEquals("Nothing hidden here.", spokenBlock(listOf(false to "Nothing ", false to "hidden here."), "Spoiler"))
    }
}
