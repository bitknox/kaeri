// Every notice on a phone or tablet goes with a horizontal swipe, either way, and TalkBack offers
// Dismiss on it. An info notice has no close button for that.

package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.test.swipeRight
import app.kaeri.design.KaeriTheme
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
import app.kaeri.i18n.R as I18nR

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class NoticeDismissTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val text = "Couldn't load suggestions" // i18n-skip
    private val host = SnackbarHostState()
    private var result: SnackbarResult? = null

    /** An info notice that stays until something dismisses it, so no timer can stand in for the swipe. */
    private fun show(action: String? = null) {
        lateinit var scope: CoroutineScope
        compose.setContent {
            KaeriTheme {
                scope = rememberCoroutineScope()
                SnackbarHost(host) { NoticeSnackbar(it) }
            }
        }
        compose.runOnIdle {
            scope.launch {
                result = host.showSnackbar(text, actionLabel = action, withDismissAction = action != null, duration = SnackbarDuration.Indefinite)
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText(text).assertIsDisplayed()
    }

    private fun assertGone() {
        compose.waitForIdle()
        assertEquals(SnackbarResult.Dismissed, result)
        assertNull(host.currentSnackbarData)
        compose.onAllNodesWithText(text).assertCountEquals(0)
    }

    @Test
    fun a_swipe_right_dismisses_an_info_notice() {
        show()
        compose.onNodeWithText(text).performTouchInput { swipeRight() }
        assertGone()
    }

    @Test
    fun a_swipe_left_dismisses_an_info_notice() {
        show()
        compose.onNodeWithText(text).performTouchInput { swipeLeft() }
        assertGone()
    }

    @Test
    fun a_short_drag_springs_back() {
        show()
        compose.onNodeWithText(text).performTouchInput {
            down(center)
            moveBy(Offset(20f, 0f))
            moveBy(Offset(20f, 0f))
            // Held still before letting go, so it is no fling.
            advanceEventTime(300)
            up()
        }
        compose.waitForIdle()
        assertNull(result)
        compose.onNodeWithText(text).assertIsDisplayed()
    }

    @Test
    fun talkback_offers_dismiss_on_an_info_notice() {
        show()
        val dismiss = compose.activity.getString(I18nR.string.notice_dismiss)
        val node = compose.onNodeWithText(text).fetchSemanticsNode()
        val action = node.config.getOrNull(SemanticsActions.CustomActions)?.singleOrNull { it.label == dismiss }
        assertTrue("the notice offers $dismiss", action != null)
        compose.runOnIdle { action!!.action() }
        assertGone()
    }

    @Test
    fun an_info_notice_has_no_close_button() {
        show()
        compose.onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsActions.OnClick)).assertCountEquals(0)
    }

    @Test
    fun a_notice_with_a_button_keeps_its_close_button_and_swipes() {
        show(action = "Try again") // i18n-skip
        compose.onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsActions.OnClick)).assertCountEquals(2)
        compose.onNodeWithText(text).performTouchInput { swipeRight() }
        assertGone()
    }
}
