// Keys in the long strip: Down and Up scroll a short step, Space, Shift-Space, Page Down and Page
// Up a screen less a little overlap, Home and End go to the chapter's top and the foot of its last
// image, and Left and Right do nothing. The strip has no pager, so a key sent to one moved nothing.

package app.kaeri.reader

import android.os.SystemClock
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent as ComposeKeyEvent
import androidx.compose.ui.test.KeyInjectionScope
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performKeyPress
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.withKeyDown
import androidx.compose.ui.unit.dp
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.tapScrollStep
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
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
class StripKeysTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val dir = kotlin.io.path.createTempDirectory("strip-keys").toFile()
    private val list = LazyListState()
    private val images = 5

    /** Five images twice as tall as wide, with a shape and no file, so nothing decodes. */
    private fun strip() {
        val tall = List(images) { Leaf(it.toUInt(), listOf(ReaderFixtures.part(it.toUInt()).copy(ratio = 0.5f)), false) } + Leaf(images.toUInt(), emptyList(), true)
        val state = TapZonesScenes.state(dir, mode = ReaderMode.STRIP).copy(leaves = tall)
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = ReaderSession {}, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> }, stripList = list)
            }
        }
        compose.waitForIdle()
    }

    private val imageSize get() = list.layoutInfo.visibleItemsInfo.first { it.index < images }.size

    /** How far down the strip is, in pixels from the top of the first image. */
    private fun scrolled(): Float = compose.runOnIdle { (list.firstVisibleItemIndex * imageSize + list.firstVisibleItemScrollOffset).toFloat() }

    private fun keys(block: KeyInjectionScope.() -> Unit) {
        compose.onNode(isFocused()).performKeyInput(block)
        compose.waitForIdle()
    }

    private val viewport get() = compose.runOnIdle { list.layoutInfo.viewportSize.height.toFloat() }
    private val screen get() = viewport * tapScrollStep().toFloat()
    private val line get() = with(compose.density) { 120.dp.toPx() }

    @Test
    fun down_and_up_scroll_a_short_step() {
        strip()
        keys { pressKey(Key.DirectionDown) }
        assertEquals(line, scrolled(), 2f)
        keys { pressKey(Key.DirectionDown) }
        assertEquals(2 * line, scrolled(), 2f)
        keys { pressKey(Key.DirectionUp) }
        assertEquals(line, scrolled(), 2f)
    }

    @OptIn(ExperimentalComposeUiApi::class)
    @Test
    fun space_and_the_page_keys_scroll_a_screen_less_a_little() {
        strip()
        keys { pressKey(Key.PageDown) }
        assertEquals(screen, scrolled(), 2f)
        keys { pressKey(Key.Spacebar) }
        assertEquals(2 * screen, scrolled(), 2f)
        keys { withKeyDown(Key.ShiftLeft) { pressKey(Key.Spacebar) } }
        assertEquals(screen, scrolled(), 2f)
        keys { pressKey(Key.PageUp) }
        assertEquals(0f, scrolled(), 2f)
    }

    @Test
    fun end_shows_the_foot_of_the_last_image_and_home_the_top_of_the_first() {
        strip()
        keys { pressKey(Key.MoveEnd) }
        compose.runOnIdle {
            val info = list.layoutInfo
            val last = info.visibleItemsInfo.first { it.index == images - 1 }
            assertEquals(info.viewportEndOffset, last.offset + last.size)
        }
        keys { pressKey(Key.MoveHome) }
        compose.runOnIdle {
            assertEquals(0, list.firstVisibleItemIndex)
            assertEquals(0, list.firstVisibleItemScrollOffset)
        }
    }

    @Test
    fun left_and_right_leave_the_strip_where_it_is() {
        strip()
        keys { pressKey(Key.DirectionDown) }
        keys {
            pressKey(Key.DirectionRight)
            pressKey(Key.DirectionLeft)
        }
        assertEquals(line, scrolled(), 2f)
    }

    /** One key down with [repeat] as its repeat count, as a held key sends them, then idle. */
    private fun down(code: Int, repeat: Int) {
        val now = SystemClock.uptimeMillis()
        compose.onNode(isFocused()).performKeyPress(ComposeKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_DOWN, code, repeat)))
        compose.waitForIdle()
    }

    private fun up(code: Int) {
        val now = SystemClock.uptimeMillis()
        compose.onNode(isFocused()).performKeyPress(ComposeKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_UP, code, 0)))
        compose.waitForIdle()
    }

    @Test
    fun a_held_arrow_keeps_scrolling_and_a_held_page_key_moves_once() {
        strip()
        repeat(3) { down(KeyEvent.KEYCODE_DPAD_DOWN, it) }
        up(KeyEvent.KEYCODE_DPAD_DOWN)
        assertEquals(3 * line, scrolled(), 2f)
        val held = scrolled()
        repeat(3) { down(KeyEvent.KEYCODE_PAGE_DOWN, it) }
        up(KeyEvent.KEYCODE_PAGE_DOWN)
        assertEquals(held + screen, scrolled(), 2f)
    }
}
