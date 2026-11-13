// The e-ink reader's own sheets, drawn on the JVM from made-up records: Settings over the page,
// its More level with the screen's rows and no Look for manga, and Contents. Written to
// android/.shots/roborazzi/eink-*.

package app.kaeri.eink

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.test.click
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderSettingKey as Key
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
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
class EinkReaderSheetsShots {
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

    private val sent = mutableListOf<ReaderCommand>()
    private val session = ReaderSession { (it as? Command.Reader)?.let { reader -> sent += reader.v1 } }

    private fun row(key: Key, value: ReaderValue, choices: List<ReaderValue> = emptyList(), scope: SettingScope = SettingScope.MEDIUM) =
        ReaderSettingRow(key, value, scope, false, false, choices)

    private fun choices(vararg names: String) = names.map { ReaderValue.Choice(it) }

    private val rows = listOf(
        row(Key.MODE, ReaderValue.Choice("paged"), choices("paged", "strip"), SettingScope.SERIES),
        row(Key.DIRECTION, ReaderValue.Choice("rtl"), choices("rtl", "ltr"), SettingScope.SERIES),
        row(Key.FIT, ReaderValue.Choice("screen"), choices("screen", "width", "height")),
        row(Key.CROP_BORDERS, ReaderValue.Toggle(true)),
        row(Key.REFRESH_IMAGE, ReaderValue.Toggle(false), scope = SettingScope.DEVICE),
        row(Key.VOLUME_KEYS_TURN, ReaderValue.Toggle(true), scope = SettingScope.DEVICE),
        row(Key.SPREADS, ReaderValue.Toggle(false)),
        row(Key.FOOT_VALUE, ReaderValue.Choice("pages_left"), choices("pages_left", "none"), SettingScope.DEVICE),
        row(Key.REFRESH_LIST, ReaderValue.Amount(0.0), scope = SettingScope.DEVICE),
        row(Key.SCREEN_MODE, ReaderValue.Choice("device"), choices("device", "hd", "regal", "balanced", "fast"), SettingScope.DEVICE),
        row(Key.CONTRAST, ReaderValue.Amount(1.0), scope = SettingScope.DEVICE),
    )

    private fun open() {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        session.apply(ReaderEvent.Opened(EinkFixtures.view(start = 0u)))
        session.apply(ReaderEvent.Leaves(EinkFixtures.SESSION, EinkFixtures.CHAPTER, EinkFixtures.leaves(3) { EinkFixtures.grey(dir.root, "$it.gray", 80, 80) }))
        session.apply(ReaderEvent.Settings(ReaderSettingsView(Medium.MANGA, EinkFixtures.WORK, rows)))
        val painter = EinkPainter.defaults(app.kaeri.engine.model.EinkPanel.GREY, FakeRefresh())
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, PageImages(), painter, EinkReaderActions({}, {}, {}), showZones = false)
            }
        }
        compose.waitForIdle()
        compose.onRoot().performTouchInput { click(androidx.compose.ui.geometry.Offset(width * 0.55f, height * 0.5f)) }
        compose.waitForIdle()
    }

    private fun shot(name: String) = captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/eink-$name.png")

    @Test
    fun settings_and_more() {
        open()
        compose.onNodeWithText("Settings").performClick()
        compose.waitForIdle()
        shot("settings")
        compose.onNodeWithText("More reader settings").performClick()
        compose.waitForIdle()
        shot("settings-more")
    }

    @Test
    fun manga_on_paper_offers_no_look() {
        open()
        // The engine serves Look to manga; the e-ink manga reader is drawn on paper only.
        val look = row(Key.LOOK, ReaderValue.Choice("dark"), choices("paper", "dark", "system"), SettingScope.DEVICE)
        session.apply(ReaderEvent.Settings(ReaderSettingsView(Medium.MANGA, EinkFixtures.WORK, rows + look)))
        compose.onNodeWithText("Settings").performClick()
        compose.waitForIdle()
        assertTrue(compose.onAllNodes(hasText("Look")).fetchSemanticsNodes().isEmpty())
        compose.onNodeWithText("More reader settings").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Contrast").assertExists()
        assertTrue(compose.onAllNodes(hasText("Look")).fetchSemanticsNodes().isEmpty())
        assertTrue(compose.onAllNodes(hasText("Dark")).fetchSemanticsNodes().isEmpty())
    }

    @Test
    fun screen_mode_arrows_stop_at_the_first_and_last_mode() {
        open()
        compose.onNodeWithText("Settings").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("More reader settings").performClick()
        compose.waitForIdle()
        // The device's setting is the first mode: nothing before it.
        compose.onNodeWithContentDescription("Previous choice").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Next choice").assertIsEnabled().performClick()
        compose.waitForIdle()
        assertEquals(ReaderValue.Choice("hd"), sent.filterIsInstance<ReaderCommand.SetSetting>().last().value)
        // Fast is the last: nothing after it, and the arrow sends nothing.
        val last = rows.map { if (it.key == Key.SCREEN_MODE) it.copy(value = ReaderValue.Choice("fast")) else it }
        session.apply(ReaderEvent.Settings(ReaderSettingsView(Medium.MANGA, EinkFixtures.WORK, last)))
        compose.waitForIdle()
        sent.clear()
        compose.onNodeWithContentDescription("Next choice").assertIsNotEnabled().performClick()
        compose.waitForIdle()
        assertTrue(sent.none { it is ReaderCommand.SetSetting })
        compose.onNodeWithContentDescription("Previous choice").assertIsEnabled()
    }

    @Test
    fun contents() {
        open()
        compose.onNodeWithText("Contents").performClick()
        compose.waitForIdle()
        val chapters = (1..14).map { EinkFixtures.chapter("local:lantern-stair/$it", "$it", "Landing $it", 20u) }
        session.apply(
            Event.Reading(
                ReadingEvent.Chapters(
                    EinkFixtures.WORK,
                    null,
                    listOf(VolumeGroup("1", null, chapters.take(7)), VolumeGroup("2", null, chapters.drop(7))),
                    null,
                ),
            ),
        )
        compose.waitForIdle()
        shot("contents")
    }
}
