package app.kaeri.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class SpoilerSpeechTest {
    @Test
    fun aClosedBarIsReadAsTheWordNotItsText() {
        val speech = SpoilerSpeech("Spoiler")
        speech.plain("In the end ")
        speech.bar()
        speech.plain(" and the tide turns.")
        assertEquals("In the end Spoiler and the tide turns.", speech.text)
        assertEquals(1, speech.closed)
    }

    @Test
    fun withEveryBarOpenTheTextReadsAsItIs() {
        val speech = SpoilerSpeech("Spoiler")
        speech.plain("The keeper of the lighthouse")
        speech.plain(" was the heir all along.")
        assertEquals(0, speech.closed)
        assertEquals("The keeper of the lighthouse was the heir all along.", speech.text)
    }
}
