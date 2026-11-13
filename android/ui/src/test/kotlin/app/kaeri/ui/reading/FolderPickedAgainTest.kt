// A folder picked again while it is a source: the form says it is already there, with Done, and
// nothing that closes the form (Done, Close, Change, Choose another folder) takes it back out with
// its series. A folder picked for the first time is still taken back when the form is closed.

package app.kaeri.ui.reading

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.BookSourceCommand
import app.kaeri.engine.model.CameIn
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.FolderReport
import app.kaeri.engine.model.NothingCameIn
import app.kaeri.store.Adding
import app.kaeri.store.AppStore
import app.kaeri.ui.LocalAppStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class FolderPickedAgainTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    // No engine: what the form sends waits in the store's queue, for the test to read.
    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    private fun report(series: Int, alreadyAdded: Boolean) = FolderReport(
        id = "shelf",
        name = "Shelf",
        path = null,
        series = series.toUInt(),
        books = series.toUInt(),
        files = series.toUInt(),
        matched = 0u,
        unsupported = emptyList(),
        locked = emptyList(),
        listed = emptyList(),
        alreadyAdded = alreadyAdded,
    )

    private fun removed(): List<String> = store.waiting.mapNotNull {
        ((it as? Command.BookSources)?.v1 as? BookSourceCommand.RemoveFolder)?.id
    }

    private fun show(answer: CameIn) {
        store.cameIn.stage(Adding.FOLDER, answer, folder = "Shelf")
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) { BookSourceDialogs() }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun a_source_picked_again_says_so_and_done_keeps_it() {
        show(CameIn.Folder(report(12, alreadyAdded = true)))
        compose.onNodeWithText("Shelf is already in your sources").assertExists()
        compose.onNodeWithText("Done").performClick()
        compose.waitForIdle()
        assertEquals(emptyList<String>(), removed())
    }

    @Test
    fun close_and_change_keep_a_source_picked_again_with_nothing_readable() {
        show(CameIn.Empty(NothingCameIn.EmptyFolder(report(0, alreadyAdded = true))))
        compose.onNodeWithText("Change").performClick()
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Close").performClick()
        compose.waitForIdle()
        assertEquals(emptyList<String>(), removed())
    }

    @Test
    fun choose_another_keeps_a_source_that_cannot_be_read_now() {
        show(CameIn.Empty(NothingCameIn.FolderUnreadable(report(0, alreadyAdded = true))))
        compose.onNodeWithText("Couldn't read Shelf").assertExists()
        compose.onNodeWithText("Choose another folder").performClick()
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Close").performClick()
        compose.waitForIdle()
        assertEquals(emptyList<String>(), removed())
    }

    @Test
    fun a_folder_picked_for_the_first_time_is_taken_back_on_close() {
        show(CameIn.Empty(NothingCameIn.EmptyFolder(report(0, alreadyAdded = false))))
        compose.onNodeWithContentDescription("Close").performClick()
        compose.waitForIdle()
        assertEquals(listOf("shelf"), removed())
    }
}
