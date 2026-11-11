// The e-ink reader on a 10.3 inch panel, the Onyx Boox Note Air 4C's 992 × 1323 dp, at the panel's
// font scale of 0.85 and at 1.0, drawn on the JVM from made-up records: the bars with Download and
// the series' settings, the end of a chapter with the next one's first page and those coming up,
// and a chapter that is slow to open after 10 seconds. Written to android/.shots/roborazzi/eink-air-*.

package app.kaeri.eink

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.BookEditionRow
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterVersion
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderSettingKey as Key
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.ReaderWait
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.reader.EndPageView
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
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
@Config(sdk = [36], qualifiers = "w992dp-h1323dp-port-300dpi")
class EinkLargeReaderShots {
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
    private val painter = EinkPainter.defaults(app.kaeri.engine.model.EinkPanel.GREY, FakeRefresh())

    private fun row(key: Key, value: ReaderValue, choices: List<ReaderValue> = emptyList(), scope: SettingScope = SettingScope.MEDIUM) =
        ReaderSettingRow(key, value, scope, false, false, choices)

    private val rows = listOf(
        row(Key.MODE, ReaderValue.Choice("paged"), listOf(ReaderValue.Choice("paged"), ReaderValue.Choice("strip")), SettingScope.SERIES),
        row(Key.DIRECTION, ReaderValue.Choice("rtl"), listOf(ReaderValue.Choice("rtl"), ReaderValue.Choice("ltr")), SettingScope.SERIES),
        row(Key.PAGE_LAYOUT, ReaderValue.Choice("single"), listOf(ReaderValue.Choice("auto"), ReaderValue.Choice("single"), ReaderValue.Choice("double")), SettingScope.DEVICE),
        row(Key.CROP_BORDERS, ReaderValue.Toggle(true)),
    )

    /** Chapter 13, downloaded, after the open chapter 12; then 14 and 15 here too, and the rest not. */
    private val next = EinkFixtures.chapter("local:lantern-stair/13", "13", "Lanterns Out", 26u).copy(download = ChapterDownloadState.Done)
    private val titles = listOf(
        "The First Landing", "Rope and Rail", "A Door Ajar", "Second Wind", "Under the Arch", "Paper Moths", "The Quiet Floor", "Old Keys",
        "A Long Fall", "Counting Steps", "The Turn", "The Long Stair", "Lanterns Out", "After Dark", "The Night Porter", "Glass Rooms",
        "The Well", "A Thin Rope", "Little Hours", "Brass and Ash", "The Upper Gate", "Snowmelt", "Far Landing", "The Last Flight",
    )
    private val chapters = titles.mapIndexed { i, title ->
        val n = i + 1
        val row = EinkFixtures.chapter("local:lantern-stair/$n", "$n", title, 24u)
        if (n in 13..15) row.copy(download = ChapterDownloadState.Done) else row
    }

    private fun shot(name: String) = captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/eink-air-$name.png")

    private fun opened() {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        session.apply(ReaderEvent.Opened(EinkFixtures.view(start = 0u, next = next)))
        session.apply(ReaderEvent.Leaves(EinkFixtures.SESSION, EinkFixtures.CHAPTER, EinkFixtures.leaves(24) { EinkFixtures.grey(dir.root, "$it.gray", 70, 100) }))
        session.apply(ReaderEvent.Settings(ReaderSettingsView(Medium.MANGA, EinkFixtures.WORK, rows)))
    }

    private fun chrome(name: String) {
        showChrome()
        shot(name)
    }

    private fun showChrome() {
        opened()
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, PageImages(), painter, EinkReaderActions({}, {}, {}, onDownload = {}), showZones = false)
            }
        }
        compose.waitForIdle()
        compose.onRoot().performTouchInput { click(androidx.compose.ui.geometry.Offset(width * 0.55f, height * 0.5f)) }
        compose.waitForIdle()
    }

    private fun end(name: String) {
        opened()
        session.apply(Event.Reading(ReadingEvent.Chapters(EinkFixtures.WORK, null, listOf(VolumeGroup("2", null, chapters)), null)))
        val first = EinkFixtures.grey(dir.root, "next.gray", 70, 100)
        session.apply(ReaderEvent.NextPreview(EinkFixtures.SESSION, next.id, first))
        val images = PageImages().also { it.load(first) }
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkEndPage(
                    state.copy(end = EndPageView(next, null, null), read = state.read + EinkFixtures.CHAPTER),
                    EinkReaderActions({}, {}, {}),
                    onContinue = {},
                    images = images,
                    onContents = {},
                )
            }
        }
        compose.waitForIdle()
        shot(name)
    }

    private fun slow(name: String) {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        val first = EinkFixtures.chapter("local:lantern-stair/1", "1", "The First Landing", null).copy(volume = "1")
        session.apply(ReaderEvent.Opening(first, ReaderWait.ChapterPages("1", "alpha")))
        compose.mainClock.autoAdvance = false
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, PageImages(), painter, EinkReaderActions({}, {}, {}), showZones = false)
            }
        }
        compose.mainClock.advanceTimeBy(11_000)
        session.apply(Event.Reading(ReadingEvent.Versions(EinkFixtures.WORK, first.id, listOf(elsewhere))))
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        shot(name)
    }

    private val elsewhere = ChapterVersion(
        edition = BookEditionRow(sourceId = "beta", sourceName = "beta", language = "en", versionLabel = null, chapters = 24u, latest = "24"),
        chapterRef = "1",
        published = null,
    )

    /** One or two pages is kept for the device here: it comes after the series' choices, under "This device". */
    @Test fun pagesSayWhereTheyAreKept() {
        showChrome()
        fun at(text: String) = compose.onNodeWithText(text, useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
        val series = at("This series") // i18n-skip
        val crop = at("Crop off") // i18n-skip
        val device = at("This device") // i18n-skip
        val pages = at("One page") // i18n-skip
        assertTrue(series.right <= crop.left && crop.right <= device.left && device.right <= pages.left)
    }

    @Test @Config(fontScale = 0.85f) fun chrome_085() = chrome("chrome-085")
    @Test fun chrome_100() = chrome("chrome-100")
    @Test @Config(fontScale = 0.85f) fun end_085() = end("end-085")
    @Test fun end_100() = end("end-100")
    @Test @Config(fontScale = 0.85f) fun slow_085() = slow("slow-085")
    @Test fun slow_100() = slow("slow-100")
}
