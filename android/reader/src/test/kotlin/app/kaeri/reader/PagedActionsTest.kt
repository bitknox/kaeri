// What TalkBack gets on the paged reader's page: one element whose actions are Next page,
// Previous page and Show controls, the names the spec gives every platform's screen reader.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.TapReading
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import app.kaeri.reader.ReaderFixtures.WORK
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class PagedActionsTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val pager = PagedReaderState()
    private var controls = 0

    @Test
    fun the_page_offers_next_previous_and_show_controls() {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 3u)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(8)))
        compose.setContent {
            val state = session.state.collectAsState().value
            PagedReader(
                state = state,
                pager = pager,
                images = PageImages(),
                direction = ReadDirection.RIGHT_TO_LEFT,
                taps = state.tapZones(),
                reading = TapReading(ReaderMode.PAGED, ReadDirection.RIGHT_TO_LEFT, false, true),
                onControls = { controls++ },
                onShown = { leaf, how, _ -> session.shown(leaf, how) },
                onMissing = {},
                onRetry = {},
                onJumped = session::jumped,
                endPage = {},
                onPastEnd = { true },
                motion = false,
            )
        }
        compose.waitForIdle()
        val page = compose.onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsActions.CustomActions)).fetchSemanticsNodes().single()
        val actions = page.config[SemanticsActions.CustomActions]
        assertEquals(listOf("Next page", "Previous page", "Show controls"), actions.map { it.label }) // i18n-skip

        compose.runOnUiThread { actions.last().action() }
        assertEquals(1, controls)
    }
}
