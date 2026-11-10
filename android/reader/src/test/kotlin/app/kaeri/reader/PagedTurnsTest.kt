// What the paged reader tells the screen about each leaf it shows: only a turn the person made
// counts as turned, so the engine's start place and a new layout after a rotation keep the chrome.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.TurnHow
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import app.kaeri.reader.ReaderFixtures.WORK
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
class PagedTurnsTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val pager = PagedReaderState()
    private val reports = mutableListOf<Triple<Int, TurnHow, Boolean>>()

    private fun open() {
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
                zones = TapZones.phone(ReadDirection.RIGHT_TO_LEFT),
                onControls = {},
                onShown = { leaf, how, turned ->
                    reports += Triple(leaf, how, turned)
                    session.shown(leaf, how)
                },
                onMissing = {},
                onRetry = {},
                onJumped = session::jumped,
                endPage = {},
                motion = false,
            )
        }
        compose.waitForIdle()
    }

    @Test
    fun the_start_place_and_a_new_layout_are_not_turns() {
        open()
        // The engine's start place.
        assertEquals(Triple(3, TurnHow.TURN, false), reports.last())

        compose.runOnIdle { pager.forward() }
        compose.waitForIdle()
        assertEquals(Triple(4, TurnHow.TURN, true), reports.last())

        // Turned to landscape, the chapter is laid out again in spreads: page 5 is now on the third
        // leaf. The engine's jump there is reported, but nobody turned.
        val spreads = List(4) { Leaf(it.toUInt(), listOf(ReaderFixtures.part((2 * it).toUInt()), ReaderFixtures.part((2 * it + 1).toUInt())), false) } +
            Leaf(4u, emptyList(), true)
        val before = reports.size
        compose.runOnIdle { session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, spreads)) }
        compose.waitForIdle()
        assertEquals(before + 1, reports.size)
        assertEquals(Triple(2, TurnHow.TURN, false), reports.last())

        // The next turn is the person's again.
        compose.runOnIdle { pager.forward() }
        compose.waitForIdle()
        assertEquals(Triple(3, TurnHow.TURN, true), reports.last())
    }
}
