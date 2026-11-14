// The reader's keys with a control inside it reached by keyboard: Enter belongs to the reader while
// the reader holds the focus, and to the control while the control does; the arrows turn either way
// unless the controls take the D-pad while they show, when the arrows go into them and move between
// them and the page keys still turn.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.clickable
import android.view.KeyEvent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.input.InputModeManager
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.requestFocus
import androidx.compose.ui.unit.dp
import app.kaeri.engine.model.ReadDirection
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ReaderKeysTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val actions = mutableListOf<ReaderAction>()
    private var clicks = 0
    private var modes: InputModeManager? = null

    private fun reader() {
        compose.setContent {
            modes = LocalInputModeManager.current
            Box(Modifier.fillMaxSize().testTag("reader").readerKeys(ReaderKeyRules(ReadDirection.LEFT_TO_RIGHT), { actions += it })) {
                Box(Modifier.size(48.dp).testTag("settings").clickable { clicks++ })
            }
        }
        compose.waitForIdle()
    }

    private fun readerWithControls(shown: Boolean) {
        compose.setContent {
            modes = LocalInputModeManager.current
            val first = remember { FocusRequester() }
            Box(Modifier.fillMaxSize().testTag("reader").readerKeys(ReaderKeyRules(ReadDirection.LEFT_TO_RIGHT), { actions += it }, controls = ReaderControlKeys(shown) { first.requestFocus() })) {
                Row {
                    Box(Modifier.size(48.dp).testTag("first").focusRequester(first).clickable { clicks++ })
                    Box(Modifier.size(48.dp).testTag("second").clickable { clicks++ })
                }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun the_routes_of_the_keys() {
        val arrows = listOf(KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_DOWN, KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_RIGHT)
        val turns = listOf(KeyEvent.KEYCODE_PAGE_DOWN, KeyEvent.KEYCODE_PAGE_UP, KeyEvent.KEYCODE_VOLUME_UP, KeyEvent.KEYCODE_VOLUME_DOWN, KeyEvent.KEYCODE_SPACE)
        for (holds in listOf(true, false)) {
            // Controls hidden, or a reader whose controls never take the D-pad: the arrows turn.
            for (key in arrows + turns) assertEquals(KeyRoute.READER, keyRoute(key, holds, controlsTakeArrows = false))
            // The page keys and Space turn whatever shows.
            for (key in turns) assertEquals(KeyRoute.READER, keyRoute(key, holds, controlsTakeArrows = true))
            for (shown in listOf(true, false)) {
                // Enter is a focused control's, and the reader's own while it holds the focus.
                assertEquals(if (holds) KeyRoute.READER else KeyRoute.CONTROL, keyRoute(KeyEvent.KEYCODE_ENTER, holds, shown))
                assertEquals(if (holds) KeyRoute.READER else KeyRoute.CONTROL, keyRoute(KeyEvent.KEYCODE_DPAD_CENTER, holds, shown))
                assertEquals(KeyRoute.READER, keyRoute(KeyEvent.KEYCODE_ESCAPE, holds, shown))
            }
        }
        // While the controls show, an arrow goes into them from the reader and moves among them after.
        for (key in arrows) {
            assertEquals(KeyRoute.INTO_CONTROLS, keyRoute(key, readerHolds = true, controlsTakeArrows = true))
            assertEquals(KeyRoute.MOVE_FOCUS, keyRoute(key, readerHolds = false, controlsTakeArrows = true))
        }
        // Tab goes into them from the reader too, and is the focused control's own after.
        assertEquals(KeyRoute.INTO_CONTROLS, keyRoute(KeyEvent.KEYCODE_TAB, readerHolds = true, controlsTakeArrows = true))
        assertEquals(KeyRoute.READER, keyRoute(KeyEvent.KEYCODE_TAB, readerHolds = false, controlsTakeArrows = true))
        assertEquals(KeyRoute.READER, keyRoute(KeyEvent.KEYCODE_TAB, readerHolds = true, controlsTakeArrows = false))
    }

    @OptIn(ExperimentalComposeUiApi::class)
    @Test
    fun while_the_controls_show_the_arrows_move_the_focus_and_the_page_keys_still_turn() {
        readerWithControls(shown = true)
        compose.runOnIdle { modes!!.requestInputMode(InputMode.Keyboard) }
        val reader = compose.onNodeWithTag("reader").assertIsFocused()
        // From the reader, an arrow goes to the first control and turns nothing.
        reader.performKeyInput { pressKey(Key.DirectionDown) }
        compose.waitForIdle()
        compose.onNodeWithTag("first").assertIsFocused()
        compose.onNodeWithTag("first").performKeyInput { pressKey(Key.DirectionRight) }
        compose.waitForIdle()
        compose.onNodeWithTag("second").assertIsFocused()
        compose.onNodeWithTag("first").assertIsNotFocused()
        // Past the last control the focus stays, and still nothing turns.
        compose.onNodeWithTag("second").performKeyInput { pressKey(Key.DirectionRight) }
        compose.waitForIdle()
        compose.onNodeWithTag("second").assertIsFocused()
        assertEquals(emptyList<ReaderAction>(), actions)
        // The page keys still turn, and Enter is the control's.
        compose.onNodeWithTag("second").performKeyInput { pressKey(Key.PageDown) }
        compose.onNodeWithTag("second").performKeyInput { pressKey(Key.Enter) }
        compose.waitForIdle()
        assertEquals(listOf(ReaderAction.FORWARD), actions)
        assertEquals(1, clicks)
    }

    @OptIn(ExperimentalComposeUiApi::class)
    @Test
    fun with_the_controls_hidden_the_arrows_turn() {
        readerWithControls(shown = false)
        compose.runOnIdle { modes!!.requestInputMode(InputMode.Keyboard) }
        compose.onNodeWithTag("reader").assertIsFocused().performKeyInput { pressKey(Key.DirectionDown) }
        compose.waitForIdle()
        compose.onNodeWithTag("reader").assertIsFocused()
        assertEquals(listOf(ReaderAction.FORWARD), actions)
    }

    @Test
    fun enter_shows_the_controls_while_the_reader_holds_the_focus() {
        reader()
        compose.onNodeWithTag("reader").assertIsFocused().performKeyInput { pressKey(Key.Enter) }
        assertEquals(listOf(ReaderAction.CONTROLS), actions)
        assertEquals(0, clicks)
    }

    @OptIn(ExperimentalComposeUiApi::class)
    @Test
    fun enter_acts_on_a_focused_control_and_the_arrows_still_turn() {
        reader()
        compose.runOnIdle { modes!!.requestInputMode(InputMode.Keyboard) }
        val settings = compose.onNodeWithTag("settings").requestFocus().assertIsFocused()
        settings.performKeyInput { pressKey(Key.Enter) }
        compose.waitForIdle()
        assertEquals(1, clicks)
        assertEquals(emptyList<ReaderAction>(), actions)
        settings.performKeyInput { pressKey(Key.DirectionRight) }
        assertEquals(listOf(ReaderAction.FORWARD), actions)
    }
}
