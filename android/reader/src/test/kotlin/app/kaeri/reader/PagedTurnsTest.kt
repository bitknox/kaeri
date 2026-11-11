// What the paged reader tells the screen about each leaf it shows: only a turn the person made
// counts as turned, so the engine's start place and a new layout after a rotation keep the chrome.
// A strip cut into screens also moves on with an upward swipe.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.test.swipeRight
import androidx.compose.ui.test.swipeUp
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
    private var crossed = 0
    private val missing = mutableListOf<Int>()

    private fun open(leaves: List<Leaf> = ReaderFixtures.leaves(8)) {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 3u)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, leaves))
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
                onMissing = { missing += it.page.toInt() },
                onRetry = {},
                onJumped = session::jumped,
                endPage = {},
                onPastEnd = {
                    crossed++
                    true
                },
                motion = false,
            )
        }
        compose.waitForIdle()
    }

    @Test
    fun the_start_place_and_a_new_layout_are_not_turns() {
        open()
        // The engine's start place.
        assertEquals(Triple(3, TurnHow.PLACED, false), reports.last())

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
        assertEquals(Triple(2, TurnHow.PLACED, false), reports.last())

        // The next turn is the person's again.
        compose.runOnIdle { pager.forward() }
        compose.waitForIdle()
        assertEquals(Triple(3, TurnHow.TURN, true), reports.last())
    }

    @Test
    fun a_new_layout_whose_jump_lands_on_the_same_leaf_number_is_reported() {
        open()
        assertEquals(Triple(3, TurnHow.PLACED, false), reports.last())
        // Laid out again, the place it shows is on leaf 3 again: the pager does not move, and the
        // engine still hears which leaf of the new layout is on screen.
        val before = reports.size
        compose.runOnIdle { session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(8))) }
        compose.waitForIdle()
        assertEquals(before + 1, reports.size)
        assertEquals(Triple(3, TurnHow.PLACED, false), reports.last())
    }

    @Test
    fun a_page_on_screen_that_no_file_came_for_is_asked_for() {
        open()
        compose.mainClock.advanceTimeBy(PageWaits.ASK_MS + 100)
        compose.waitForIdle()
        // Only the leaf on screen asks; the pages beside it are the engine's to make ahead.
        assertEquals(listOf(3), missing)
    }

    @Test
    fun a_forward_turn_on_the_end_page_crosses_to_the_next_chapter() {
        open()
        // The end page, after the eight pages.
        compose.runOnIdle { pager.go(8, TurnHow.JUMP) }
        compose.waitForIdle()
        assertEquals(8, pager.leaf)

        // A key or a tap in the forward zone.
        compose.runOnIdle { pager.forward(TurnHow.KEY) }
        assertEquals(1, crossed)

        // Right to left, a swipe to the right goes on; one to the left returns to the last page.
        compose.onRoot().performTouchInput { swipeRight() }
        compose.waitForIdle()
        assertEquals(2, crossed)
        compose.onRoot().performTouchInput { swipeLeft() }
        compose.waitForIdle()
        assertEquals(2, crossed)
        assertEquals(7, pager.leaf)
    }

    @Test
    fun forward_turns_before_the_end_page_never_cross() {
        open()
        compose.runOnIdle { pager.go(6, TurnHow.JUMP) }
        compose.runOnIdle { pager.forward() }
        compose.runOnIdle { pager.forward() }
        compose.waitForIdle()
        assertEquals(8, pager.leaf)
        assertEquals(0, crossed)
    }

    @Test
    fun a_strip_cut_into_screens_moves_on_with_an_upward_swipe() {
        open(ReaderFixtures.cuts(4, 2))
        val from = pager.leaf
        compose.onRoot().performTouchInput { swipeUp() }
        compose.waitForIdle()
        assertEquals(from + 1, pager.leaf)
        assertEquals(TurnHow.SWIPE, reports.last().second)
        compose.onRoot().performTouchInput { swipeDown() }
        compose.waitForIdle()
        assertEquals(from, pager.leaf)
    }

    @Test
    fun an_upward_swipe_on_pages_turns_nothing() {
        open()
        val from = pager.leaf
        compose.onRoot().performTouchInput { swipeUp() }
        compose.waitForIdle()
        assertEquals(from, pager.leaf)
    }
}
