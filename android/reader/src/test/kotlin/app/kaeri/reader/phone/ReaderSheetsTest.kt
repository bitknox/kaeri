// The reader's sheets go when what they were opened for is done: a chapter picked from the
// Chapters sheet opens with the sheet gone, as a page picked from it does. Back from a page of the
// settings sheet goes to its main page, and the next Back closes it. A Back gesture on the main page
// shrinks the sheet as Material's own does, and one on another page leaves it as it is.

package app.kaeri.reader.phone

import androidx.activity.ComponentActivity
import androidx.activity.BackEventCompat
import androidx.activity.ComponentDialog
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasScrollToIndexAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.width
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderFixtures
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.TapZonesScenes
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowDialog

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ReaderSheetsTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val dir = kotlin.io.path.createTempDirectory("reader-sheets").toFile()

    /** The page's node, which carries the reader's moves as actions. */
    private val page = SemanticsMatcher("the page with its moves") { node ->
        node.config.getOrNull(SemanticsActions.CustomActions).orEmpty().any { it.label == "Controls" }
    }

    /** The paged reader open at chapter 12 of 11 to 13, its controls up. */
    private fun reader() {
        val rows = (11..13).map { ReaderFixtures.chapter("local:lantern-stair/$it", "$it", null) }
        val state = TapZonesScenes.state(dir).copy(
            view = ReaderFixtures.view(start = 0u).copy(chapter = rows[1]),
            leaves = ReaderFixtures.leaves(3),
            volumes = listOf(VolumeGroup(null, null, rows, false)),
        )
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = ReaderSession {}, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> })
            }
        }
        compose.waitForIdle()
        val node = compose.onNode(page, useUnmergedTree = true).fetchSemanticsNode()
        compose.runOnUiThread { node.config[SemanticsActions.CustomActions].first { it.label == "Controls" }.action() }
        compose.waitForIdle()
    }

    private fun row(n: Int) = compose.onNode(hasText("Chapter $n") and hasClickAction() and hasAnyAncestor(hasScrollToIndexAction()))

    @Test
    fun picking_a_chapter_closes_the_chapters_sheet() {
        reader()
        compose.onNode(hasContentDescription("Chapters") and hasClickAction()).performClick()
        compose.waitForIdle()
        compose.onNode(hasText("Chapters") and hasClickAction()).performClick()
        compose.waitForIdle()
        row(13).performClick()
        compose.waitForIdle()
        row(13).assertDoesNotExist()
        compose.onNode(hasText("Pages") and hasClickAction()).assertDoesNotExist()
    }

    /** Back as the sheet's own window takes it. */
    private fun back() {
        val dialog = ShadowDialog.getLatestDialog() as ComponentDialog
        compose.runOnUiThread { dialog.onBackPressedDispatcher.onBackPressed() }
        compose.waitForIdle()
    }

    @Test
    fun back_from_more_settings_goes_to_the_main_page_and_the_next_back_closes_the_sheet() {
        reader()
        compose.onNode(hasContentDescription("Reader settings") and hasClickAction()).performClick()
        compose.waitForIdle()
        val more = compose.onNode(hasText("More reader settings") and hasClickAction())
        more.performClick()
        compose.waitForIdle()
        more.assertDoesNotExist()

        back()
        more.assertExists()

        back()
        compose.onNode(hasText("More reader settings")).assertDoesNotExist()
    }

    @Test
    fun the_back_after_more_settings_closes_a_sheet_dragged_open() {
        reader()
        compose.onNode(hasContentDescription("Reader settings") and hasClickAction()).performClick()
        compose.waitForIdle()
        // Dragged up to full height, as on a phone where the row sits below the fold.
        compose.onNode(hasText("Reading mode")).performTouchInput { swipeUp() }
        compose.waitForIdle()
        val more = compose.onNode(hasText("More reader settings") and hasClickAction())
        more.performClick()
        compose.waitForIdle()

        back()
        more.assertExists()

        back()
        compose.onNode(hasText("More reader settings")).assertDoesNotExist()
    }

    /** A Back gesture drawn to [progress] in the sheet's window and held there. */
    private fun gesture(progress: Float) {
        val dialog = ShadowDialog.getLatestDialog() as ComponentDialog
        compose.runOnUiThread {
            dialog.onBackPressedDispatcher.dispatchOnBackStarted(BackEventCompat(0f, 1200f, 0f, BackEventCompat.EDGE_LEFT))
            dialog.onBackPressedDispatcher.dispatchOnBackProgressed(BackEventCompat(400f, 1200f, progress, BackEventCompat.EDGE_LEFT))
        }
        compose.waitForIdle()
    }

    private fun letGo() {
        val dialog = ShadowDialog.getLatestDialog() as ComponentDialog
        compose.runOnUiThread { dialog.onBackPressedDispatcher.dispatchOnBackCancelled() }
        compose.waitForIdle()
    }

    @Test
    fun a_back_gesture_shrinks_the_sheet_on_its_main_page_only() {
        reader()
        compose.onNode(hasContentDescription("Reader settings") and hasClickAction()).performClick()
        compose.waitForIdle()
        val more = compose.onNode(hasText("More reader settings") and hasClickAction())
        // Bounds with the layers' scale on them, as drawn.
        val open = more.getBoundsInRoot()

        gesture(1f)
        val drawn = more.getBoundsInRoot()
        // Narrowed by Material's 48 dp at the gesture's end, less the row's share of the margins.
        assertTrue("the row is ${drawn.width} wide while drawn, ${open.width} open", drawn.width < open.width - 30.dp)
        letGo()
        assertEquals(open.width, more.getBoundsInRoot().width)

        more.performClick()
        compose.waitForIdle()
        val back = compose.onNode(hasContentDescription("Back, More reader settings", substring = true), useUnmergedTree = true)
        val page = back.getBoundsInRoot()
        gesture(1f)
        assertEquals(page, back.getBoundsInRoot())
        letGo()
        more.assertDoesNotExist()
    }
}
