// A ready send whose files are gone by the time Share is pressed: no share sheet, the send says
// its files are gone, and it is not marked as shared. Files still there go to the share sheet.

package app.kaeri.ui.reading

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.ExportFile
import app.kaeri.store.AppStore
import app.kaeri.store.ExportJob
import app.kaeri.store.ExportModel
import app.kaeri.ui.AppActions
import app.kaeri.ui.LocalAppStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class SendGoneTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    @get:Rule
    val dir = TemporaryFolder()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    private fun file(name: String, there: Boolean): ExportFile {
        val path = File(dir.root, name)
        if (there) path.writeBytes(ByteArray(16))
        return ExportFile(path.path, name, 16uL, null)
    }

    @Test
    fun files_are_gone_when_one_is_missing_or_there_are_none() {
        val here = file("Paper Kite Club 1.cbz", there = true)
        val missing = file("Paper Kite Club 2.cbz", there = false)
        assertFalse(ExportShare.gone(listOf(here)))
        assertTrue(ExportShare.gone(listOf(here, missing)))
        assertTrue(ExportShare.gone(emptyList()))
        assertTrue("a folder is not a file", ExportShare.gone(listOf(ExportFile(dir.root.path, "shelf", 0uL, null))))
    }

    /** The row for a ready send, its Share handing over only files still there, as the app's does. */
    private fun row(files: List<ExportFile>): MutableList<List<ExportFile>> {
        val offered = mutableListOf<List<ExportFile>>()
        val actions = AppActions(
            shareFiles = { shown ->
                if (ExportShare.gone(shown)) {
                    false
                } else {
                    offered += shown
                    true
                }
            },
        )
        jobs(store.export).value = mapOf(7uL to ExportJob.Ready(7uL, "work-1", "eink-7", listOf("1", "2"), files))
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) { SendStatusRows(null, actions) }
            }
        }
        compose.waitForIdle()
        return offered
    }

    @Test
    fun share_with_a_file_gone_says_so_and_is_not_marked_shared() {
        val offered = row(listOf(file("Paper Kite Club 1.cbz", there = true), file("Paper Kite Club 2.cbz", there = false)))
        compose.onNodeWithText("Share").performClick()
        compose.waitForIdle()
        assertEquals("no share sheet", 0, offered.size)
        compose.onNodeWithText("The files are gone. Send them again.").assertExists()
        compose.onNodeWithText("Share").assertDoesNotExist()
        assertFalse(store.export.wasShared(7uL))
    }

    @Test
    fun share_with_every_file_there_hands_them_over_and_is_marked_shared() {
        val offered = row(listOf(file("Paper Kite Club 1.cbz", there = true)))
        compose.onNodeWithText("Share").performClick()
        compose.waitForIdle()
        assertEquals(1, offered.size)
        compose.onNodeWithText("The files are gone. Send them again.").assertDoesNotExist()
        assertTrue(store.export.wasShared(7uL))
    }

    @Suppress("UNCHECKED_CAST")
    private fun jobs(export: ExportModel): MutableStateFlow<Map<ULong, ExportJob>> =
        ExportModel::class.java.getDeclaredField("_jobs").apply { isAccessible = true }.get(export) as MutableStateFlow<Map<ULong, ExportJob>>
}
