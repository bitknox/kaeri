// A novel's end page and the controls: turning past the last page takes them away, so the end
// page's buttons are clear of them, and a tap in the middle shows and hides them there as on any
// page.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import app.kaeri.design.ObiSilk
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReaderMode
import app.kaeri.reader.phone.NovelPlace
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class NovelEndPageTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val dir = kotlin.io.path.createTempDirectory("novel-end").toFile()

    private fun novel() = TapZonesScenes.state(dir, mode = ReaderMode.PAGED).let { state ->
        state.copy(view = state.view?.copy(medium = Medium.NOVEL), leaves = emptyList())
    }

    @Test
    fun the_end_page_takes_the_controls_away_and_a_middle_tap_brings_them_back() {
        var shown: NovelPlace? = null
        var controls: () -> Unit = {}
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(
                    session = session,
                    state = novel(),
                    images = PageImages(),
                    actions = PhoneReaderActions(onClose = {}),
                    novel = { place, onControls ->
                        shown = place
                        controls = onControls
                    },
                )
            }
        }
        compose.waitForIdle()
        // The controls are up on the last page, and the page turns past it.
        compose.runOnIdle { controls() }
        compose.onNodeWithContentDescription("Chapters").assertIsDisplayed()
        compose.runOnIdle { shown!!.ended = true }
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Chapters").assertDoesNotExist()

        // A tap in the middle of the end page shows them, and another hides them.
        compose.onRoot().performTouchInput { click(center) }
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Chapters").assertIsDisplayed()
        compose.onRoot().performTouchInput { click(center) }
        compose.waitForIdle()
        compose.onNodeWithContentDescription("Chapters").assertDoesNotExist()
    }
}
