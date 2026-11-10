// The reader's chapter list opens with the open chapter in the middle, as near as the list's ends
// allow, and TalkBack and the keyboard start on it (docs/native-ux.md, Reading).

package app.kaeri.reader.phone

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderFixtures
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ChapterListCentringTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private fun id(n: Int) = "local:lantern-stair/$n"

    /** The list of a novel (so it opens on the chapters) of [count] chapters, open at [here]. */
    private fun open(count: Int, here: Int) {
        val rows = (1..count).map { n ->
            ReaderFixtures.chapter(id(n), "$n", null, null).copy(state = if (n < here) ChapterReadState.Read else ChapterReadState.Unread)
        }
        val view = ReaderFixtures.view().copy(medium = Medium.NOVEL, chapter = rows[here - 1])
        val state = ReaderState(view = view, volumes = listOf(VolumeGroup(null, null, rows, false)))
        compose.setContent {
            ChaptersSheet(ReaderSession {}, state, PageImages(), 0, false, {}, {}, instant = true)
        }
        compose.waitForIdle()
    }

    private fun row(n: Int): SemanticsNodeInteraction =
        compose.onNode(hasText("Chapter $n") and hasClickAction(), useUnmergedTree = false)

    @Test
    fun the_open_chapter_opens_in_the_middle_with_the_focus() {
        open(120, 60)
        row(60).assertIsDisplayed().assertIsFocused()
        // As many rows show above it as below, give or take the one cut at an edge. The list
        // draws only the rows in view.
        val drawn = (30..90).filter { n -> compose.onAllNodes(hasText("Chapter $n") and hasClickAction()).fetchSemanticsNodes().isNotEmpty() }
        val above = drawn.count { it < 60 }
        val below = drawn.count { it > 60 }
        assertTrue("$drawn", above >= 3 && kotlin.math.abs(above - below) <= 1)
        row(56).assertIsDisplayed()
        row(64).assertIsDisplayed()
        // Rows a screen away are not drawn at all.
        row(45).assertDoesNotExist()
        row(75).assertDoesNotExist()
    }

    @Test
    fun a_list_that_fits_stays_at_its_top() {
        open(5, 3)
        row(1).assertIsDisplayed()
        row(3).assertIsDisplayed().assertIsFocused()
    }

    @Test
    fun the_last_chapter_sits_at_the_end() {
        open(120, 120)
        row(120).assertIsDisplayed().assertIsFocused()
        row(112).assertIsDisplayed()
    }
}
