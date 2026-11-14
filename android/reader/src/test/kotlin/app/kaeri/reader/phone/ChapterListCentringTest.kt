// The reader's chapter list opens with the open chapter in the middle, as near as the list's ends
// allow, and TalkBack and the keyboard start on it (docs/native-ux.md, Reading).

package app.kaeri.reader.phone

import androidx.activity.ComponentActivity
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollToIndexAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderFixtures
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import org.junit.Assert.assertEquals
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

    /** Chapter [n]'s row in the list. The sheet's header names the open chapter too. */
    private fun row(n: Int): SemanticsNodeInteraction =
        compose.onNode(inList("Chapter $n"), useUnmergedTree = false)

    private fun inList(words: String) = hasText(words) and hasClickAction() and hasAnyAncestor(hasScrollToIndexAction())

    @Test
    fun the_open_chapter_opens_in_the_middle_with_the_focus() {
        open(120, 60)
        row(60).assertIsDisplayed().assertIsFocused()
        // As many rows show above it as below, give or take the one cut at an edge. The list
        // draws only the rows in view.
        val drawn = (30..90).filter { n -> compose.onAllNodes(inList("Chapter $n")).fetchSemanticsNodes().isNotEmpty() }
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
    fun the_sheet_stays_still_when_the_chapters_come_in_after_it_opened() {
        val rows = (1..120).map { n ->
            ReaderFixtures.chapter(id(n), "$n", null, null).copy(state = if (n < 60) ChapterReadState.Read else ChapterReadState.Unread)
        }
        val view = ReaderFixtures.view().copy(medium = Medium.NOVEL, chapter = rows[59])
        var state by mutableStateOf(ReaderState(view = view, volumes = emptyList()))
        compose.setContent {
            ChaptersSheet(ReaderSession {}, state, PageImages(), 0, false, {}, {}, instant = true)
        }
        compose.waitForIdle()
        fun top() = compose.onNodeWithText(view.title).getBoundsInRoot().top
        val before = top()
        state = state.copy(volumes = listOf(VolumeGroup(null, null, rows, false)))
        compose.waitForIdle()
        assertEquals(before, top())
        row(60).assertIsDisplayed().assertIsFocused()
    }

    @Test
    @Config(qualifiers = "w915dp-h412dp-land-420dpi")
    fun a_phone_held_sideways_has_the_whole_list_on_screen() {
        open(120, 60)
        val list = compose.onNode(hasScrollToIndexAction()).getBoundsInRoot()
        assertTrue("$list", list.bottom <= compose.onRoot().getBoundsInRoot().bottom)
        row(60).assertIsDisplayed()
    }

    @Test
    fun the_last_chapter_sits_at_the_end() {
        open(120, 120)
        row(120).assertIsDisplayed().assertIsFocused()
        row(112).assertIsDisplayed()
    }
}
