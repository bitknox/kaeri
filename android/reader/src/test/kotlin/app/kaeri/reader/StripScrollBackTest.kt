// A strip scrolled back across the divider into the chapter kept above: once the divider has gone
// below the screen, that chapter is the one open again, so the place kept, the chapter named and
// Go to page follow what is on screen.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.junit.Assert.assertEquals
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
class StripScrollBackTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val dir = kotlin.io.path.createTempDirectory("strip-scroll-back").toFile()
    private val sent = mutableListOf<ReaderCommand>()
    private val session = ReaderSession { sent += (it as Command.Reader).v1 }
    private val next = "local:lantern-stair/13"

    private fun leaves(chapter: String) =
        List(3) { Leaf(it.toUInt(), listOf(ReaderFixtures.part(it.toUInt(), ReaderFixtures.grey(dir, "$chapter-$it".replace('/', '-').replace(':', '-'), 400, 300))), false) } +
            Leaf(3u, emptyList(), true)

    @Test
    fun scrolled_back_into_the_chapter_above_the_place_and_chapter_follow_it() {
        session.open(ReaderFixtures.WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u, mode = ReaderMode.STRIP)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, leaves(CHAPTER)))
        session.jumped(session.state.value.jump!!.id)
        // Flowed on into chapter 13 below it.
        session.flowInto(next)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u, mode = ReaderMode.STRIP, next = null).copy(chapter = ReaderFixtures.chapter(next, "13"))))
        session.apply(ReaderEvent.Leaves(SESSION, next, leaves(next)))
        assertEquals(CHAPTER, session.state.value.previous?.chapter)

        // Reading chapter 13, below the divider.
        val list = LazyListState(firstVisibleItemIndex = 5)
        lateinit var scope: CoroutineScope
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
                    onFlow = { session.flowInto(it.id) },
                    onBack = session::flowBack,
                    list = list,
                )
            }
        }
        compose.waitForIdle()
        assertTrue(sent.none { it is ReaderCommand.OpenChapter && it.chapter == CHAPTER })
        sent.clear()

        // Back up, until only chapter 12 is on screen.
        compose.runOnIdle { scope.launch { list.animateScrollToItem(0) } }
        compose.waitForIdle()
        assertEquals(ReaderCommand.OpenChapter(SESSION, CHAPTER), sent.first())
        val view = ReaderFixtures.view(start = 0u, mode = ReaderMode.STRIP)
        session.apply(ReaderEvent.Opened(view))
        compose.waitForIdle()
        // Chapter 12 is the open one, its images where they were on screen, with nothing above.
        assertEquals(CHAPTER, session.state.value.chapter)
        assertNull(session.state.value.previous)
        assertEquals(3, session.state.value.leaves.count { !it.endPage })
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, leaves(CHAPTER)))
        compose.waitForIdle()
        assertNull(session.state.value.jump)
        assertEquals(0, list.firstVisibleItemIndex)
        // The place is said in chapter 12, at its top.
        val at = sent.filterIsInstance<ReaderCommand.StripAt>().last()
        assertEquals(CHAPTER, at.chapter)
        assertEquals(0u, at.at.image)
        assertTrue(sent.none { it is ReaderCommand.OpenChapter && it.chapter == next })
    }
}
