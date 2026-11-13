package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.ui.input.InputMode
import androidx.compose.ui.input.InputModeManager
import androidx.compose.ui.platform.LocalInputModeManager
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isFocused
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performTextInput
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.Event
import app.kaeri.store.AppClock
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.Scene
import java.time.Clock
import java.time.ZoneId
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.receiveAsFlow
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Enter in Shows' Search with a keyboard searches and leaves the focus in the field, as reading's
 * Search does, rather than on the first control of the rail.
 */
@RunWith(RobolectricTestRunner::class)
class SearchEnterFocusTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val links = Channel<AppLink>(Channel.BUFFERED)
    private var modes: InputModeManager? = null

    @After
    fun tearDown() {
        scope.cancel()
        AppClock.clock = Clock.systemDefaultZone()
    }

    private fun enterInSearch() {
        assumeTrue(HAS_TESTING_ENGINE)
        val recorded = Scene.named("watching")
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZoneId.of("Europe/Copenhagen"))
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        store.apply(recorded.events.filter { it !is Event.JobStarted && it !is Event.JobNote && it !is Event.JobDone && it !is Event.Toast })
        compose.setContent {
            modes = LocalInputModeManager.current
            KaeriTheme { AppRoot(store, start = Destination.Search, links = links.receiveAsFlow()) }
        }
        compose.waitForIdle()
        compose.runOnIdle { modes!!.requestInputMode(InputMode.Keyboard) }
        // The full-screen field, as tapping the field or a link to Search opens it.
        links.trySend(AppLink.Search)
        compose.waitForIdle()
        val typing = compose.onAllNodes(hasSetTextAction() and hasAnyAncestor(isDialog())).onFirst()
        typing.performTextInput("hoshi")
        compose.waitForIdle()
        typing.performImeAction()
        compose.mainClock.advanceTimeBy(3_000)
        compose.waitForIdle()
    }

    private fun assertFieldFocused() {
        assertTrue("the search view closed", compose.onAllNodes(isDialog()).fetchSemanticsNodes().isEmpty())
        val focused = compose.onAllNodes(isFocused()).fetchSemanticsNodes()
        assertEquals("one control has the focus", 1, focused.size)
        assertTrue("the focus is in the search field", compose.onAllNodes(isFocused() and hasSetTextAction()).fetchSemanticsNodes().size == 1)
    }

    @Test
    @Config(sdk = [36], qualifiers = "w1280dp-h800dp-land-320dpi")
    fun enter_on_a_tablet_leaves_the_focus_in_the_field() {
        enterInSearch()
        assertFieldFocused()
    }

    @Test
    @Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
    fun enter_on_a_phone_leaves_the_focus_in_the_field() {
        enterInSearch()
        assertFieldFocused()
    }
}
