// A strip flows on only when the person scrolls onto the divider with the chapter's images in: a
// chapter that just opened with its images still loading is short, and must not flow on, or count
// its foot as reached, by itself.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.PageFile
import app.kaeri.engine.model.ReaderMode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class StripFlowGateTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val dir = kotlin.io.path.createTempDirectory("strip-flow-gate").toFile()
    private val flowed = mutableListOf<String>()
    private val feet = mutableListOf<Boolean>()

    /** A chapter of one image, short enough that the divider under it is on screen at once. */
    private fun strip(file: PageFile?) {
        val leaves = listOf(Leaf(0u, listOf(ReaderFixtures.part(0u, file)), false), Leaf(1u, emptyList(), true))
        val state = ReaderState(view = ReaderFixtures.view(start = 0u, mode = ReaderMode.STRIP), leaves = leaves)
        compose.setContent {
            Box(Modifier.fillMaxSize()) {
                StripReader(
                    state = state,
                    images = PageImages(),
                    onAt = { _, foot -> feet += foot },
                    onTap = { _, _ -> },
                    onScrolled = {},
                    onMissing = {},
                    onRetry = {},
                    onJumped = {},
                    footer = {},
                    divider = { Text(it.label) },
                    onFlow = { flowed += it.id },
                    list = LazyListState(),
                )
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun a_chapter_still_loading_its_images_neither_flows_on_nor_reaches_its_foot_by_itself() {
        strip(file = null)
        compose.onRoot().performTouchInput { swipeUp(startY = centerY, endY = centerY - 40f) }
        compose.waitForIdle()
        assertEquals(emptyList<String>(), flowed)
        assertFalse(feet.any { it })
    }

    @Test
    fun a_chapter_left_alone_does_not_flow_on_even_with_its_images_in() {
        strip(file = ReaderFixtures.grey(dir, "p0", 400, 300))
        assertEquals(emptyList<String>(), flowed)
    }

    @Test
    fun a_scroll_onto_the_divider_with_the_images_in_flows_on() {
        strip(file = ReaderFixtures.grey(dir, "p0", 400, 300))
        compose.onRoot().performTouchInput { swipeUp(startY = centerY, endY = centerY - 40f) }
        compose.waitForIdle()
        assertEquals(listOf("local:lantern-stair/13"), flowed)
        assertTrue(feet.any { it })
    }
}
