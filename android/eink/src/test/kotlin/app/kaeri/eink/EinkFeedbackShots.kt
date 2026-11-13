// What a tap shows in the e-ink reader while the next thing loads, drawn on the JVM from made-up
// records: the end page at rest, its Continue pressed, the line naming the chapter past the quiet
// time, the chapter that failed to open, and a Contents row pressed. Written to
// android/.shots/roborazzi/eink-feedback-*.

package app.kaeri.eink

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pressKey
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
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
class EinkFeedbackShots {
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

    private fun open() {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        session.apply(ReaderEvent.Opened(EinkFixtures.view(start = 0u)))
        session.apply(ReaderEvent.Leaves(EinkFixtures.SESSION, EinkFixtures.CHAPTER, EinkFixtures.leaves(3) { EinkFixtures.grey(dir.root, "$it.gray", 80, 120) }))
        val painter = EinkPainter.defaults(app.kaeri.engine.model.EinkPanel.GREY, FakeRefresh())
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, PageImages(), painter, EinkReaderActions({}, {}, {}), showZones = false)
            }
        }
        compose.waitForIdle()
    }

    private fun shot(name: String) = captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/eink-feedback-$name.png")

    private fun toEndPage() {
        repeat(3) {
            compose.onRoot().performKeyInput { pressKey(Key.VolumeDown) }
            compose.waitForIdle()
        }
    }

    @Test
    fun next_chapter() {
        open()
        toEndPage()
        shot("1-end-page")
        compose.mainClock.autoAdvance = false
        compose.onNodeWithText("Continue to chapter 13").performClick()
        compose.mainClock.advanceTimeBy(32)
        shot("2-continue-pressed")
        compose.mainClock.advanceTimeBy(1000)
        shot("3-loading-chapter")
        compose.mainClock.autoAdvance = true
        session.apply(Event.Reading(ReadingEvent.Failed(ReadingAbout.READER, ReadingFailure.Offline)))
        compose.waitForIdle()
        shot("4-failed")
    }

    @Test
    fun contents_row() {
        open()
        compose.onRoot().performTouchInput { click(Offset(width * 0.55f, height * 0.5f)) }
        compose.waitForIdle()
        compose.onNodeWithText("Contents").performClick()
        val rows = List(20) { EinkFixtures.chapter(id = "local:lantern-stair/${it + 1}", number = "${it + 1}", title = null) }
        session.apply(Event.Reading(ReadingEvent.Chapters(EinkFixtures.WORK, null, listOf(VolumeGroup(null, null, rows, false)), null)))
        compose.waitForIdle()
        compose.mainClock.autoAdvance = false
        compose.onNodeWithText("Chapter 14").performClick()
        compose.mainClock.advanceTimeBy(32)
        shot("5-contents-row-pressed")
        compose.mainClock.advanceTimeBy(1000)
        shot("6-contents-loading")
        compose.mainClock.autoAdvance = true
    }
}
