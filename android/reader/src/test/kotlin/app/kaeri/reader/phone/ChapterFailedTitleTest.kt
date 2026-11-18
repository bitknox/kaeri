// A chapter that would not open is named in the heading: by the number the screen that asked
// listed, or, when the reader never learned which chapter it was, by words without a number,
// never "Couldn't open chapter " with nothing after it.

package app.kaeri.reader.phone

import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.test.core.app.ApplicationProvider
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.i18n.Strings
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderFixtures
import app.kaeri.reader.ReaderSession
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ChapterFailedTitleTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val session = ReaderSession {}

    /** Opens the reader fresh, as an Updates row or a notification does, and fails before any view. */
    private fun failedOpen(row: app.kaeri.engine.model.ChapterRow?): String {
        session.open(ReaderFixtures.WORK, ReaderFixtures.surface, "local:lantern-stair/22", row = row)
        session.apply(Event.Reading(ReadingEvent.Failed(ReadingAbout.READER, ReadingFailure.Offline)))
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                val state = session.state.collectAsState().value
                PhoneReader(session = session, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> })
            }
        }
        compose.waitForIdle()
        return failedChapterTitle(context, session.state.value.openingChapter)
    }

    @Test
    fun a_chapter_the_screen_listed_is_named_and_stays_named_on_try_again() {
        val row = ReaderFixtures.chapter("local:lantern-stair/22", "22")
        val title = failedOpen(row)
        assertEquals(Strings.readerFailedChapter(context, "22"), title)
        compose.onNodeWithText(title).assertExists()

        session.retryOpen()
        session.apply(Event.Reading(ReadingEvent.Failed(ReadingAbout.READER, ReadingFailure.Offline)))
        compose.waitForIdle()
        compose.onNodeWithText(Strings.readerFailedChapter(context, "22")).assertExists()
    }

    @Test
    fun a_chapter_never_named_gets_words_without_a_number() {
        val title = failedOpen(null)
        assertEquals(Strings.readerFailedChapterPlain(context), title)
        assertFalse("no trailing space: '$title'", title.endsWith(" "))
        compose.onNodeWithText(title).assertExists()
    }

    @Test
    fun a_row_for_another_chapter_is_not_taken_for_the_one_asked() {
        val title = failedOpen(ReaderFixtures.chapter("local:lantern-stair/9", "9"))
        assertEquals(Strings.readerFailedChapterPlain(context), title)
    }
}
