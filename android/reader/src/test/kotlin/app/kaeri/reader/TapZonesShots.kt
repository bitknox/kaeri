// The tap zones on the phone and the tablet, drawn on the JVM from made-up records and the engine's
// own cells: the guide over a right-to-left page (Auto, so Sides), Top and bottom on a left-to-right
// page, the scrolling guide over a strip, and the Tap zones page of the settings sheet, with zones
// on and Off. Written to android/.shots/roborazzi/tap-zones-*. Also: a guide that comes back after
// a change takes the controls away, so nothing covers its labels.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.click
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.design.ObiSilk
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderSettingKey as Key
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.model.TapReading
import app.kaeri.engine.model.TapZoneSettings
import app.kaeri.engine.model.tapMap
import app.kaeri.engine.model.ZoneLayout
import app.kaeri.engine.model.ZoneSize
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import app.kaeri.reader.phone.SettingsPane
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import java.io.File
import org.junit.Rule
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The records both sizes draw from. */
internal object TapZonesScenes {
    private fun choices(vararg names: String) = names.map { ReaderValue.Choice(it) }

    fun zones(paged: ZoneLayout = ZoneLayout.AUTO, scrolling: ZoneLayout = ZoneLayout.OFF) = TapZoneSettings(
        paged = paged,
        scrolling = scrolling,
        size = ZoneSize.STANDARD,
        flipSides = false,
        flipEnds = false,
        eink = false,
        swipesTurn = true,
        guideSeenPaged = "",
        guideSeenScrolling = "",
    )

    fun settings(zones: TapZoneSettings) = ReaderSettingsView(
        Medium.MANGA,
        ReaderFixtures.WORK,
        listOf(
            ReaderSettingRow(Key.TAP_PAGED, ReaderValue.Choice(zones.paged.configName), SettingScope.DEVICE, false, false, choices("auto", "sides", "l_shape", "wide_next", "top_bottom", "both_next", "off")),
            ReaderSettingRow(Key.TAP_SCROLL, ReaderValue.Choice(zones.scrolling.configName), SettingScope.DEVICE, false, false, choices("off", "sides", "l_shape", "wide_next", "top_bottom", "both_next")),
            ReaderSettingRow(Key.TAP_SIZE, ReaderValue.Choice("standard"), SettingScope.DEVICE, false, false, choices("standard", "small")),
            ReaderSettingRow(Key.TAP_FLIP_SIDES, ReaderValue.Toggle(false), SettingScope.DEVICE, false, false, emptyList()),
            ReaderSettingRow(Key.TAP_FLIP_ENDS, ReaderValue.Toggle(false), SettingScope.DEVICE, false, false, emptyList()),
        ),
        zones,
    )

    fun state(dir: File, direction: ReadDirection = ReadDirection.RIGHT_TO_LEFT, mode: ReaderMode = ReaderMode.PAGED, zones: TapZoneSettings = zones()) = ReaderState(
        view = ReaderFixtures.view(start = 0u, direction = direction, mode = mode),
        leaves = ReaderFixtures.leaves(3) { ReaderFixtures.grey(dir, "p$it", 352, 500) },
        settings = settings(zones),
    )
}

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class TapZonesShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val dir = kotlin.io.path.createTempDirectory("tap-zones-shots").toFile()

    private fun shot(name: String) = captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/tap-zones-$name.png")

    private fun reader(state: ReaderState, pane: SettingsPane? = null) {
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = session, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> }, settingsAsked = pane)
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun the_guide_over_a_right_to_left_page() {
        reader(TapZonesScenes.state(dir))
        shot("phone-guide-auto-rtl")
    }

    @Test
    fun the_guide_for_top_and_bottom() {
        reader(TapZonesScenes.state(dir, ReadDirection.LEFT_TO_RIGHT, zones = TapZonesScenes.zones(ZoneLayout.TOP_BOTTOM)))
        shot("phone-guide-top-bottom-ltr")
    }

    @Test
    fun the_scrolling_guide_over_a_strip() {
        reader(TapZonesScenes.state(dir, mode = ReaderMode.STRIP, zones = TapZonesScenes.zones(scrolling = ZoneLayout.SIDES)))
        shot("phone-guide-strip-sides")
    }

    @Test
    fun the_tap_zones_page() {
        reader(TapZonesScenes.state(dir), SettingsPane.TapZones)
        shot("phone-page-paged-rtl")
    }

    @Test
    fun the_tap_zones_page_with_zones_off() {
        // Off has no zones to show: Show zones is drawn as off too. The page opens the sheet all the
        // way, so the button is on the screen, not below a half-open sheet.
        reader(TapZonesScenes.state(dir, zones = TapZonesScenes.zones(ZoneLayout.OFF)), SettingsPane.TapZones)
        val show = compose.onNodeWithText("Show zones").assertIsNotEnabled().fetchSemanticsNode()
        val bottom = show.positionOnScreen.y + show.size.height
        val screen = compose.activity.resources.displayMetrics.heightPixels
        assertTrue("Show zones ends at $bottom px, past the screen's $screen", show.size.height > 0 && bottom <= screen)
        shot("phone-page-off")
    }

    @Test
    fun a_guide_after_a_change_takes_the_controls_away() {
        var state by mutableStateOf(TapZonesScenes.state(dir))
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = session, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> }, settingsAsked = null)
            }
        }
        compose.waitForIdle()
        // The first touch in the middle clears the first-open guide and still opens the controls.
        compose.onRoot().performTouchInput { click(Offset(width * 0.5f, height * 0.5f)) }
        compose.waitForIdle()
        compose.waitUntil(5_000) { compose.onAllNodesWithContentDescription("Close").fetchSemanticsNodes().isNotEmpty() }
        assertTrue(compose.onAllNodesWithText("Change in Reader settings").fetchSemanticsNodes().isEmpty())

        // The engine keeps the cleared guide as seen; then new zones bring it back, over the whole
        // page, so the controls go.
        val seen = tapMap(TapZonesScenes.zones(), TapReading(ReaderMode.PAGED, ReadDirection.RIGHT_TO_LEFT, false, false), null).guide
        state = TapZonesScenes.state(dir, zones = TapZonesScenes.zones().copy(guideSeenPaged = seen))
        compose.waitForIdle()
        assertTrue(compose.onAllNodesWithContentDescription("Close").fetchSemanticsNodes().isNotEmpty())
        state = TapZonesScenes.state(dir, zones = TapZonesScenes.zones(ZoneLayout.TOP_BOTTOM).copy(guideSeenPaged = seen))
        compose.waitUntil(5_000) { compose.onAllNodesWithText("Change in Reader settings").fetchSemanticsNodes().isNotEmpty() }
        compose.waitUntil(5_000) { compose.onAllNodesWithContentDescription("Close").fetchSemanticsNodes().isEmpty() }
    }
}

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w1280dp-h800dp-land-mdpi")
class TapZonesTabletShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val dir = kotlin.io.path.createTempDirectory("tap-zones-shots").toFile()

    private fun shot(name: String) = captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/tap-zones-$name.png")

    private fun reader(state: ReaderState, pane: SettingsPane? = null) {
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = session, state = state, images = PageImages(), actions = PhoneReaderActions(onClose = {}), novel = { _, _ -> }, settingsAsked = pane)
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun the_guide_on_a_tablet() {
        reader(TapZonesScenes.state(dir, ReadDirection.LEFT_TO_RIGHT, zones = TapZonesScenes.zones(ZoneLayout.L_SHAPE)))
        shot("tablet-guide-l-shape-ltr")
    }

    @Test
    fun the_tap_zones_page_on_a_tablet() {
        reader(TapZonesScenes.state(dir, ReadDirection.LEFT_TO_RIGHT), SettingsPane.TapZones)
        shot("tablet-page-paged-ltr")
    }
}
