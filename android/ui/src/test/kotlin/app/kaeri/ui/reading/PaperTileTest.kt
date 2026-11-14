// A grid tile on paper: a series with a new chapter shows the dot and "New" in place of its line,
// and TalkBack hears "New" as the tile's state; one without shows its line and no state. The
// cover sits centred in a column wider than it.
package app.kaeri.ui.reading

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.width
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.dp
import app.kaeri.eink.EinkLook
import app.kaeri.eink.EinkTheme
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.WorkTile
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
@Config(sdk = [36])
class PaperTileTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val grid = PaperGrid(columns = 4, rows = 3, cover = 160.dp)

    private fun show(tile: WorkTile) {
        compose.setContent {
            EinkTheme(EinkLook()) {
                Box(Modifier.width(220.dp).testTag("cell")) { PaperTile(tile, grid, line = "Manga · Chapter 4") {} } // i18n-skip
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun aNewChapterShowsTheMarkAndSaysIt() {
        show(WorkTile(work = "a:1", title = "Ember Post Office", medium = Medium.MANGA, newChapter = true)) // i18n-skip
        compose.onNodeWithText("New", useUnmergedTree = true).assertExists() // i18n-skip
        assertTrue(compose.onAllNodes(hasText("Manga · Chapter 4"), useUnmergedTree = true).fetchSemanticsNodes().isEmpty()) // i18n-skip
        val tile = compose.onNodeWithText("Ember Post Office").fetchSemanticsNode() // i18n-skip
        assertEquals("New", tile.config.getOrElseNullable(SemanticsProperties.StateDescription) { null }) // i18n-skip
    }

    @Test
    fun withoutOneTheLineShowsAndNoState() {
        show(WorkTile(work = "a:2", title = "Paper Kite", medium = Medium.MANGA)) // i18n-skip
        compose.onNodeWithText("Manga · Chapter 4", useUnmergedTree = true).assertExists() // i18n-skip
        val tile = compose.onNodeWithText("Paper Kite").fetchSemanticsNode() // i18n-skip
        assertFalse(tile.config.contains(SemanticsProperties.StateDescription))
    }

    @Test
    fun theCoverIsCentredInItsColumn() {
        show(WorkTile(work = "a:3", title = "Paper Kite", medium = Medium.MANGA)) // i18n-skip
        val cell = compose.onNodeWithTag("cell").fetchSemanticsNode().boundsInRoot
        val tile = compose.onNodeWithText("Paper Kite").fetchSemanticsNode().boundsInRoot // i18n-skip
        // As much room on either side of the tile.
        assertEquals(0f, (tile.left - cell.left) - (cell.right - tile.right), 1f)
    }
}
