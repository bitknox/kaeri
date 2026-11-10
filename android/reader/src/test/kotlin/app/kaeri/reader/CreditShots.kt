// Credit pages and chapter parts in the phone and tablet reader, drawn on the JVM from made-up
// records: the fixtures engine has no group that repeats a page, so the Fixtures app cannot reach
// these. Page 1's line with and without the chrome, a credit page shown and kept, the ticks where a
// part starts, the end page's line, and a strip's line, caption and divider. Written to
// android/.shots/roborazzi/credits-*.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.unit.dp
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.ChapterPart
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.CreditPage
import app.kaeri.engine.model.PageEncoding
import app.kaeri.engine.model.PageFile
import app.kaeri.engine.model.PixelFormat
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import app.kaeri.reader.ReaderFixtures.WORK
import app.kaeri.reader.phone.Chrome
import app.kaeri.reader.phone.PhoneEndPage
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import app.kaeri.reader.phone.StripCredits
import app.kaeri.reader.phone.StripDivider
import com.github.takahirom.roborazzi.captureRoboImage
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.File

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = CreditShots.PHONE)
class CreditShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    @get:Rule
    val dir = TemporaryFolder()

    private val session = ReaderSession {}
    private val actions = PhoneReaderActions(onClose = {})
    private val next = ReaderFixtures.chapter("local:lantern-stair/13", "13", "Lanterns Out", 34u)
    private val images = PageImages()

    private fun file(name: String) = "${roborazziSystemPropertyOutputDirectory()}/credits-$name.png"

    /**
     * A made-up page in grey: a story page is panels ruled on white; a credit page is a dark
     * plate with a light card and lines of type, as a group's page tends to look.
     */
    private fun page(name: String, credit: Boolean, width: Int = 352, height: Int = 500): PageFile {
        val bytes = ByteArray(width * height)
        for (y in 0 until height) for (x in 0 until width) {
            val v = if (credit) {
                val card = x in width / 6 until width * 5 / 6 && y in height / 3 until height * 2 / 3
                val type = card && (y - height / 3) % 28 in 10..16 && x in width / 4 until width * 3 / 4
                when {
                    type -> 40
                    card -> 235
                    else -> 30
                }
            } else {
                val margin = x < 16 || x >= width - 16 || y < 16 || y >= height - 16
                val gutter = y in height / 2 - 6..height / 2 + 6 || (y < height / 2 && x in width / 2 - 6..width / 2 + 6)
                val rule = !margin && !gutter && (x in 16..19 || x in width - 20 until width - 16 || y in 16..19 || y in height - 20 until height - 16 ||
                    y in height / 2 - 10..height / 2 - 7 || y in height / 2 + 7..height / 2 + 10)
                when {
                    rule -> 20
                    margin || gutter -> 250
                    else -> 250 - ((x + y) % 97) / 3
                }
            }
            bytes[y * width + x] = v.toByte()
        }
        val out = File(dir.root, name).apply { writeBytes(bytes) }
        return PageFile(out.path, PageEncoding.Raw(PixelFormat.GRAY8, width.toUInt()), width.toUInt(), height.toUInt(), null, false, bytes.size.toULong())
    }

    private val start by lazy { CreditPage(0u, false, page("credit-start", true), 11uL) }
    private val end by lazy { CreditPage(9u, true, page("credit-end", true), 22uL) }

    /** Chapter 12 in 8 story pages, a credit page before them and one after, in [mode]. */
    private fun opened(mode: ReaderMode = ReaderMode.PAGED, parts: Boolean = false) {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u, mode = mode, next = next)))
        val story = (0u until 8u).associateWith { page("p$it", false) }
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(8) { story[it] }))
        story.values.forEach(images::load)
        images.load(start.file)
        images.load(end.file)
        session.apply(ReaderEvent.CreditPages(SESSION, CHAPTER, listOf(start, end), Msg.Reading(ReadingMsg.ReaderCreditPagesSkipped(2))))
        if (parts) {
            session.apply(
                ReaderEvent.ChapterParts(
                    SESSION,
                    CHAPTER,
                    listOf(ChapterPart(1u, 1u, null, false), ChapterPart(2u, 5u, null, false)),
                    2u,
                    emptyList(),
                ),
            )
        }
    }

    private fun reader() {
        compose.mainClock.autoAdvance = false
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                val state = session.state.collectAsState().value
                PhoneReader(session = session, state = state, images = images, actions = actions, novel = { _, _ -> })
            }
        }
        settle()
    }

    private fun settle(ms: Long = 1_200) {
        compose.mainClock.advanceTimeBy(ms)
        compose.waitForIdle()
    }

    private fun chrome() {
        compose.onRoot().performTouchInput { click(Offset(width * 0.5f, height * 0.5f)) }
        settle()
    }

    private fun lineAndChrome(prefix: String) {
        opened()
        reader()
        captureScreenRoboImage(file("$prefix-line"))
        chrome()
        captureScreenRoboImage(file("$prefix-line-chrome"))
    }

    private fun shownAndKept(prefix: String) {
        opened()
        reader()
        session.showStartCredits()
        session.showCredit(0u)
        settle()
        captureScreenRoboImage(file("$prefix-shown"))
        chrome()
        captureScreenRoboImage(file("$prefix-shown-chrome"))
        chrome()
        session.keepCredit()
        settle()
        captureScreenRoboImage(file("$prefix-kept"))
    }

    @Test
    fun phone_page_one_names_the_credit_page_skipped() = lineAndChrome("phone")

    @Test
    fun phone_a_credit_page_shown_then_kept() = shownAndKept("phone")

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_page_one_names_the_credit_page_skipped() = lineAndChrome("tablet")

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_a_credit_page_shown_then_kept() = shownAndKept("tablet")

    @Test
    fun phone_a_tick_where_part_two_starts() {
        opened(parts = true)
        reader()
        chrome()
        captureScreenRoboImage(file("phone-parts-chrome"))
    }

    @Test
    fun the_end_page_names_the_page_skipped_after_the_last() {
        opened()
        compose.setContent {
            val state = session.state.collectAsState().value
            PhoneEndPage(state.copy(end = EndPageView(next, null, "Lantern Archive")), session, actions, images)
        }
        compose.waitForIdle()
        captureScreenRoboImage(file("end-page"))
    }

    @Test
    fun a_strip_names_its_credit_pages_and_shows_them_with_a_caption() {
        opened(mode = ReaderMode.STRIP)
        compose.setContent {
            val state = session.state.collectAsState().value
            Column(Modifier.fillMaxWidth().background(Chrome.ground)) {
                StripCredits(state, session, images, CHAPTER, listOf(start), Chrome.ground)
                Box(Modifier.fillMaxWidth().height(24.dp))
                Box(Modifier.fillMaxWidth().height(200.dp)) {
                    StripDivider(state.copy(end = EndPageView(next, null, "Lantern Archive")), next)
                }
            }
        }
        compose.waitForIdle()
        compose.onRoot().captureRoboImage(file("strip-line"))
        session.reveal(listOf(start), CHAPTER)
        compose.waitForIdle()
        compose.onRoot().captureRoboImage(file("strip-shown"))
    }

    companion object {
        const val PHONE = "w412dp-h915dp-port-420dpi"
        const val TABLET = "w1280dp-h800dp-land-xhdpi"
    }
}
