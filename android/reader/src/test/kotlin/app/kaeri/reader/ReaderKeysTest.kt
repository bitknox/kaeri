// The reader's keys with a control inside it reached by keyboard: Enter belongs to the reader while
// the reader holds the focus, and to the control while the control does; the arrows turn either way.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.input.InputModeManager
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertIsFocused
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
