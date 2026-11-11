// A came-in dialog on paper: on the 7 inch and the 10.3 inch panel alike, the page behind is not
// dimmed and the window has no animation; in colour the platform's dim stays.
package app.kaeri.ui.reading

import android.app.Dialog
import androidx.activity.ComponentActivity
import androidx.compose.material3.Text
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.eink.EinkLook
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowDialog

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class PaperDialogTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private fun open(paper: EinkLook?): Dialog {
        compose.setContent {
            CompositionLocalProvider(LocalEinkLayout provides paper) {
                CameInDialog(open = true) {
                    CameInForm(title = "Join a plugin store", primary = null, secondary = null, close = {}) { Text("Body") } // i18n-skip
                }
            }
        }
        compose.waitForIdle()
        return requireNotNull(ShadowDialog.getLatestDialog())
    }

    @Test
    @Config(qualifiers = "w600dp-h960dp-port-300dpi")
    fun sevenInchPaperHasNoDimAndNoAnimation() {
        val attributes = requireNotNull(open(EinkLook()).window).attributes
        assertEquals(0f, attributes.dimAmount, 0f)
        assertEquals(0, attributes.windowAnimations)
    }

    @Test
    @Config(qualifiers = "w992dp-h1323dp-port-300dpi")
    fun largePaperHasNoDimAndNoAnimation() {
        val attributes = requireNotNull(open(EinkLook()).window).attributes
        assertEquals(0f, attributes.dimAmount, 0f)
        assertEquals(0, attributes.windowAnimations)
    }

    @Test
    @Config(qualifiers = "w600dp-h960dp-port-300dpi")
    fun colourKeepsTheDim() {
        val attributes = requireNotNull(open(null).window).attributes
        assertNotEquals(0f, attributes.dimAmount)
    }
}
