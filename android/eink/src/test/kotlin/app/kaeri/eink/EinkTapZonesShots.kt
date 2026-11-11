// The tap zones on e-ink, drawn on the JVM from made-up records and the engine's own cells: the
// guide for Sides right to left, Top and bottom left to right on a 7 inch screen, and the Tap zones
// page reached through Settings and More, which fits without a scroll. Show zones is one refresh,
// and Off with swipes off says what still turns pages. Each row's line follows the size and the
// flips, and TalkBack hears a row's layout with its Selected state. Written to
// android/.shots/roborazzi/eink-tap-zones-*.

package app.kaeri.eink

import android.content.ComponentName
import android.view.View
import android.view.ViewGroup
import android.view.accessibility.AccessibilityNodeInfo
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.click
import androidx.compose.ui.test.performSemanticsAction
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
import app.kaeri.engine.model.Refresh
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.model.TapZoneSettings
import app.kaeri.engine.model.ZoneLayout
import app.kaeri.engine.model.ZoneSize
import app.kaeri.engine.model.tapMap
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.configName
import app.kaeri.reader.tapReading
import app.kaeri.reader.tapZones
import app.kaeri.reader.zoneChoices
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
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

    private val fake = FakeRefresh()

    private fun zones(paged: ZoneLayout, swipes: Boolean = true, size: ZoneSize = ZoneSize.STANDARD, flipSides: Boolean = false, flipEnds: Boolean = false) = TapZoneSettings(
        paged = paged,
        scrolling = ZoneLayout.OFF,
        size = size,
        flipSides = flipSides,
        flipEnds = flipEnds,
        eink = true,
        swipesTurn = swipes,
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
            ReaderSettingRow(Key.TAP_SIZE, ReaderValue.Choice(if (zones.size == ZoneSize.SMALL) "small" else "standard"), SettingScope.DEVICE, false, false, choices("standard", "small")),
            ReaderSettingRow(Key.TAP_FLIP_SIDES, ReaderValue.Toggle(zones.flipSides), SettingScope.DEVICE, false, false, emptyList()),
            ReaderSettingRow(Key.TAP_FLIP_ENDS, ReaderValue.Toggle(zones.flipEnds), SettingScope.DEVICE, false, false, emptyList()),
            ReaderSettingRow(Key.SWIPE_TURNS, ReaderValue.Toggle(zones.swipesTurn), SettingScope.DEVICE, false, false, emptyList()),
            ReaderSettingRow(Key.VOLUME_KEYS_TURN, ReaderValue.Toggle(true), SettingScope.DEVICE, false, false, emptyList()),
        ),
        zones,
    )

    private fun open(zones: TapZoneSettings, direction: ReadDirection = ReadDirection.RIGHT_TO_LEFT) {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        session.apply(ReaderEvent.Opened(EinkFixtures.view(start = 0u, direction = direction)))
        session.apply(ReaderEvent.Leaves(EinkFixtures.SESSION, EinkFixtures.CHAPTER, EinkFixtures.leaves(3) { EinkFixtures.grey(dir.root, "$it.gray", 80, 80) }))
        session.apply(ReaderEvent.Settings(settings(zones)))
        val painter = EinkPainter.defaults(app.kaeri.engine.model.EinkPanel.GREY, fake)
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

    /** Through the bars, Settings and More to the Tap zones page. */
    private fun tapZonesPage() {
        compose.onRoot().performTouchInput { click(androidx.compose.ui.geometry.Offset(width * 0.55f, height * 0.5f)) }
        compose.waitForIdle()
        compose.onNodeWithText("Settings").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("More reader settings").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Tap zones").performClick()
        compose.waitForIdle()
    }

    /** Whether the whole of the node with [text] is on screen, with nothing below the fold. */
    private fun wholeOnScreen(text: String): Boolean {
        val node = compose.onNodeWithText(text).fetchSemanticsNode()
        val root = compose.onRoot().fetchSemanticsNode().boundsInRoot
        return node.size.height > 0 && node.boundsInRoot.height >= node.size.height - 1f && node.boundsInRoot.bottom <= root.bottom
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
        // A full page: everything on it fits without a scroll, which on e-ink costs refreshes.
        assertTrue(wholeOnScreen("Swipes turn pages"))
        assertTrue(wholeOnScreen("Show zones"))
        shot("page-sides-rtl")
    }

    @Test
    fun show_zones_is_one_refresh() {
        open(zones(ZoneLayout.SIDES))
        tapZonesPage()
        fake.clear()
        compose.onNodeWithText("Show zones").performSemanticsAction(SemanticsActions.OnClick)
        compose.waitForIdle()
        compose.onNodeWithText("Tap, or swipe right").assertIsDisplayed()
        // The page and bars go and the guide comes in the same refresh.
        assertEquals(listOf(Refresh.REGION), fake.requests.map { it.refresh })
    }

    @Test
    fun off_with_swipes_off_says_what_still_turns_pages() {
        open(zones(ZoneLayout.OFF, swipes = false))
        tapZonesPage()
        assertTrue(notes().contains("Only the controls or keys turn pages"))
        assertTrue("Swipes and page buttons only" !in notes())
        shot("page-off-no-swipes")
    }

    /**
     * The note under each layout on the open page, worked out as the page does. A row speaks its
     * layout and zones in place of its texts, so the notes are not in the semantics tree.
     */
    private fun notes(): List<String> = compose.runOnIdle {
        val state = session.state.value
        val zones = state.tapZones(eink = true)
        val reading = state.tapReading()!!
        state.zoneChoices(Key.TAP_PAGED).map { einkTapNote(compose.activity, it, tapMap(zones, reading, it), zones.swipesTurn) }
    }

    /** What Android hands TalkBack for the node whose words start with [start]. */
    private fun info(start: String): AccessibilityNodeInfo {
        val matcher = SemanticsMatcher("words that start with $start") { node ->
            node.config.getOrNull(SemanticsProperties.ContentDescription).orEmpty().any { it.startsWith(start) }
        }
        val id = compose.onNode(matcher).fetchSemanticsNode().id
        val content = compose.activity.findViewById<ViewGroup>(android.R.id.content)
        val view = generateSequence<View>(content) { (it as? ViewGroup)?.takeIf { group -> group.childCount > 0 }?.getChildAt(0) }
            .first { it.javaClass.name.endsWith("AndroidComposeView") }
        return compose.runOnIdle { view.accessibilityNodeProvider.createAccessibilityNodeInfo(id)!! }
    }

    @Test
    fun a_layout_row_says_its_layout_and_keeps_its_selected_state() {
        open(zones(ZoneLayout.SIDES))
        tapZonesPage()
        val sides = info("Sides, ")
        assertTrue(sides.contentDescription.toString(), sides.contentDescription.toString().startsWith("Sides, Next page on the left"))
        assertEquals("Selected", sides.stateDescription?.toString())
        assertEquals("Not selected", info("Top and bottom, ").stateDescription?.toString())
    }

    @Test
    fun the_notes_follow_flip_top_and_bottom() {
        // Right to left, so Next is on the left; the rows are flipped.
        open(zones(ZoneLayout.SIDES, flipEnds = true))
        tapZonesPage()
        listOf(
            "Next on the left, the widest zone",
            "Next along the top and on the left",
            "Controls below, next takes most",
            "Next on top, previous at the bottom",
            "Previous in the top middle",
        ).forEach { assertTrue("$it is not in ${notes()}", it in notes()) }
    }

    @Test
    fun the_notes_follow_the_small_size_and_flip_left_and_right() {
        // Right to left flipped back: Next is on the right, and at the small size no zone is wider.
        open(zones(ZoneLayout.SIDES, size = ZoneSize.SMALL, flipSides = true))
        tapZonesPage()
        val notes = notes()
        listOf("Next on the right", "Next along the bottom and on the right", "Previous on top, next at the bottom")
            .forEach { assertTrue("$it is not in $notes", it in notes) }
        assertTrue(notes.joinToString(), notes.none { it.contains("the widest zone") })
    }
}
