// A chapter that would not open while the reader never learned which chapter it was: the heading
// says so in words, never "Couldn't open chapter " with nothing after it. It uses only what the
// reader had before that heading changed, so it builds on both sides of the change.

package app.kaeri.reader.phone

import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.test.core.app.ApplicationProvider
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.i18n.Strings
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderFixtures
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ChapterFailedHeadingTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val context: Context = ApplicationProvider.getApplicationContext()

    private fun show(state: ReaderState) {
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = ReaderSession {}, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> })
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun a_chapter_never_named_has_no_heading_with_an_empty_number() {
        show(ReaderState(failure = ReadingFailure.Offline))
        compose.onAllNodesWithText(Strings.readerFailedChapter(context, "")).assertCountEquals(0)
        compose.onAllNodesWithText(Strings.readerFailedChapter(context, "").trimEnd()).assertCountEquals(0)
    }

    @Test
    fun a_chapter_the_reader_knows_is_named() {
        show(ReaderState(failure = ReadingFailure.Offline, openingChapter = ReaderFixtures.chapter(number = "12")))
        compose.onNodeWithText(Strings.readerFailedChapter(context, "12")).assertExists()
    }
}
