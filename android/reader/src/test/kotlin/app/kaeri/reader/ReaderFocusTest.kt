// A screen reader's way into the strip and the novel: each puts its moves (Next, Previous and
// Controls) on a node that says where the reader is, so focus can land on it and its actions come
// up in the actions menu, as the paged reader's page does.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.NovelAssets
import app.kaeri.engine.model.ReaderMode
import app.kaeri.reader.phone.NovelPlace
import app.kaeri.reader.phone.NovelScreen
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ReaderFocusTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val dir = kotlin.io.path.createTempDirectory("reader-focus").toFile()

    /** A node a screen reader can rest on (it has words) that carries the move [action]. */
    private fun placeWith(action: String) = SemanticsMatcher("words and the action $action") { node ->
        !node.config.getOrNull(SemanticsProperties.ContentDescription).isNullOrEmpty() &&
            node.config.getOrNull(SemanticsActions.CustomActions).orEmpty().any { it.label == action }
    }

    private fun words(action: String) =
        compose.onNode(placeWith(action), useUnmergedTree = true).fetchSemanticsNode().config[SemanticsProperties.ContentDescription].joinToString()

    @Test
    fun the_strip_says_its_page_and_carries_its_moves() {
        val state = TapZonesScenes.state(dir, mode = ReaderMode.STRIP)
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = session, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> })
            }
        }
        compose.waitForIdle()
        assertEquals("Page 1 of 3", words("Scroll down"))
        compose.onNode(placeWith("Scroll up"), useUnmergedTree = true).assertExists()
        compose.onNode(placeWith("Controls"), useUnmergedTree = true).assertExists()
    }

    @Test
    fun the_novel_says_where_it_is_and_carries_its_moves() {
        val state = TapZonesScenes.state(dir)
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                NovelScreen(session, state, NovelPlace(), NovelAssets("", "", ""), resource = { null }, onControls = {})
            }
        }
        compose.waitForIdle()
        // Before the paginator counts its pages, the chapter.
        assertEquals("Chapter 12 · The Long Stair", words("Next page"))
        compose.onNode(placeWith("Previous page"), useUnmergedTree = true).assertExists()
        compose.onNode(placeWith("Controls"), useUnmergedTree = true).assertExists()
    }
}
