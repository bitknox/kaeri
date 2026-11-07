package app.kaeri.app

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.unit.dp
import app.kaeri.eink.EinkModel
import app.kaeri.reader.phone.ReaderLauncher
import app.kaeri.reader.phone.ReaderRequest
import app.kaeri.store.AppStore
import app.kaeri.ui.reading.readerCoverKey
import app.kaeri.ui.reading.readerZoomSource
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.After
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * The reader grows out of the cover it opened from and shrinks back into it. The cover it grows
 * out of is drawn over the app, outside the app's own composition locals, so it needs its own.
 */
@RunWith(RobolectricTestRunner::class)
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
}
