// Taps on the strip's Next zone: a second quick tap, landing while the first tap's own step still
// moves the strip, adds its step to what is left, so two taps go two steps. A strip moving by
// itself is still stopped by a touch, without a step.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.TapReading
import app.kaeri.engine.model.ZoneLayout
import app.kaeri.engine.model.tapMap
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
class StripTapTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val dir = kotlin.io.path.createTempDirectory("strip-tap").toFile()

    @Test
    fun a_second_quick_tap_adds_its_step_to_what_is_left() {
        // Top and bottom: Next is the bottom of the screen. The guide was seen, so none shows.
        val plain = TapZonesScenes.zones(scrolling = ZoneLayout.TOP_BOTTOM)
        val seen = tapMap(plain, TapReading(ReaderMode.STRIP, ReadDirection.RIGHT_TO_LEFT, false, false), null).guide
        // Two tall pages, so one image is on screen through both steps. They have a shape and no
        // file, so no decode is left running on another thread when the test ends.
        val tall = List(2) { Leaf(it.toUInt(), listOf(ReaderFixtures.part(it.toUInt()).copy(ratio = 64f / 1000f)), false) } + Leaf(2u, emptyList(), true)
        val state = TapZonesScenes.state(dir, mode = ReaderMode.STRIP, zones = plain.copy(guideSeenScrolling = seen)).copy(leaves = tall)
        val list = LazyListState()
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = session, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> }, stripList = list)
            }
        }
        compose.waitForIdle()
        val viewport = list.layoutInfo.viewportSize.height
        val image = list.layoutInfo.visibleItemsInfo.maxBy { it.size }.index
        fun top() = list.layoutInfo.visibleItemsInfo.first { it.index == image }.offset
        val start = top()

        compose.mainClock.autoAdvance = false
        compose.onRoot().performTouchInput { click(Offset(width * 0.5f, height * 0.92f)) }
        compose.mainClock.advanceTimeBy(96)
        compose.onRoot().performTouchInput { click(Offset(width * 0.5f, height * 0.92f)) }
        compose.mainClock.autoAdvance = true
        compose.waitForIdle()

        val step = viewport * tapScrollStep().toFloat()
        assertEquals(2 * step, (start - top()).toFloat(), viewport * 0.02f)
    }
}
