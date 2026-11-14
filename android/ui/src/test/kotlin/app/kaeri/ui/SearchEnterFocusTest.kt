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
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performTextInput
import androidx.test.platform.app.InstrumentationRegistry
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
import org.junit.rules.ExternalResource
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Enter in Shows' Search with a keyboard searches and leaves the focus in the field, as reading's
 * Search does, rather than on nothing, which sends the next Tab to the first control of the rail.
 * The window starts in touch mode, as a device's does.
 */
@RunWith(RobolectricTestRunner::class)
class SearchEnterFocusTest {
    @get:Rule(order = 0)
    val touchMode = object : ExternalResource() {
        override fun before() = InstrumentationRegistry.getInstrumentation().setInTouchMode(true)
    }

    @get:Rule(order = 1)
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val links = Channel<AppLink>(Channel.BUFFERED)
    private var modes: InputModeManager? = null

    @After
    fun tearDown() {
        scope.cancel()
        AppClock.clock = Clock.systemDefaultZone()
    }

    private fun showSearch() {
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
        compose.runOnIdle { assertEquals("the window starts in touch mode", InputMode.Touch, modes!!.inputMode) }
    }

    /** Types in the full-screen field and sends its search action, as Enter or the search key does. */
    private fun typeAndSearch() {
        val typing = compose.onAllNodes(hasSetTextAction() and hasAnyAncestor(isDialog())).onFirst()
        typing.performTextInput("hoshi")
        compose.waitForIdle()
        typing.performImeAction()
        compose.mainClock.advanceTimeBy(3_000)
        compose.waitForIdle()
    }

    /** With the keyboard: the full-screen field opened by a link to Search, then Enter. */
    private fun enterInSearch() {
        showSearch()
        compose.runOnIdle { modes!!.requestInputMode(InputMode.Keyboard) }
        links.trySend(AppLink.Search)
        compose.waitForIdle()
        typeAndSearch()
    }

    /**
     * The field tapped open, typed in and searched while the window stays in touch mode, as on a
     * tablet whose on-screen keyboard takes the attached keyboard's keys.
     */
    private fun searchFromATap() {
        showSearch()
        compose.onAllNodes(hasSetTextAction() and !hasAnyAncestor(isDialog())).onFirst().performClick()
        compose.waitForIdle()
        assertTrue("the tap opens the search view", compose.onAllNodes(isDialog()).fetchSemanticsNodes().isNotEmpty())
        typeAndSearch()
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

    @Test
    @Config(sdk = [36], qualifiers = "w1280dp-h800dp-land-320dpi-keysexposed-qwerty")
    fun a_search_in_touch_mode_with_a_keyboard_attached_leaves_the_focus_in_the_field() {
        searchFromATap()
        assertFieldFocused()
        compose.runOnIdle { assertEquals(InputMode.Keyboard, modes!!.inputMode) }
    }

    @Test
    @Config(sdk = [36], qualifiers = "w1280dp-h800dp-land-320dpi-nokeys")
    fun a_search_by_touch_alone_gives_the_focus_to_nothing() {
        searchFromATap()
        assertTrue("the search view closed", compose.onAllNodes(isDialog()).fetchSemanticsNodes().isEmpty())
        assertEquals("nothing has the focus", 0, compose.onAllNodes(isFocused()).fetchSemanticsNodes().size)
        compose.runOnIdle { assertEquals(InputMode.Touch, modes!!.inputMode) }
    }
}
