// A strip opens where the engine's start place says: its strip spot, or its page when it names
// none, as a place kept in paged reading does. The first scroll says a place from there, never
// from the top.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.ReadPlace
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.StripPos
import app.kaeri.engine.model.TurnHow
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import app.kaeri.reader.ReaderFixtures.WORK
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class StripStartTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val sent = mutableListOf<ReaderCommand>()
    private val session = ReaderSession { sent += (it as Command.Reader).v1 }
    private val list = LazyListState()
    private lateinit var scope: CoroutineScope

    /** A scroll by [px], held over a few frames as a finger's is, then let go. */
    private fun drag(px: Float) {
        compose.runOnIdle { scope.launch { list.scroll { scrollBy(px); repeat(4) { withFrameNanos { } } } } }
        compose.waitForIdle()
    }

    /** Twelve images of a chapter, each a page of its own, and the end page. */
    private val leaves = List(12) { Leaf(it.toUInt(), listOf(ReaderFixtures.part(it.toUInt()).copy(ratio = 0.5f)), false) } + Leaf(12u, emptyList(), true)

    private fun openAt(start: ReadPlace) {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(mode = ReaderMode.STRIP).copy(start = start)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, leaves))
        compose.setContent {
            scope = rememberCoroutineScope()
            val state by session.state.collectAsState()
            Box(Modifier.fillMaxSize()) {
                StripReader(
                    state = state,
                    images = PageImages(),
                    onAt = session::stripAt,
                    onTap = { _, _ -> },
                    onScrolled = {},
                    onMissing = {},
                    onRetry = {},
                    onJumped = session::jumped,
                    footer = {},
                    divider = { Text(it.label) },
                    list = list,
                )
            }
        }
        compose.waitForIdle()
    }

    private fun firstScrollSays(image: UInt) {
        sent.clear()
        drag(60f)
        val at = sent.filterIsInstance<ReaderCommand.StripAt>()
        assertTrue(at.isNotEmpty())
        assertTrue(at.all { it.chapter == CHAPTER && it.at.image == image })
    }

    @Test
    fun a_start_with_a_strip_spot_opens_there() {
        openAt(ReadPlace(WORK, CHAPTER, 9u, StripPos(9u, 0f), null))
        assertEquals(9, list.firstVisibleItemIndex)
        assertEquals(0, list.firstVisibleItemScrollOffset)
        firstScrollSays(9u)
    }

    @Test
    fun a_start_kept_in_paged_reading_opens_at_its_page() {
        // A place from paged reading names a page and no strip spot.
        openAt(ReadPlace(WORK, CHAPTER, 9u, null, null))
        assertEquals(9, list.firstVisibleItemIndex)
        firstScrollSays(9u)
    }

    @Test
    fun a_spot_part_way_down_an_image_opens_part_way_down_it() {
        openAt(ReadPlace(WORK, CHAPTER, 9u, StripPos(9u, 0.5f), null))
        assertEquals(9, list.firstVisibleItemIndex)
        val size = list.layoutInfo.visibleItemsInfo.first { it.index == 9 }.size
        assertEquals(size / 2f, list.firstVisibleItemScrollOffset.toFloat(), size * 0.02f)
    }

    @Test
    fun a_switch_from_pages_to_the_strip_keeps_the_page_shown() {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(12)))
        session.jumped(session.state.value.jump!!.id)
        session.shown(9, TurnHow.TURN)
        // Long strip chosen: the chapter comes laid out again as whole images.
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, leaves))
        val jump = session.state.value.jump!!
        assertEquals(9, jump.leaf)
        assertTrue(jump.strip == null || jump.strip.image == 9u)
    }
}
