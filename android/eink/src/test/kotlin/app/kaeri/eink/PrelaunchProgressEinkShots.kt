// The prelaunch board's e-ink reader states, drawn on the JVM from made-up records: piece 4's
// server line on the page between chapters (still, with Sending as a ring) and piece 2's demo,
// its credit in the top bar and its end. Written to android/.shots/roborazzi/pl-*.

package app.kaeri.eink

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import app.kaeri.engine.model.DemoWorkInfo
import app.kaeri.engine.model.EinkPanel
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.reader.EndPageView
import app.kaeri.reader.LocalServerLines
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.store.ServerLine
import app.kaeri.store.ServerLines
import app.kaeri.store.WhyNot
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
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
class PrelaunchProgressEinkShots {
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

    private val host = "books.home.example"
    private val actions = EinkReaderActions({}, {}, {}, {}, {})
    private val lines = ServerLines(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    private fun file(name: String) = "${roborazziSystemPropertyOutputDirectory()}/pl-$name.png"

    private fun end(name: String, line: ServerLine) {
        lines.stage(EinkFixtures.CHAPTER, line, "books-home")
        val next = EinkFixtures.chapter("local:lantern-stair/13", "13", "Night Ferry", 34u)
        val state = ReaderState(
            view = EinkFixtures.view(next = next),
            leaves = EinkFixtures.leaves(3),
            end = EndPageView(next, null, host),
            read = setOf(EinkFixtures.CHAPTER),
        )
        compose.setContent {
            EinkTheme {
                CompositionLocalProvider(LocalServerLines provides lines) {
                    EinkEndPage(state, actions, onContinue = {})
                }
            }
        }
        compose.waitForIdle()
        captureScreenRoboImage(file(name))
    }

    @Test fun p4_eink_01() = end("p4-eink-01", ServerLine.Saved(host))
    @Test fun p4_eink_02() = end("p4-eink-02", ServerLine.Sending(host))
    @Test fun p4_eink_03() = end("p4-eink-03", ServerLine.Offline(host))
    @Test fun p4_eink_04() = end("p4-eink-04", ServerLine.Retrying(host))
    @Test fun p4_eink_05() = end("p4-eink-05", ServerLine.Failed(host, WhyNot.KEY))

    private val demo = DemoWorkInfo(
        id = "star-post",
        series = "The Star Post",
        chapter = "The Night Route",
        author = "Mina Aster",
        licence = "CC BY 4.0",
        licenceUrl = "https://licences.example/by/4.0",
        makerUrl = "https://maker.example",
        filePageUrl = null,
        credit = "The Star Post by Mina Aster. Licensed CC BY 4.0.",
        novel = false,
        pages = 24u,
    )

    @Test
    fun p2_eink_05() {
        val session = ReaderSession {}
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        session.apply(ReaderEvent.Opened(EinkFixtures.view(start = 0u, next = null).copy(title = demo.series, demoCredit = demo)))
        session.apply(ReaderEvent.Leaves(EinkFixtures.SESSION, EinkFixtures.CHAPTER, EinkFixtures.leaves(3) { EinkFixtures.grey(dir.root, "$it.gray", 80, 80) }))
        val painter = EinkPainter.defaults(EinkPanel.GREY, FakeRefresh())
        compose.mainClock.autoAdvance = false
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, PageImages(), painter, actions)
            }
        }
        compose.mainClock.advanceTimeBy(1_000)
        captureScreenRoboImage(file("p2-eink-05"))
        compose.mainClock.autoAdvance = true
    }

    @Test
    fun p2_eink_06() {
        val state = ReaderState(
            view = EinkFixtures.view(next = null).copy(title = demo.series, demoCredit = demo),
            leaves = EinkFixtures.leaves(3),
        )
        compose.setContent { EinkTheme { EinkEndPage(state, actions, onContinue = {}) } }
        compose.waitForIdle()
        captureScreenRoboImage(file("p2-eink-06"))
    }
}
