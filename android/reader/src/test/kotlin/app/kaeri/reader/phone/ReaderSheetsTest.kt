// The reader's sheets go when what they were opened for is done: a chapter picked from the
// Chapters sheet opens with the sheet gone, as a page picked from it does.

package app.kaeri.reader.phone

import androidx.activity.ComponentActivity
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
import androidx.compose.ui.test.performClick
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderFixtures
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.TapZonesScenes
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

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
}
