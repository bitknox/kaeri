// Credit pages in the e-ink reader, drawn on the JVM from made-up records: page 1's box with Show,
// the credit page shown with Not a credit page, kept with Undo, the chrome over it, and the end
// page's line. Written to android/.shots/roborazzi/eink-credits-*.

package app.kaeri.eink

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import app.kaeri.engine.model.CreditPage
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.reader.EndPageView
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
class EinkCreditShots {
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
    private val next = EinkFixtures.chapter("local:lantern-stair/13", "13", "Lanterns Out", 20u)

    private fun open(reader: Boolean = true) {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        session.apply(ReaderEvent.Opened(EinkFixtures.view(start = 0u)))
        session.apply(ReaderEvent.Leaves(EinkFixtures.SESSION, EinkFixtures.CHAPTER, EinkFixtures.leaves(3) { EinkFixtures.grey(dir.root, "$it.gray", 80, 120) }))
        val start = CreditPage(0u, false, EinkFixtures.grey(dir.root, "start.gray", 60, 90), 11uL)
        val end = CreditPage(4u, true, EinkFixtures.grey(dir.root, "end.gray", 60, 90), 22uL)
        session.apply(ReaderEvent.CreditPages(EinkFixtures.SESSION, EinkFixtures.CHAPTER, listOf(start, end), app.kaeri.engine.model.Msg.Reading(app.kaeri.engine.model.ReadingMsg.ReaderCreditPagesSkipped(2))))
        if (!reader) return
        val painter = EinkPainter.defaults(app.kaeri.engine.model.EinkPanel.GREY, FakeRefresh())
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, PageImages(), painter, EinkReaderActions({}, {}, {}), showZones = false)
            }
        }
        compose.waitForIdle()
    }

    private fun shot(name: String) = captureScreenRoboImage("${roborazziSystemPropertyOutputDirectory()}/eink-credits-$name.png")

    private fun middle() {
        compose.onRoot().performTouchInput { click(Offset(width * 0.55f, height * 0.5f)) }
        compose.waitForIdle()
    }

    @Test
    fun page_one_shown_kept() {
        open()
        shot("line")
        session.showStartCredits()
        session.showCredit(0u)
        compose.waitForIdle()
        shot("shown")
        middle()
        shot("shown-chrome")
        middle()
        session.keepCredit()
        compose.waitForIdle()
        shot("kept")
    }

    @Test
    fun the_bar_ticks_where_part_two_starts() {
        open()
        session.apply(
            ReaderEvent.ChapterParts(
                EinkFixtures.SESSION,
                EinkFixtures.CHAPTER,
                listOf(app.kaeri.engine.model.ChapterPart(1u, 1u, null, false), app.kaeri.engine.model.ChapterPart(2u, 2u, null, false)),
                2u,
                emptyList(),
            ),
        )
        middle()
        shot("parts")
    }

    @Test
    fun the_end_page_names_the_page_skipped_after_the_last() {
        open(reader = false)
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkEndPage(state.copy(end = EndPageView(next, null, null)), EinkReaderActions({}, {}, {}), onContinue = {})
            }
        }
        compose.waitForIdle()
        shot("end-page")
    }

    @Test
    fun the_end_page_offers_the_skip_past_extras_between_continue_and_back() {
        open(reader = false)
        val extra = EinkFixtures.chapter("local:lantern-stair/12.5", "12.5", "Volume 1 extras", 6u)
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkEndPage(state.copy(end = EndPageView(extra, null, null, next)), EinkReaderActions({}, {}, {}), onContinue = {})
            }
        }
        compose.waitForIdle()
        shot("end-page-skip")
    }
}
