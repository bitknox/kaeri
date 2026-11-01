package app.kaeri.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import app.kaeri.engine.ffi.FfiLinkSent
import app.kaeri.tv.ui.sentWords

/** Send to phone says what became of the link, a different thing for each outcome. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class SendToPhoneTest {
    private val context = RuntimeEnvironment.getApplication()

    @Test
    fun each_outcome_has_its_own_words() {
        val sent = sentWords(context, FfiLinkSent.Sent(1u))
        val held = sentWords(context, FfiLinkSent.Held)
        val refused = sentWords(context, FfiLinkSent.Refused)
        assertEquals("Sent. Your phone opens it now.", sent)
        assertEquals(3, setOf(sent, held, refused).size)
        assertNotEquals("", refused)
    }
}
