package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onRoot
import app.kaeri.design.KaeriTheme
import app.kaeri.eink.EinkLook
import app.kaeri.engine.model.AppEdition
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.MediumView
import app.kaeri.i18n.R
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.testkit.captureRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import kotlin.math.roundToInt
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * A launch with no answer kept, before the engine says what Kaeri is for (the launch rail board):
 * the rail, the bar and the e-ink bar stand with nothing in them, Home's placeholders beside
 * them; when the answer comes, the setup's destinations come in and the placeholders never leave.
 * No engine runs: the Medium event is laid on as the engine would send it. The frames are kept as
 * Roborazzi shots, `launch-rail-*`.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class LaunchRailTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val store = AppStore(scope)

    @Before
    fun setUp() {
        assumeTrue(HAS_TESTING_ENGINE)
    }

    @After
    fun tearDown() {
        scope.cancel()
    }

    private fun text(id: Int) = compose.activity.getString(id)

    /** Whether [words] are on screen at least once: the large top bar draws its title twice. */
    private fun shows(words: String) =
        assertTrue("$words on screen", compose.onAllNodesWithText(words).fetchSemanticsNodes().isNotEmpty())

    private fun answer(answer: KaeriIsFor) = compose.runOnUiThread {
        store.apply(Event.Medium(MediumView(answer = answer, edition = AppEdition.FULL, fixed = false, readingAvailable = true, ask = null, account = null)))
    }

    private fun launch(eink: EinkLook? = null) {
        compose.mainClock.autoAdvance = false
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(app.kaeri.ui.reading.LocalEinkLayout provides eink) {
                    AppRoot(store, Destination.Home)
                }
            }
        }
        compose.mainClock.advanceTimeBy(16)
    }

    /** Whether the spotlight's plate stands at [x], [y] dp: its raised ground, not the page's. */
    private fun plateAt(x: Int, y: Int): Boolean {
        val density = compose.activity.resources.displayMetrics.density
        val image = compose.onRoot().captureToImage().asAndroidBitmap()
        val pixel = image.getPixel((x * density).roundToInt(), (y * density).roundToInt())
        val ground = android.graphics.Color.rgb(0x16, 0x1A, 0x2E)
        return pixel != ground && android.graphics.Color.alpha(pixel) == 255
    }

    private fun shot(name: String) =
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/launch-rail-$name.png")

    @Test
    @Config(qualifiers = "w1280dp-h800dp-land-xhdpi")
    fun the_rail_waits_for_the_answer_and_home_never_empties() {
        launch()
        // The frame only: the menu and Accounts and settings, Home titled Home, no destinations.
        compose.onNodeWithContentDescription(text(R.string.nav_expand)).assertExists()
        compose.onNodeWithContentDescription(text(R.string.nav_accounts_and_settings)).assertExists()
        shows(text(R.string.home_title))
        for (label in listOf(R.string.nav_library, R.string.nav_search, R.string.nav_calendar, R.string.nav_updates)) {
            compose.onAllNodesWithText(text(label)).assertCountEquals(0)
        }
        // Watch together's button waits for the answer too.
        compose.onAllNodes(SemanticsMatcher.expectValue(SemanticsProperties.ContentDescription, listOf(text(R.string.together_people_button)))).assertCountEquals(0)
        assertTrue("no placeholders in the first 600 ms", !plateAt(400, 300))
        compose.mainClock.advanceTimeBy(1_000)
        assertTrue("Home's placeholders stand", plateAt(400, 300))
        compose.mainClock.advanceTimeBy(4_000)
        shows(text(R.string.home_loading))
        shot("tablet-neutral")

        // Read alone: its destinations come in and the page beside them stays whole at every frame.
        answer(KaeriIsFor.READ)
        repeat(40) {
            compose.mainClock.advanceTimeBy(16)
            assertTrue("the placeholders stay at frame $it", plateAt(400, 300))
            shows(text(R.string.home_title))
            shows(text(R.string.home_loading))
        }
        shows(text(R.string.nav_updates))
        shows(text(R.string.nav_library))
        compose.onAllNodesWithText(text(R.string.nav_calendar)).assertCountEquals(0)
        shot("tablet-read")
    }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-land-xhdpi")
    fun watch_together_comes_in_with_the_rail_for_watch() {
        launch()
        compose.mainClock.advanceTimeBy(1_000)
        answer(KaeriIsFor.WATCH)
        compose.mainClock.advanceTimeBy(16)
        assertTrue("the placeholders stay", plateAt(400, 300))
        compose.mainClock.advanceTimeBy(600)
        compose.onNodeWithContentDescription(text(R.string.together_people_button)).assertExists()
        shows(text(R.string.nav_calendar))
        assertTrue("the placeholders stay", plateAt(400, 300))
    }

    @Test
    @Config(qualifiers = "w412dp-h915dp-port-420dpi")
    fun the_phone_bar_keeps_its_height_with_no_items() {
        launch()
        val bar = SemanticsMatcher.keyIsDefined(SemanticsProperties.SelectableGroup)
        val empty = compose.onNode(bar, useUnmergedTree = true).getUnclippedBoundsInRoot()
        compose.onAllNodesWithText(text(R.string.nav_library)).assertCountEquals(0)
        compose.mainClock.advanceTimeBy(1_000)
        shot("phone-neutral")
        answer(KaeriIsFor.BOTH)
        compose.mainClock.advanceTimeBy(1_000)
        shows(text(R.string.nav_watch))
        shows(text(R.string.nav_read))
        val full = compose.onNode(bar, useUnmergedTree = true).getUnclippedBoundsInRoot()
        assertEquals("the bar's top", full.top, empty.top)
        assertEquals("the bar's height", full.bottom - full.top, empty.bottom - empty.top)
        shot("phone-both")
    }

    @Test
    @Config(qualifiers = "w572dp-h772dp-port-xhdpi")
    fun the_eink_bar_waits_and_the_loading_line_stays() {
        launch(EinkLook())
        val loading = text(R.string.state_loading)
        compose.onAllNodesWithText(loading).assertCountEquals(0)
        compose.onAllNodesWithText(text(R.string.nav_library)).assertCountEquals(0)
        compose.mainClock.advanceTimeBy(1_000)
        shows(loading)
        shot("eink-neutral")
        answer(KaeriIsFor.READ)
        // One picture later the e-reader's tabs are there, and the Loading line never left.
        compose.mainClock.advanceTimeBy(16)
        shows(loading)
        shows(text(R.string.nav_library))
        shows(text(R.string.eink_tab_more))
        shot("eink-read")
    }
}
