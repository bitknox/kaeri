// The tap zones on e-ink, drawn on the JVM from made-up records and the engine's own cells: the
// guide for Sides right to left, Top and bottom left to right on a 7 inch screen, and the Tap zones
// page reached through Settings and More. Written to android/.shots/roborazzi/eink-tap-zones-*.

package app.kaeri.eink

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderSettingKey as Key
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.model.TapZoneSettings
import app.kaeri.engine.model.ZoneLayout
import app.kaeri.engine.model.ZoneSize
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.configName
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import org.junit.Rule
import org.junit.Test
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w674dp-h896dp-port-mdpi")
class EinkTapZonesShots {
    private val compose = createAndroidComposeRule<ComponentActivity>()
    private val dir = TemporaryFolder()

    @get:Rule
    val rules: RuleChain = RuleChain.outerRule(dir).around(
        object : ExternalResource() {
            override fun before() {
                val app = RuntimeEnvironment.getApplication()
                shadowOf(app.packageManager).addActivityIfNotPresent(ComponentName(app, ComponentActivity::class.java))
            }
        },
    ).around(compose)

    private val session = ReaderSession { }

    private fun choices(vararg names: String) = names.map { ReaderValue.Choice(it) }

    private fun zones(paged: ZoneLayout) = TapZoneSettings(
        paged = paged,
        scrolling = ZoneLayout.OFF,
        size = ZoneSize.STANDARD,
        flipSides = false,
        flipEnds = false,
        eink = true,
        swipesTurn = true,
        guideSeenPaged = "",
        guideSeenScrolling = "",
    )

    private fun settings(zones: TapZoneSettings) = ReaderSettingsView(
        Medium.MANGA,
        EinkFixtures.WORK,
        listOf(
            ReaderSettingRow(Key.MODE, ReaderValue.Choice("paged"), SettingScope.SERIES, false, false, choices("paged", "strip")),
            ReaderSettingRow(Key.DIRECTION, ReaderValue.Choice("rtl"), SettingScope.SERIES, false, false, choices("rtl", "ltr")),
            ReaderSettingRow(Key.TAP_PAGED, ReaderValue.Choice(zones.paged.configName), SettingScope.DEVICE, false, false, choices("auto", "sides", "l_shape", "wide_next", "top_bottom", "both_next", "off")),
            ReaderSettingRow(Key.TAP_SIZE, ReaderValue.Choice("standard"), SettingScope.DEVICE, false, false, choices("standard", "small")),
            ReaderSettingRow(Key.TAP_FLIP_SIDES, ReaderValue.Toggle(false), SettingScope.DEVICE, false, false, emptyList()),
            ReaderSettingRow(Key.TAP_FLIP_ENDS, ReaderValue.Toggle(false), SettingScope.DEVICE, false, false, emptyList()),
            ReaderSettingRow(Key.SWIPE_TURNS, ReaderValue.Toggle(true), SettingScope.DEVICE, false, false, emptyList()),
            ReaderSettingRow(Key.VOLUME_KEYS_TURN, ReaderValue.Toggle(true), SettingScope.DEVICE, false, false, emptyList()),
        ),
        zones,
    )

    private fun open(zones: TapZoneSettings, direction: ReadDirection = ReadDirection.RIGHT_TO_LEFT) {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        session.apply(ReaderEvent.Opened(EinkFixtures.view(start = 0u, direction = direction)))
        session.apply(ReaderEvent.Leaves(EinkFixtures.SESSION, EinkFixtures.CHAPTER, EinkFixtures.leaves(3) { EinkFixtures.grey(dir.root, "$it.gray", 80, 80) }))
        session.apply(ReaderEvent.Settings(settings(zones)))
        val painter = EinkPainter.defaults(app.kaeri.engine.model.EinkPanel.GREY, FakeRefresh())
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, PageImages(), painter, EinkReaderActions({}, {}, {}))
            }
        }
        compose.waitForIdle()
    }

    private fun shot(name: String) = captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/eink-tap-zones-$name.png")

    @Test
    fun the_guide_for_sides_right_to_left() {
        open(zones(ZoneLayout.SIDES))
        shot("guide-sides-rtl")
    }

    @Test
    fun the_guide_for_top_and_bottom_left_to_right() {
        open(zones(ZoneLayout.TOP_BOTTOM), ReadDirection.LEFT_TO_RIGHT)
        shot("guide-top-bottom-ltr")
    }

    @Test
    fun the_tap_zones_page() {
        // The first touch clears the guide and, in the middle, still opens the bars.
        open(zones(ZoneLayout.SIDES))
        compose.onRoot().performTouchInput { click(androidx.compose.ui.geometry.Offset(width * 0.55f, height * 0.5f)) }
        compose.waitForIdle()
        compose.onNodeWithText("Tap, or swipe right").assertDoesNotExist()
        compose.onNodeWithText("Settings").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("More reader settings").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Tap zones").performClick()
        compose.waitForIdle()
        shot("page-sides-rtl")
    }
}
