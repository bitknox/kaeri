// A swipe still settling when another chapter opens: its turn names a leaf of the chapter it
// started in, so the new chapter stays on the page it opened at.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeRight
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.TapReading
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
class PagedSettleTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val pager = PagedReaderState()
    private val shown = mutableListOf<Pair<Int, TurnHow>>()

    @Test
    fun a_swipe_settling_as_another_chapter_opens_leaves_that_chapter_where_it_opened() {
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
                reading = TapReading(ReaderMode.PAGED, ReadDirection.RIGHT_TO_LEFT, false, false),
                onControls = {},
                onShown = { leaf, how, _ ->
                    shown += leaf to how
                    session.shown(leaf, how)
                },
                onMissing = {},
                onRetry = {},
                onJumped = session::jumped,
                endPage = {},
                onPastEnd = { true },
                motion = true,
            )
        }
        compose.waitForIdle()
        assertEquals(3, pager.leaf)

        // A swipe to the next page, still easing out when chapter 13 opens at its first page.
        compose.mainClock.autoAdvance = false
        compose.onRoot().performTouchInput { swipeRight() }
        compose.mainClock.advanceTimeByFrame()
        val next = "local:lantern-stair/13"
        session.openChapter(next)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u).copy(chapter = ReaderFixtures.chapter(next, "13"))))
        session.apply(ReaderEvent.Leaves(SESSION, next, ReaderFixtures.leaves(8)))
        compose.mainClock.advanceTimeBy(2_000)
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()

        assertEquals(next, session.state.value.chapter)
        assertEquals(0, pager.leaf)
        assertEquals(0 to TurnHow.PLACED, shown.last())
    }
}
