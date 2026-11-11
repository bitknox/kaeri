package app.kaeri.app

import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.dp
import app.kaeri.eink.EinkModel
import app.kaeri.engine.model.EinkMatch
import app.kaeri.engine.model.EinkPanel
import app.kaeri.engine.model.EinkProfile
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.NovelSupport
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.VendorFamily
import app.kaeri.reader.phone.ReaderLauncher
import app.kaeri.reader.phone.ReaderRequest
import app.kaeri.store.AppStore
import app.kaeri.ui.reading.readerCoverKey
import app.kaeri.ui.reading.readerZoomSource
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The reader grows out of the cover it opened from and shrinks back into it. The cover it grows
 * out of is drawn over the app, outside the app's own composition locals, so it needs its own.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ReaderZoomTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main), artworkWithoutEngine = { null })

    @After
    fun close() {
        ReaderLauncher.close()
    }

    @Test
    fun opensFromACoverAndClosesBackIntoIt() {
        val work = "a:900301"
        val from = readerCoverKey("continue", work)
        compose.setContent {
            ReaderHost(store, EinkModel {}) {
                Box(Modifier.size(120.dp, 170.dp).readerZoomSource(from))
            }
        }
        compose.runOnIdle {
            ReaderLauncher.open(ReaderRequest(work, chapter = "c:13", cover = "https://art.example/lantern.jpg", from = from))
        }
        compose.waitForIdle()
        compose.runOnIdle { ReaderLauncher.close() }
        compose.waitForIdle()
    }

    @Test
    fun onEinkTheReaderIsThereAtOnceAndGoneAtOnce() {
        // The app under it is black; the e-ink reader opens on paper, here on the screen's
        // question, which needs no engine. A fade would leave the first frames dark, and the
        // last ones light.
        val eink = EinkModel {}
        val unsure = EinkProfile(
            eink = EinkMatch.UNSURE, panel = EinkPanel.GREY, ppi = 300u, family = VendorFamily.OTHER, refreshHook = false,
            pageButtons = false, colourSure = true, novels = NovelSupport.FULL, policy = null,
        )
        eink.apply(Event.Reading(ReadingEvent.Eink(unsure)))
        compose.setContent {
            ReaderHost(store, eink) {
                Box(Modifier.fillMaxSize().background(Color.Black))
            }
        }
        compose.waitForIdle()
        compose.mainClock.autoAdvance = false
        compose.runOnUiThread { ReaderLauncher.open(ReaderRequest("a:900301", chapter = "c:13", eink = true)) }
        compose.mainClock.advanceTimeByFrame()
        compose.mainClock.advanceTimeByFrame()
        assertTrue("the reader faded in", centre() > 200)
        compose.runOnUiThread { ReaderLauncher.close() }
        compose.mainClock.advanceTimeByFrame()
        compose.mainClock.advanceTimeByFrame()
        assertTrue("the reader faded out", centre() < 30)
    }

    /** The brightest channel of the screen's centre pixel. */
    private fun centre(): Int {
        val image = compose.onRoot().captureToImage().asAndroidBitmap()
        val pixel = image.getPixel(image.width / 2, image.height / 2)
        return maxOf(android.graphics.Color.red(pixel), android.graphics.Color.green(pixel), android.graphics.Color.blue(pixel))
    }
}
