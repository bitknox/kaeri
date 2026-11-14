// A folder taken out of Your sources says so with Undo for as long as the engine keeps it, and the
// notice goes when the engine lets the folder go.

package app.kaeri.ui.reading

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.BookFolderRow
import app.kaeri.engine.model.BookFolderScan
import app.kaeri.engine.model.BookSourcesView
import app.kaeri.ui.LocalSnackbars
import app.kaeri.ui.NoticeSnackbar
import app.kaeri.ui.Snackbars
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class FolderRemovedNoticeTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private fun folder(removed: Boolean) = BookFolderRow(id = "f1", name = "Comics folder", scan = BookFolderScan.Done, removed = removed) // i18n-skip

    private var view by mutableStateOf<BookSourcesView?>(null)
    private val restored = mutableListOf<String>()

    private fun show() {
        compose.setContent {
            KaeriTheme {
                val scope = rememberCoroutineScope()
                val snackbars = remember { Snackbars(SnackbarHostState(), scope) }
                CompositionLocalProvider(LocalSnackbars provides snackbars) {
                    Column {
                        FolderRemovedNotice(view.folderRemoving()) { restored += it }
                        SnackbarHost(snackbars.host) { NoticeSnackbar(it) }
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun a_removed_folder_offers_undo() {
        view = BookSourcesView(folders = listOf(folder(removed = true)))
        show()
        compose.onNodeWithText("Comics folder removed").assertExists() // i18n-skip
        compose.onNodeWithText("Undo").performClick() // i18n-skip
        compose.waitForIdle()
        assertEquals(listOf("f1"), restored)
    }

    @Test
    fun the_notice_goes_with_the_folder() {
        view = BookSourcesView(folders = listOf(folder(removed = true)))
        show()
        compose.onNodeWithText("Comics folder removed").assertExists() // i18n-skip
        // The engine lets the folder go after its 8 s: Undo would do nothing from then on.
        view = BookSourcesView(folders = emptyList())
        compose.waitForIdle()
        compose.onAllNodesWithText("Comics folder removed").assertCountEquals(0) // i18n-skip
    }

    @Test
    fun a_folder_in_use_says_nothing() {
        view = BookSourcesView(folders = listOf(folder(removed = false)))
        assertNull(view.folderRemoving())
        show()
        compose.onAllNodesWithText("Undo").assertCountEquals(0) // i18n-skip
    }
}
