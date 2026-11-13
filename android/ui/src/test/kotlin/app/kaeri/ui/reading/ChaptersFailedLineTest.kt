// The line under a series page's failed source says what still opens once, offline too.

package app.kaeri.ui.reading

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.i18n.Strings
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class ChaptersFailedLineTest {
    private val context: Context = ApplicationProvider.getApplicationContext()

    @Test
    fun offline_with_chapters_downloaded_says_it_in_one_sentence() {
        val line = chaptersFailedLine(context, ReadingFailure.Offline, 4)
        assertEquals(Strings.readSeriesOfflineDownloadedOpen(context, 4), line)
        assertEquals(1, Regex("still open").findAll(line).count())
    }

    @Test
    fun offline_with_none_downloaded_keeps_the_offline_words() {
        assertEquals(failureWords(context, ReadingFailure.Offline), chaptersFailedLine(context, ReadingFailure.Offline, 0))
    }
}
