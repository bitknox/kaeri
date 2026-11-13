// The phone reader's states the Fixtures app cannot reach on its own, drawn on the JVM from
// made-up records: the slow open over the cover, a chapter that would not open (with and without
// another source), the end page naming the next chapter's source with its first page and while
// that page loads, the demo's card on a white page, a strip image still loading, the strip's
// divider, and a novel the web view could not show. Written to android/.shots/roborazzi/reader-*.

package app.kaeri.reader

import androidx.activity.ComponentActivity
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.unit.dp
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.BookEditionRow
import app.kaeri.engine.model.ChapterVersion
import app.kaeri.engine.model.DemoWorkInfo
import app.kaeri.reader.phone.Chrome
import app.kaeri.engine.model.ReaderWait
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.reader.phone.PhoneEndPage
import app.kaeri.reader.phone.PhoneReader
import app.kaeri.reader.phone.PhoneReaderActions
import app.kaeri.reader.phone.StripDivider
import com.github.takahirom.roborazzi.captureRoboImage
import com.github.takahirom.roborazzi.captureScreenRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class ReaderShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val session = ReaderSession {}
    private val actions = PhoneReaderActions(onClose = {})
    private val next = ReaderFixtures.chapter("local:lantern-stair/13", "13", "Lanterns Out", 34u)

    private fun file(name: String) = "${roborazziSystemPropertyOutputDirectory()}/reader-$name.png"

    private fun reader(state: ReaderState, cover: (@Composable (Modifier) -> Unit)? = null, novel: @Composable (app.kaeri.reader.phone.NovelPlace) -> Unit = {}) {
        compose.setContent {
            Box(Modifier.fillMaxSize().background(ObiSilk.Ground)) {
                PhoneReader(
                    session = session,
                    state = state,
                    images = PageImages(),
                    actions = actions,
                    novel = { place, _ -> novel(place) },
                    cover = cover,
                )
            }
        }
    }

    /** A made-up cover: dusk over a stair of lanterns, in shapes only. */
    private val cover: @Composable (Modifier) -> Unit = { modifier ->
        Canvas(modifier) {
            drawRect(Brush.verticalGradient(listOf(Color(0xFF2B2F57), Color(0xFFC8693F))))
            for (i in 0 until 7) {
                val y = size.height * (0.85f - i * 0.1f)
                val x = size.width * (0.2f + i * 0.09f)
                drawRect(Color(0xFF161A2E), Offset(x - 40f, y), androidx.compose.ui.geometry.Size(140f, 18f))
                drawCircle(Color(0xFFF2C46B), radius = 14f, center = Offset(x + 30f, y - 22f))
            }
        }
    }

    @Test
    fun opening_slowly_over_the_cover() {
        val state = ReaderState(opening = true, openingChapter = ReaderFixtures.chapter(), openingWait = ReaderWait.ChapterPages("1", "Paper Lantern"))
        compose.mainClock.autoAdvance = false
        reader(state, cover)
        compose.mainClock.advanceTimeBy(300)
        captureScreenRoboImage(file("opening-300ms"))
        compose.mainClock.advanceTimeBy(700)
        captureScreenRoboImage(file("opening-1s"))
        compose.mainClock.advanceTimeBy(3_200)
        captureScreenRoboImage(file("opening-slow-4s"))
        compose.mainClock.advanceTimeBy(12_000)
        captureScreenRoboImage(file("opening-long-16s"))
    }

    private fun failed(sources: List<ChapterVersion>?) = ReaderState(
        failure = ReadingFailure.Other("The file ended before its last page."),
        openingChapter = next,
        sources = sources,
    )

    private val elsewhere = ChapterVersion(
        edition = BookEditionRow(
            sourceId = "lantern-archive",
            sourceName = "Lantern Archive",
            language = "en",
            versionLabel = null,
            chapters = 40u,
            latest = "40",
        ),
        chapterRef = "13",
        published = null,
    )

    @Test
    fun a_chapter_that_would_not_open_with_another_source() {
        reader(failed(listOf(elsewhere)))
        compose.waitForIdle()
        captureScreenRoboImage(file("failed-choose-source"))
    }

    @Test
    fun a_chapter_that_would_not_open_anywhere_else() {
        reader(failed(emptyList()))
        compose.waitForIdle()
        captureScreenRoboImage(file("failed-try-again"))
    }

    @Test
    fun a_novel_the_web_view_could_not_show() {
        val view = ReaderFixtures.view().copy(medium = app.kaeri.engine.core.Medium.NOVEL)
        reader(ReaderState(view = view)) { place ->
            LaunchedEffect(Unit) { place.failed = "The chapter's page has no text in it." }
        }
        compose.waitForIdle()
        captureScreenRoboImage(file("novel-failed"))
    }

    @Test
    fun the_end_page_names_the_next_chapter_s_source_and_shows_its_first_page() {
        val dir = kotlin.io.path.createTempDirectory("reader-shots").toFile()
        val first = ReaderFixtures.grey(dir, "p0", 352, 500)
        val state = ReaderState(
            view = ReaderFixtures.view(next = next),
            leaves = ReaderFixtures.leaves(3),
            end = EndPageView(next, null, "Lantern Archive"),
            preview = NextPreview(next.id, first),
        )
        // Decoded before the shot, as the engine's preview is by the time the end page shows.
        val images = PageImages().also { it.load(first) }
        compose.setContent {
            PhoneEndPage(state, session, actions, images)
        }
        compose.waitForIdle()
        captureScreenRoboImage(file("end-page-source"))
    }

    @Test
    fun the_end_page_names_the_next_chapter_while_its_first_page_loads() {
        val state = ReaderState(
            view = ReaderFixtures.view(next = next),
            leaves = ReaderFixtures.leaves(3),
            end = EndPageView(next, null, "Lantern Archive"),
        )
        compose.setContent {
            PhoneEndPage(state, session, actions, PageImages())
        }
        compose.waitForIdle()
        captureScreenRoboImage(file("end-page-waiting"))
    }

    /** Chapter 12.5, an extra, with chapter 13 after it: the end page offers the skip. */
    private fun skipState(to: String = "13"): Pair<ReaderState, PageImages> {
        val dir = kotlin.io.path.createTempDirectory("reader-shots").toFile()
        val extra = ReaderFixtures.chapter("local:lantern-stair/12.5", "12.5", "Volume 1 extras", 6u)
        val whole = ReaderFixtures.chapter("local:lantern-stair/$to", to, "Lanterns Out", 34u)
        val first = ReaderFixtures.grey(dir, "p0", 352, 500)
        val state = ReaderState(
            view = ReaderFixtures.view(next = extra),
            leaves = ReaderFixtures.leaves(3),
            end = EndPageView(extra, null, "Lantern Archive", whole),
            preview = NextPreview(extra.id, first),
        )
        return state to PageImages().also { it.load(first) }
    }

    @Test
    fun the_end_page_offers_the_skip_past_extras_under_continue() {
        val (state, images) = skipState()
        compose.setContent { PhoneEndPage(state, session, actions, images) }
        compose.waitForIdle()
        captureScreenRoboImage(file("end-page-skip"))
    }

    @Test
    fun the_skip_past_extras_in_the_light_look() {
        val (state, images) = skipState()
        compose.setContent {
            CompositionLocalProvider(app.kaeri.reader.phone.LocalChrome provides app.kaeri.reader.phone.LightChrome) {
                PhoneEndPage(state, session, actions, images)
            }
        }
        compose.waitForIdle()
        captureScreenRoboImage(file("end-page-skip-light"))
    }

    @Test
    @Config(qualifiers = "w320dp-h712dp-port-420dpi")
    fun the_skip_past_extras_at_320_dp_with_a_four_digit_chapter() {
        val (state, images) = skipState(to = "1024")
        compose.setContent { PhoneEndPage(state, session, actions, images) }
        compose.waitForIdle()
        captureScreenRoboImage(file("end-page-skip-narrow"))
    }

    @Test
    fun the_demo_card_follows_a_white_page() {
        val demo = DemoWorkInfo(
            id = "lantern-demo",
            series = "Lantern Stair",
            chapter = "The First Flight",
            author = "A. Maker",
            licence = "CC BY 4.0",
            licenceUrl = null,
            makerUrl = "https://maker.example",
            filePageUrl = null,
            credit = "Lantern Stair by A. Maker. Licensed CC BY 4.0.",
            novel = false,
            pages = 3u,
        )
        val state = ReaderState(
            view = ReaderFixtures.view(next = null).copy(demoCredit = demo),
            leaves = ReaderFixtures.leaves(3),
        )
        compose.setContent {
            Box(Modifier.fillMaxSize().background(Color.White)) {
                PhoneEndPage(state, session, actions, PageImages(), pageGround = Color.White)
            }
        }
        compose.waitForIdle()
        captureScreenRoboImage(file("demo-end-white"))
    }

    @Test
    fun a_strip_image_waits_with_its_number_then_a_line_under_it() {
        compose.setContent {
            Column {
                for (long in listOf(false, true)) {
                    Box(Modifier.fillMaxWidth().height(240.dp).background(Chrome.ground)) { ImageWaiting(7, long) }
                }
            }
        }
        compose.waitForIdle()
        compose.onRoot().captureRoboImage(file("strip-waiting"))
    }

    @Test
    fun a_tall_strip_image_keeps_its_words_in_the_middle_of_the_part_on_screen() {
        compose.setContent {
            val into = with(LocalDensity.current) { 1000.dp.roundToPx() }
            Box(Modifier.fillMaxWidth().height(400.dp).testTag("window")) {
                Column(Modifier.verticalScroll(rememberScrollState(into))) {
                    Box(Modifier.fillMaxWidth().height(2400.dp)) { ImageWaiting(7, false) }
                }
            }
        }
        compose.waitForIdle()
        val window = compose.onNodeWithTag("window").getBoundsInRoot()
        val words = compose.onNodeWithText("Image 8").getBoundsInRoot()
        val middle = (words.top + words.bottom) / 2 - window.top
        assertEquals(200f, middle.value, 2f)
    }

    @Test
    fun the_strip_divider_names_the_next_episode_and_its_source() {
        val state = ReaderState(
            view = ReaderFixtures.view(next = next),
            end = EndPageView(next, null, "Lantern Archive"),
        )
        compose.setContent {
            Box(Modifier.fillMaxWidth().height(168.dp)) { StripDivider(state, next) }
        }
        compose.waitForIdle()
        compose.onRoot().captureRoboImage(file("strip-divider"))
    }
}
