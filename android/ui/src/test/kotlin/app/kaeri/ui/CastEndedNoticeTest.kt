// The notice that someone else took the TV offers Play here, and still goes by itself like the
// other cast notices, and at once when a new cast starts.

package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.Msg
import app.kaeri.store.CastEnded
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class CastEndedNoticeTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val text = "Another device took the TV" // i18n-skip
    private val ended = MutableSharedFlow<CastEnded>(extraBufferCapacity = 1)
    private val startedAt = MutableStateFlow<Long?>(null)

    private fun show(timed: Boolean = true) {
        compose.mainClock.autoAdvance = false
        compose.setContent {
            KaeriTheme {
                val scope = rememberCoroutineScope()
                val snackbars = remember { Snackbars(SnackbarHostState(), scope) }
                LaunchedEffect(Unit) {
                    sayCastEnded(ended, startedAt) { snackbars.show(text, "Play here", timed = timed) {} } // i18n-skip
                }
                SnackbarHost(snackbars.host) { NoticeSnackbar(it) }
            }
        }
        compose.mainClock.advanceTimeBy(100)
    }

    private fun takeOver() {
        ended.tryEmit(CastEnded(Msg.NoticeCastTakenOver("Living room"), null, null)) // i18n-skip
        compose.mainClock.advanceTimeBy(500)
        compose.onNodeWithText(text).assertExists()
    }

    @Test
    fun the_notice_goes_by_itself_after_the_time_a_button_gets() {
        startedAt.value = 1_000
        show()
        takeOver()
        compose.mainClock.advanceTimeBy(6_000)
        compose.onNodeWithText(text).assertExists()
        compose.mainClock.advanceTimeBy(3_000)
        compose.onAllNodesWithText(text).assertCountEquals(0)
    }

    @Test
    fun a_new_cast_takes_the_notice_down_at_once() {
        startedAt.value = 1_000
        show()
        takeOver()
        // The cast that ended keeps its start: nothing new has started.
        startedAt.value = null
        compose.mainClock.advanceTimeBy(500)
        compose.onNodeWithText(text).assertExists()
        startedAt.value = 5_000
        compose.mainClock.advanceTimeBy(500)
        compose.onAllNodesWithText(text).assertCountEquals(0)
    }

    @Test
    fun a_notice_with_a_button_that_is_not_timed_still_stays() {
        show(timed = false)
        takeOver()
        compose.mainClock.advanceTimeBy(60_000)
        compose.onNodeWithText(text).assertExists()
    }
}
