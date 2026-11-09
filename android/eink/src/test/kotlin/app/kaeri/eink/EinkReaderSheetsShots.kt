// The e-ink reader's own sheets, drawn on the JVM from made-up records: Settings over the page,
// its More level with the screen's rows, and Contents. Written to android/.shots/roborazzi/eink-*.

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
import app.kaeri.engine.model.Event
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

    private val session = ReaderSession { }

    private fun row(key: Key, value: ReaderValue, choices: List<ReaderValue> = emptyList(), scope: SettingScope = SettingScope.MEDIUM) =
        ReaderSettingRow(key, value, scope, false, false, choices)

    private fun choices(vararg names: String) = names.map { ReaderValue.Choice(it) }

    private val rows = listOf(
        row(Key.MODE, ReaderValue.Choice("paged"), choices("paged", "strip"), SettingScope.SERIES),
        row(Key.DIRECTION, ReaderValue.Choice("rtl"), choices("rtl", "ltr"), SettingScope.SERIES),
        row(Key.FIT, ReaderValue.Choice("screen"), choices("screen", "width", "height")),
        row(Key.CROP_BORDERS, ReaderValue.Toggle(true)),
        row(Key.REFRESH_IMAGE, ReaderValue.Amount(1.0), scope = SettingScope.DEVICE),
        row(Key.VOLUME_KEYS_TURN, ReaderValue.Toggle(true), scope = SettingScope.DEVICE),
        row(Key.SPREADS, ReaderValue.Toggle(false)),
        row(Key.REFRESH_LIST, ReaderValue.Amount(6.0), scope = SettingScope.DEVICE),
        row(Key.GALLERY_SPEED, ReaderValue.Choice("standard"), choices("standard", "fast", "best"), SettingScope.DEVICE),
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
