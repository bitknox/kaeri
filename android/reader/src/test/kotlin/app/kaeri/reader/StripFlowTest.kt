// A strip flung onto the divider into the next chapter: the room under the divider holds the
// scroll while that chapter opens, and its images then take that room's place. They never push
// the scroll past themselves onto the divider after them, which would flow on again.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.unit.dp
import app.kaeri.engine.model.ReaderMode
import kotlinx.coroutines.runBlocking
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
class StripFlowTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun the_images_of_the_chapter_flowed_into_take_the_room_under_its_divider() {
        val into = ReaderFixtures.chapter("local:lantern-stair/13", "13", "Lanterns Out", 4u)
        val after = ReaderFixtures.chapter("local:lantern-stair/14", "14", null, 4u)
        val first = ReaderState(view = ReaderFixtures.view(mode = ReaderMode.STRIP, next = into), leaves = ReaderFixtures.leaves(4))
        var state by mutableStateOf(first)
        val flows = mutableListOf<String>()
        val list = LazyListState()
        compose.setContent {
            StripReader(
                state = state,
                images = PageImages(),
                onAt = { _, _ -> },
                onTap = { _, _ -> },
                onScrolled = {},
                onMissing = {},
                onRetry = {},
                onJumped = {},
                footer = {},
                divider = { Box(Modifier.fillMaxWidth().height(40.dp)) },
                onFlow = { flows += it.id },
                list = list,
            )
        }
        compose.waitForIdle()
        // Four images, the divider into chapter 13, then the room under it.
        assertEquals(6, list.layoutInfo.totalItemsCount)
        // Flung past the divider, which flows on, to the end: the room fills the screen.
        compose.runOnIdle { runBlocking { list.scrollToItem(4) } }
        compose.waitForIdle()
        compose.runOnIdle { runBlocking { list.scrollToItem(5) } }
        compose.waitForIdle()
        assertEquals(listOf(into.id), flows)

        // Chapter 13 opens with the one left kept above it, and then its images come.
        val opened = ReaderState(view = ReaderFixtures.view(mode = ReaderMode.STRIP, next = after).copy(chapter = into), previous = first)
        compose.runOnIdle { state = opened }
        compose.waitForIdle()
        assertEquals(5, list.firstVisibleItemIndex)
        compose.runOnIdle { state = opened.copy(leaves = ReaderFixtures.leaves(4)) }
        compose.waitForIdle()

        // The scroll is where chapter 13 starts, under its divider, and nothing flowed on to 14.
        val shown = list.layoutInfo.visibleItemsInfo.map { it.key }
        assertTrue("$shown", "${into.id} 0" in shown)
        assertEquals(listOf(into.id), flows)
    }
}
