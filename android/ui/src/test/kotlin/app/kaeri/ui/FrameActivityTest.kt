package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.Snapshot
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.Dp
import app.kaeri.design.KaeriTheme
import app.kaeri.i18n.R as I18nR
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assume.assumeTrue
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Home's activity line takes the account button's place in a crossfade: while both show, the
 * words sit clear of the button rather than running under it.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class FrameActivityTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val store = AppStore(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    @Test
    fun the_activity_words_sit_clear_of_the_account_button_as_they_cross() {
        assumeTrue(HAS_TESTING_ENGINE)
        var activity by mutableStateOf<String?>(null)
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    ScreenFrame(title = "Read", account = Account(name = null, open = {}), activity = activity) {}
                }
            }
        }
        compose.waitForIdle()
        compose.mainClock.autoAdvance = false
        compose.runOnUiThread {
            activity = "Loading suggestions"
            Snapshot.sendApplyNotifications()
        }
        // Part way through the crossfade, both are there.
        compose.mainClock.advanceTimeBy(100)
        val account = compose.activity.getString(I18nR.string.nav_accounts_and_settings)
        val button = compose.onNodeWithContentDescription(account).getUnclippedBoundsInRoot()
        val words = compose.onNodeWithText("Loading suggestions", useUnmergedTree = true).getUnclippedBoundsInRoot()
        assertTrue("words end at ${words.right}, the button starts at ${button.left}", words.right <= button.left + Dp(0.5f))
    }
}
