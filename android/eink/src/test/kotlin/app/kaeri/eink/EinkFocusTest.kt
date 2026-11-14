// The focus mark on paper: focus moved by keys draws an ink ring along the focused control's
// shape, with a paper line inside it that shows on an ink button too; the ring moves with the
// focus, and a touch on its own never draws one.

package app.kaeri.eink

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.input.InputModeManager
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.requestFocus
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w572dp-h772dp-port-xhdpi")
class EinkFocusTest {
    private val compose = createAndroidComposeRule<ComponentActivity>()

    @get:Rule
    val rules: RuleChain = RuleChain.outerRule(
        object : ExternalResource() {
            override fun before() {
                val app = RuntimeEnvironment.getApplication()
                shadowOf(app.packageManager).addActivityIfNotPresent(ComponentName(app, ComponentActivity::class.java))
            }
        },
    ).around(compose)

    private var clicks = 0
    private var modes: InputModeManager? = null

    private fun controls() {
        compose.setContent {
            modes = LocalInputModeManager.current
            EinkTheme {
                Column(
                    Modifier.fillMaxSize().background(EinkColours.Paper).padding(24.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    EinkSolidButton("Continue", { clicks++ })
                    EinkTextButton("Done", { clicks++ })
                    EinkOutlinedButton("Details", { clicks++ })
                }
            }
        }
        compose.waitForIdle()
    }

    /** The colour at [dpIn] from the control's start edge, halfway down it. */
    private fun SemanticsNodeInteraction.at(dpIn: Float): Color {
        val image: ImageBitmap = captureToImage()
        val px = (dpIn * 2).toInt() // xhdpi: 2 px a dp
        return image.toPixelMap()[px, image.height / 2]
    }

    /** Out of touch mode, as the first key press on a device takes the window. */
    @OptIn(ExperimentalComposeUiApi::class)
    private fun keyboardMode() {
        compose.runOnUiThread { modes!!.requestInputMode(InputMode.Keyboard) }
        compose.waitForIdle()
        assertEquals(InputMode.Keyboard, modes!!.inputMode)
    }

    @Test
    fun keys_draw_the_mark_on_the_focused_control_and_it_moves_with_the_focus() {
        controls()
        val solid = compose.onNodeWithText("Continue")
        val text = compose.onNodeWithText("Done")
        val outlined = compose.onNodeWithText("Details")
        // Unfocused: the ink button is ink at its edge and 4 dp in, the text button paper, the
        // outlined button paper 5 dp in, past its 2 dp border.
        assertEquals(EinkColours.Ink, solid.at(4f))
        assertEquals(EinkColours.Paper, text.at(1f))
        assertEquals(EinkColours.Paper, outlined.at(5.5f))

        keyboardMode()
        solid.requestFocus()
        compose.waitForIdle()
        solid.assertIsFocused()
        // On ink the ring cannot show, so the paper line inside it does: 3 to 5 dp in.
        assertEquals(EinkColours.Paper, solid.at(4f))

        text.requestFocus()
        compose.waitForIdle()
        text.assertIsFocused()
        assertEquals(EinkColours.Ink, text.at(1f))
        assertEquals(EinkColours.Ink, solid.at(4f))

        outlined.requestFocus()
        compose.waitForIdle()
        outlined.assertIsFocused()
        // Inside the button's own border: 2 dp of paper, then the ring 4 to 7 dp in.
        assertEquals(EinkColours.Ink, outlined.at(5.5f))
        assertEquals(EinkColours.Paper, text.at(1f))
        assertEquals(0, clicks)

        // Enter acts on the focused control.
        outlined.performKeyInput { pressKey(Key.Enter) }
        compose.waitForIdle()
        assertEquals(1, clicks)
    }

    @Test
    fun a_touch_never_draws_the_mark() {
        controls()
        val text = compose.onNodeWithText("Done")
        text.performClick()
        compose.mainClock.advanceTimeBy(PRESS_MS + 50)
        compose.waitForIdle()
        assertEquals(1, clicks)
        assertEquals(EinkColours.Paper, text.at(1f))
    }
}
