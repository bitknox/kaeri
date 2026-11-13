package app.kaeri.app

import androidx.activity.ComponentActivity
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.input.InputModeManager
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.requestFocus
import androidx.compose.ui.unit.dp
import app.kaeri.eink.EinkModel
import app.kaeri.reader.phone.ReaderLauncher
import app.kaeri.reader.phone.ReaderRequest
import app.kaeri.store.AppStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The series page stays composed under the open reader. While the reader is open nothing on that
 * page takes the focus or reaches a screen reader: Tab and Enter stay in the reader, and the
 * control that had the focus before the reader opened has it again once it closes.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ReaderUnderTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main), artworkWithoutEngine = { null })
    private var clicks = 0
    /** Whether a control on the page under the reader took the focus while the reader was open. */
    private var reached = false
    private var readerOpen = false
    private var modes: InputModeManager? = null

    @After
    fun close() {
        ReaderLauncher.close()
    }

    /** A series page with its Read and More buttons, under the reader host. */
    @OptIn(ExperimentalComposeUiApi::class)
    private fun seriesPage() {
        compose.setContent {
            modes = LocalInputModeManager.current
            ReaderHost(store, EinkModel {}) {
                val focus = Modifier.onFocusChanged { if (it.isFocused && readerOpen) reached = true }
                Column {
                    Box(Modifier.size(120.dp, 48.dp).testTag("read").then(focus).clickable {})
                    Box(Modifier.size(48.dp).testTag("more").then(focus).clickable { clicks++ })
                }
            }
        }
        compose.waitForIdle()
        compose.runOnIdle { modes!!.requestInputMode(InputMode.Keyboard) }
    }

    private fun open() {
        compose.runOnIdle {
            readerOpen = true
            ReaderLauncher.open(ReaderRequest("a:900301", chapter = "c:13"))
        }
        compose.waitForIdle()
    }

    private fun shut() {
        compose.runOnIdle {
            readerOpen = false
            ReaderLauncher.close()
        }
        compose.waitForIdle()
    }

    private fun keys(vararg keys: Key) = keys.forEach { key ->
        compose.onNode(isFocused()).performKeyInput { pressKey(key) }
        compose.waitForIdle()
    }

    @Test
    fun tab_and_enter_never_reach_the_page_under_the_phone_reader() {
        seriesPage()
        open()
        keys(Key.Tab, Key.Tab, Key.Enter)
        assertEquals(false, reached)
        assertEquals(0, clicks)
        // The reader still has the keys: Escape hides its controls, and the next one closes it.
        keys(Key.Escape)
        if (ReaderLauncher.request.value != null) keys(Key.Escape)
        assertEquals(null, ReaderLauncher.request.value)
        shut()
        // Nothing had the focus before the reader opened, so nothing takes it as it closes.
        compose.onNodeWithTag("more").assertIsNotFocused()
        compose.onNodeWithTag("read").assertIsNotFocused()
    }

    @Test
    fun a_screen_reader_finds_the_page_again_only_once_the_reader_closes() {
        seriesPage()
        open()
        compose.onNodeWithTag("read", useUnmergedTree = true).assertDoesNotExist()
        shut()
        compose.onNodeWithTag("read").assertExists()
    }

    @Test
    fun the_focus_goes_back_where_it_was_when_the_reader_closes() {
        seriesPage()
        compose.onNodeWithTag("more").requestFocus().assertIsFocused()
        open()
        compose.onNodeWithTag("more", useUnmergedTree = true).assertDoesNotExist()
        keys(Key.Tab, Key.Enter)
        assertEquals(false, reached)
        assertEquals(0, clicks)
        shut()
        compose.onNodeWithTag("more").assertIsFocused()
    }
}
