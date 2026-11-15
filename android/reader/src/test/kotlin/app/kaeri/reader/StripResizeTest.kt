// A strip keeps the image at the top and the spot in it across a new width, as a fold or a
// smaller window gives it: the list alone keeps the pixels into its top row, which run past a
// shorter image into the ones after it.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.unit.dp
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.ReadPlace
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.StripPos
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
@Config(sdk = [36], qualifiers = "w600dp-h900dp-port-320dpi")
class StripResizeTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val sent = mutableListOf<ReaderCommand>()
    private val session = ReaderSession { sent += (it as Command.Reader).v1 }
    private val list = LazyListState()
    private val width = mutableStateOf(600.dp)
    private lateinit var scope: CoroutineScope

    /**
     * Twenty images and the end page. The sixth is a long one, ten times as tall as it is wide;
     * the rest are the usual page's shape.
     */
    private val leaves = List(20) { i ->
        Leaf(i.toUInt(), listOf(ReaderFixtures.part(i.toUInt()).copy(ratio = if (i == 5) 0.1f else 0.7f)), false)
    } + Leaf(20u, emptyList(), true)

    private fun openAt(start: ReadPlace) {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(mode = ReaderMode.STRIP).copy(start = start)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, leaves))
        compose.setContent {
            scope = rememberCoroutineScope()
            val state by session.state.collectAsState()
            val w by width
            Box(Modifier.width(w).fillMaxHeight()) {
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

    /** How far into the image at the top the screen's top edge is, 0 to 1. */
    private fun into(): Float {
        val top = list.layoutInfo.visibleItemsInfo.first { it.index == list.firstVisibleItemIndex }
        return list.firstVisibleItemScrollOffset.toFloat() / top.size
    }

    private fun resize(to: Int) {
        width.value = to.dp
        compose.waitForIdle()
    }

    @Test
    fun a_narrower_strip_keeps_the_long_image_and_the_spot_in_it() {
        // Four fifths down the long image: at half the width it is half as tall, and the pixels
        // into it would run on through the next images.
        openAt(ReadPlace(WORK, CHAPTER, 5u, StripPos(5u, 0.8f), null))
        assertEquals(5, list.firstVisibleItemIndex)
        assertEquals(0.8f, into(), 0.02f)
        resize(300)
        assertEquals("the image at the top after the strip got narrower", 5, list.firstVisibleItemIndex)
        assertEquals(0.8f, into(), 0.02f)
        resize(600)
        assertEquals("the image at the top once the strip is wide again", 5, list.firstVisibleItemIndex)
        assertEquals(0.8f, into(), 0.02f)
    }

    @Test
    fun a_scroll_after_a_new_width_still_moves_the_strip_and_says_where() {
        openAt(ReadPlace(WORK, CHAPTER, 5u, StripPos(5u, 0.8f), null))
        resize(300)
        sent.clear()
        // Past the rest of the long image, onto the next one.
        compose.runOnIdle { scope.launch { list.scroll { scrollBy(2000f); repeat(4) { withFrameNanos { } } } } }
        compose.waitForIdle()
        assertTrue("moved past the long image", list.firstVisibleItemIndex > 5)
        val at = sent.filterIsInstance<ReaderCommand.StripAt>().lastOrNull()
        assertEquals(list.firstVisibleItemIndex.toUInt(), at?.at?.image)
    }
}
