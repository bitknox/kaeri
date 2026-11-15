// A chapter row read by TalkBack says it is a chapter: a source's bare "32" is read "Chapter 32",
// and a label with its own word is read as it is.

package app.kaeri.ui.reading

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ChapterRowSpeechTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    /** What TalkBack reads first on the row: its name. The state line follows it. */
    private fun said(row: ChapterRow): String {
        compose.setContent {
            KaeriTheme { ChapterRowItem(row, chosen = null, onClick = {}, onLongClick = {}, onStop = {}, onRetry = {}) }
        }
        val node = compose.onNode(hasClickAction()).fetchSemanticsNode()
        // No description, which TalkBack would read in place of every line of the row.
        assertEquals(null, node.config.getOrNull(SemanticsProperties.ContentDescription))
        return node.config.getOrNull(SemanticsProperties.Text).orEmpty().first().text
    }

    @Test
    fun a_bare_number_is_read_as_a_chapter() {
        assertEquals("Chapter 32", said(ChapterRow(id = "c32", label = "32", state = ChapterReadState.Unread, number = "32", download = null)))
    }

    @Test
    fun a_bare_number_keeps_its_title() {
        val row = ChapterRow(id = "c33", label = "33", state = ChapterReadState.Unread, number = "33", download = null, title = "Rust Bloom")
        assertEquals("Chapter 33 Rust Bloom", said(row))
    }

    @Test
    fun a_label_with_its_own_word_is_read_as_it_is() {
        assertEquals("Chapter 12", said(ChapterRow(id = "c12", label = "Chapter 12", state = ChapterReadState.Unread, number = "12", download = null)))
    }
}
