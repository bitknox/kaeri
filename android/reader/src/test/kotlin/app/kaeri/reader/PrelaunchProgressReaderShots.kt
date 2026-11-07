// The prelaunch board's reader states on the phone and tablet, drawn on the JVM from made-up
// records: piece 4's server line on the page between chapters (saved, sending, offline, didn't
// answer, not saved) with its space held so Next never moves, and piece 2's demo with its credit
// capsule and its end card. Written to android/.shots/roborazzi/pl-*.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.Dp
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.DemoWorkInfo
import app.kaeri.reader.phone.PhoneEndPage
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import app.kaeri.store.ServerLine
import app.kaeri.store.ServerLines
import app.kaeri.store.WhyNot
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = PrelaunchProgressReaderShots.PHONE)
class PrelaunchProgressReaderShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val actions = PhoneReaderActions(onClose = {})
    private val host = "books.home.example"
    private val next = ReaderFixtures.chapter("local:lantern-stair/13", "13", "Night Ferry", 34u)
    private val lines = ServerLines(CoroutineScope(SupervisorJob() + Dispatchers.Main))

    private fun file(name: String) = "${roborazziSystemPropertyOutputDirectory()}/pl-$name.png"

    /** The page between chapter 12 and 13, read, AniList told, with [line] from the server. */
    private fun endPage(line: ServerLine) {
        val dir = kotlin.io.path.createTempDirectory("progress-shots").toFile()
        val first = ReaderFixtures.grey(dir, "p0", 352, 500)
        lines.stage(ReaderFixtures.CHAPTER, line, "books-home")
        val state = ReaderState(
            view = ReaderFixtures.view(next = next),
            leaves = ReaderFixtures.leaves(3),
            end = EndPageView(next, null, host),
            preview = NextPreview(next.id, first),
            read = setOf(ReaderFixtures.CHAPTER),
            readTrackers = mapOf(ReaderFixtures.CHAPTER to listOf("AniList")),
        )
        val images = PageImages().also { it.load(first) }
        compose.setContent {
            CompositionLocalProvider(LocalServerLines provides lines) {
                PhoneEndPage(state, session, actions, images)
            }
        }
        compose.waitForIdle()
    }

    private fun shoot(name: String, line: ServerLine) {
        endPage(line)
        captureScreenRoboImage(file(name))
    }

    @Test fun p4_01_phone() = shoot("p4-01-phone", ServerLine.Saved(host))
    @Test fun p4_02_phone() = shoot("p4-02-phone", ServerLine.Sending(host))
    @Test fun p4_03_phone() = shoot("p4-03-phone", ServerLine.Offline(host))
    @Test fun p4_04_phone() = shoot("p4-04-phone", ServerLine.Retrying(host))
    @Test fun p4_05_phone() = shoot("p4-05-phone", ServerLine.Failed(host, WhyNot.KEY))

    @Test @Config(qualifiers = TABLET) fun p4_01_tablet() = shoot("p4-01-tablet", ServerLine.Saved(host))
    @Test @Config(qualifiers = TABLET) fun p4_05_tablet() = shoot("p4-05-tablet", ServerLine.Failed(host, WhyNot.KEY))

    private fun nextTop(): Dp = compose.onNodeWithText("Night Ferry").getUnclippedBoundsInRoot().top

    /** Next sits in one place whatever the line says, and before any line has come. */
    @Test
    fun next_never_moves() {
        endPage(ServerLine.Saved(host))
        val saved = nextTop()
        for (line in listOf(ServerLine.Sending(host), ServerLine.Offline(host), ServerLine.Retrying(host), ServerLine.Failed(host, WhyNot.GAVE_UP))) {
            compose.runOnUiThread { lines.stage(ReaderFixtures.CHAPTER, line, "books-home") }
            compose.mainClock.advanceTimeBy(400)
            compose.waitForIdle()
            assertEquals("Next under $line", saved, nextTop())
        }
    }

    // Piece 2: the demo

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

    /** A made-up cover: a night street under a moon, in shapes only. */
    private val cover: @Composable (Modifier) -> Unit = { modifier ->
        Canvas(modifier) {
            drawRect(Brush.verticalGradient(listOf(Color(0xFF1E2550), Color(0xFF3B4790))))
            drawCircle(Color(0xFFF2C46B), radius = size.width * 0.12f, center = Offset(size.width * 0.7f, size.height * 0.25f))
            for (i in 0 until 5) {
                drawRect(Color(0xFF12152B), Offset(i * size.width / 5f, size.height * (0.7f - (i % 2) * 0.06f)), Size(size.width / 5f + 1f, size.height))
            }
        }
    }

    @Test
    fun p2_05_phone() {
        val dir = kotlin.io.path.createTempDirectory("demo-shots").toFile()
        val page = ReaderFixtures.grey(dir, "p0", 352, 500)
        val state = ReaderState(
            view = ReaderFixtures.view(start = 0u, next = null).copy(title = demo.series, demoCredit = demo),
            leaves = ReaderFixtures.leaves(3) { if (it == 0u) page else null },
        )
        val images = PageImages().also { it.load(page) }
        compose.mainClock.autoAdvance = false
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(session = session, state = state, images = images, actions = actions, novel = { _, _ -> }, cover = cover)
            }
        }
        // Within the credit's 3 s, after the chrome came in.
        compose.mainClock.advanceTimeBy(1_000)
        captureScreenRoboImage(file("p2-05-phone"))
        compose.mainClock.autoAdvance = true
    }

    @Test
    fun p2_06_phone() {
        val state = ReaderState(
            view = ReaderFixtures.view(next = null).copy(title = demo.series, demoCredit = demo),
            leaves = ReaderFixtures.leaves(3),
        )
        compose.setContent {
            PhoneEndPage(state, session, actions, PageImages(), cover = cover)
        }
        compose.waitForIdle()
        captureScreenRoboImage(file("p2-06-phone"))
    }

    companion object {
        const val PHONE = "w412dp-h915dp-port-420dpi"
        const val TABLET = "w1280dp-h800dp-land-xhdpi"
    }
}
