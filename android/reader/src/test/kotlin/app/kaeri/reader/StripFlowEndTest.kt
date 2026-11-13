// A strip ends at the join on the end page when it cannot flow on: offline with the next chapter
// not on the device, as the engine's end page says, or when the next chapter failed to open for
// any reason. The chapter read stays above it.

package app.kaeri.reader

import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.test.core.app.ApplicationProvider
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.NoNext
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.i18n.Strings
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class StripFlowEndTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val dir = kotlin.io.path.createTempDirectory("strip-flow-end").toFile()
    private val next = ReaderFixtures.view().next!!

    private fun strip(state: ReaderState): LazyListState {
        val list = LazyListState()
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = ReaderSession {}, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> }, stripList = list)
            }
        }
        compose.waitForIdle()
        // Down to the join, under the last image.
        compose.runOnIdle { runBlocking { list.scrollToItem(3) } }
        compose.waitForIdle()
        return list
    }

    private fun base() = TapZonesScenes.state(dir, mode = ReaderMode.STRIP)

    @Test
    fun offline_with_the_next_chapter_not_on_the_device_the_strip_ends_on_the_end_page() {
        val list = strip(base().copy(end = EndPageView(next = null, whyNone = NoNext.OFFLINE)))
        assertEquals("end", list.layoutInfo.visibleItemsInfo.last().key)
        compose.onNodeWithText(next.title!!).assertDoesNotExist()
    }

    @Test
    fun offline_before_the_engines_end_page_the_strip_shows_no_divider_into_the_chapter_after() {
        val list = strip(base().copy(offline = true))
        assertEquals("end", list.layoutInfo.visibleItemsInfo.last().key)
        compose.onNodeWithText(next.title!!).assertDoesNotExist()
    }

    private fun failedFlow(failure: ReadingFailure) {
        val list = strip(base().copy(flowFailed = FlowFailure(next.id, next, failure)))
        assertEquals("end", list.layoutInfo.visibleItemsInfo.last().key)
        // The chapter read is still the strip's: its three images and the end page.
        assertEquals(4, list.layoutInfo.totalItemsCount)
        compose.onNodeWithText(next.title!!).assertExists()
        compose.onNodeWithText(failure.text(context)).assertExists()
        compose.onNodeWithText(Strings.readerTryAgain(context)).assertExists()
    }

    @Test
    fun a_next_chapter_that_failed_offline_is_named_at_the_join_with_try_again() = failedFlow(ReadingFailure.Offline)

    @Test
    fun a_next_chapter_whose_source_failed_is_named_at_the_join_with_try_again() =
        failedFlow(ReadingFailure.SourceDown("Lantern Archive"))
}
