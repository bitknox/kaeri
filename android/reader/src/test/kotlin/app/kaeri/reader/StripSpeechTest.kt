// What a screen reader hears of a strip that flowed on: the page on screen, counted in its own
// chapter, whether that is the chapter kept above the divider or the one open below it.

package app.kaeri.reader

import android.content.Context
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.test.core.app.ApplicationProvider
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.i18n.Strings
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
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
class StripSpeechTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val dir = kotlin.io.path.createTempDirectory("strip-speech").toFile()
    private val session = ReaderSession {}
    private val next = "local:lantern-stair/13"

    private fun leaves(chapter: String, count: Int) =
        List(count) { Leaf(it.toUInt(), listOf(ReaderFixtures.part(it.toUInt(), ReaderFixtures.grey(dir, "$chapter-$it".replace('/', '-').replace(':', '-'), 400, 300))), false) } +
            Leaf(count.toUInt(), emptyList(), true)

    /** Chapter 12's three images kept above the divider, and chapter 13's five below it. */
    private fun flowed(): LazyListState {
        session.open(ReaderFixtures.WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u, mode = ReaderMode.STRIP)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, leaves(CHAPTER, 3)))
        session.jumped(session.state.value.jump!!.id)
        session.flowInto(next)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u, mode = ReaderMode.STRIP, next = null).copy(chapter = ReaderFixtures.chapter(next, "13"))))
        session.apply(ReaderEvent.Leaves(SESSION, next, leaves(next, 5)))
        val list = LazyListState()
        compose.setContent {
            val state by session.state.collectAsState()
            Box(Modifier.fillMaxSize()) {
                StripReader(
                    state = state,
                    images = PageImages(),
                    onAt = { _, _ -> },
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
        return list
    }

    private fun scrollTo(list: LazyListState, index: Int) {
        compose.runOnIdle { runBlocking { list.scrollToItem(index) } }
        compose.waitForIdle()
    }

    @Test
    fun in_the_chapter_kept_above_the_page_is_counted_in_that_chapter() {
        val list = flowed()
        scrollTo(list, 1)
        compose.onNodeWithContentDescription(Strings.readerPageOf(context, 2, 3)).assertExists()
    }

    @Test
    fun below_the_divider_the_page_is_counted_in_the_open_chapter() {
        val list = flowed()
        // Chapter 13's second image: three above, the divider, then 13's images from item 4.
        scrollTo(list, 5)
        compose.onNodeWithContentDescription(Strings.readerPageOf(context, 2, 5)).assertExists()
    }

    @Test
    fun the_image_at_the_top_is_found_in_the_right_chapter() {
        assertEquals(true to 2, stripImageAt(2, aboveCount = 3, first = 4, count = 5))
        assertNull(stripImageAt(3, aboveCount = 3, first = 4, count = 5))
        assertEquals(false to 0, stripImageAt(4, aboveCount = 3, first = 4, count = 5))
        assertNull(stripImageAt(9, aboveCount = 3, first = 4, count = 5))
        // Nothing kept above: every image is the open chapter's.
        assertEquals(false to 0, stripImageAt(0, aboveCount = 0, first = 0, count = 5))
        assertTrue(stripImageAt(5, aboveCount = 0, first = 0, count = 5) == null)
        assertFalse(stripImageAt(1, aboveCount = 0, first = 0, count = 5)?.first ?: true)
    }
}
