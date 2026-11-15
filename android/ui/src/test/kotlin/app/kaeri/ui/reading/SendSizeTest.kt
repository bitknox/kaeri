// The Send sheet's Size: the engine's figure for every chosen chapter. It says 0 for a size it
// doesn't know, which the sheet leaves unsaid, rather than "Working it out" for good.

package app.kaeri.ui.reading

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import app.kaeri.engine.model.DownloadEstimate
import app.kaeri.engine.model.DownloadPick
import app.kaeri.i18n.Strings
import app.kaeri.store.Wording
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class SendSizeTest {
    private val context: Context = ApplicationProvider.getApplicationContext()

    private fun estimate(bytes: Long, all: Long) = DownloadEstimate(
        work = "a:1",
        pick = DownloadPick.Chapters(listOf("c1", "c2")),
        chapters = 1u,
        bytes = bytes.toULong(),
        line = emptyList(),
        allBytes = all.toULong(),
    )

    @Test
    fun an_unknown_size_is_left_unsaid() {
        assertNull(sendBytes(estimate(0, 0)))
        assertNull(sendSizeWords(context, estimate(0, 0), waited = true))
    }

    @Test
    fun the_size_is_every_chosen_chapter_and_waits_with_working_it_out() {
        assertEquals(64_000_000L, sendBytes(estimate(30_000_000, 64_000_000)))
        assertEquals(
            Strings.sendSizeAbout(context, Wording.bytes(context, 64_000_000L)),
            sendSizeWords(context, estimate(30_000_000, 64_000_000), waited = false),
        )
        assertEquals(Strings.sendSizeWorking(context), sendSizeWords(context, null, waited = true))
        assertNull(sendSizeWords(context, null, waited = false))
    }
}
