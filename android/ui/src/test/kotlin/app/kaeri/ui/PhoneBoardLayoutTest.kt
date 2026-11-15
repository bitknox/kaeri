package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.dp
import kotlin.math.abs
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import app.kaeri.design.CoverPlate
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.EpisodeKind
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.FollowedMark
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE

/**
 * The phone board's direction A at 412 dp: an episode row's title keeps 224 dp with a follower on
 * the still, filler is in the facts, and a card with no banner fills its frame with the cover, or
 * with a plate in the cover's colour when there is none.
 */
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class PhoneBoardLayoutTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private lateinit var store: AppStore

    @Before
    fun setUp() {
        assumeTrue(HAS_TESTING_ENGINE)
        store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = { url -> if (url == COVER) cover() else null })
    }

    @After
    fun tearDown() {
        scope.cancel()
    }

    /** A made-up show with no banner and no still: the card's art comes from its cover. */
    private val entry = Entry(
        id = 900005u, title = "The Steel Gardener", secondary = "Hagane no Niwashi", format = "ONA", episodes = 10u,
        year = 2026u, score = null, studio = null, synopsis = "", coverUrl = COVER, coverColor = PLATE, bannerUrl = null,
        genres = emptyList(), availableOn = emptyList(), progress = null,
        airingIn = null, airingEpisode = null, nextEpisode = null, nextAiringIn = null, lastAired = null,
        resume = null, related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = null,
        titleRomaji = null, titleEnglish = null, titleNative = null, season = null, startDate = null,
        endDate = null, source = null, adult = false, detail = null,
        status = null, synopsisSpans = emptyList(),
    )

    /** A 460 by 690 cover, AniList's largest, in two greens far from the plate's red. */
    private fun cover(): ByteArray {
        val bitmap = android.graphics.Bitmap.createBitmap(460, 690, android.graphics.Bitmap.Config.ARGB_8888)
        val canvas = android.graphics.Canvas(bitmap)
        canvas.drawColor(android.graphics.Color.rgb(30, 140, 90))
        canvas.drawRect(0f, 0f, 460f, 230f, android.graphics.Paint().apply { color = android.graphics.Color.rgb(60, 200, 120) })
        return java.io.ByteArrayOutputStream().also { bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) }.toByteArray()
    }

    private fun row(kind: EpisodeKind?, followed: List<FollowedMark>) = EpisodeRow(
        number = "8", title = "The gardener's long walk to the far greenhouse", thumbnail = null, description = "Rain on the glass.",
        durationSecs = 1440u, watched = 0.0, completed = false, kind = kind, skippable = false,
        airsAt = null, synopsisPending = false, spoilerAhead = false, followed = followed,
    )

    private fun card(row: EpisodeRow) {
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    Box(Modifier.width(412.dp)) {
                        EpisodeCard(entry, row, current = true, unaired = false, hidden = false, form = RowForm.TALL, onPlay = {}, onMenu = {}, onShow = {}, followed = row.followed)
                    }
                }
            }
        }
    }

    @Test
    fun the_title_keeps_224_dp_beside_a_follower_on_the_still() {
        card(row(null, listOf(FollowedMark("Ines", null))))
        val width = compose.onNodeWithText(row(null, emptyList()).title!!, useUnmergedTree = true).fetchSemanticsNode().size.width
        // 412 less the 16 dp sides, the 144 dp still and the 12 dp gap.
        // Within a pixel: the row's sides and gap round on their own at 2.625.
        assertNear("the title's width", with(compose.density) { 224.dp.roundToPx() }, width)
    }

    @Test
    fun a_filler_episode_says_so_in_its_facts() {
        card(row(EpisodeKind.FILLER, emptyList()))
        val filler = compose.activity.getString(app.kaeri.i18n.R.string.episode_filler)
        compose.onNodeWithText(filler, substring = true, useUnmergedTree = true).fetchSemanticsNode()
    }

    /**
     * Johan's change to direction A: a row keeps the tall room while its text may still come, then
     * fits its details once they land: short with no synopsis, else the synopsis's own lines.
     */
    @Test
    fun rows_keep_the_tall_room_until_their_details_land_then_fit_them() {
        val pending = row(null, emptyList()).copy(description = null, synopsisPending = true)
        val landed = mapOf(
            114 to pending.copy(synopsisPending = false),
            142 to pending.copy(synopsisPending = false, description = "Rain on the glass."),
            182 to pending.copy(
                synopsisPending = false,
                description = "The gardener walks the long way round to the far greenhouse, counting the panes that the storm cracked, " +
                    "and finds a door that was never on the plans, with a lamp still lit behind it and a name on the frame.",
            ),
        )
        val shown = androidx.compose.runtime.mutableStateOf(pending)
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    Box(Modifier.width(412.dp).testTag("row")) {
                        EpisodeCard(
                            entry, shown.value, current = false, unaired = false, hidden = false,
                            form = rowForm(app.kaeri.engine.model.EpisodeText.EXPECTED, protectedRow = false, textless = textless(shown.value)),
                            onPlay = {}, onMenu = {}, onShow = {},
                        )
                    }
                }
            }
        }
        fun height() = compose.onNodeWithTag("row").fetchSemanticsNode().size.height
        fun px(dp: Int) = with(compose.density) { dp.dp.roundToPx() }
        for ((dp, row) in landed) {
            compose.runOnIdle { shown.value = pending }
            compose.mainClock.advanceTimeBy(1_000)
            compose.waitForIdle()
            assertNear("tall while the text may come", px(182), height())
            compose.runOnIdle { shown.value = row }
            compose.mainClock.advanceTimeBy(1_000)
            compose.waitForIdle()
            assertNear("$dp dp once the details land", px(dp), height())
            // The text fills the room it was given: every line the row made room for shows.
            val text = row.description ?: continue
            val laid = mutableListOf<androidx.compose.ui.text.TextLayoutResult>()
            compose.onNodeWithText(text.take(16), substring = true, useUnmergedTree = true).fetchSemanticsNode()
                .config[androidx.compose.ui.semantics.SemanticsActions.GetTextLayoutResult].action?.invoke(laid)
            assertEquals("the lines shown in the $dp dp row", if (dp == 142) 1 else 3, laid.first().lineCount)
        }
    }

    /**
     * Johan's report from the 0.5 build: in a table where other rows have text, a watched row and
     * the one in progress with no synopsis take the short height, not the tall room with an empty
     * band under them; the row with text takes its own line.
     */
    @Test
    fun rows_without_text_are_short_among_rows_with_text() {
        val base = row(null, emptyList()).copy(synopsisPending = false)
        val rows = listOf(
            Triple("watched", base.copy(number = "6", description = null, completed = true, watched = 1.0), false),
            Triple("in-progress", base.copy(number = "7", description = null, watched = 0.4), true),
            Triple("with-text", base.copy(number = "8", description = "Rain on the glass."), false),
        )
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    androidx.compose.foundation.layout.Column(Modifier.width(412.dp)) {
                        for ((tag, row, current) in rows) {
                            Box(Modifier.testTag(tag)) {
                                EpisodeCard(
                                    entry, row, current = current, unaired = false, hidden = false,
                                    form = rowForm(app.kaeri.engine.model.EpisodeText.PRESENT, protectedRow = false, textless = textless(row)),
                                    onPlay = {}, onMenu = {}, onShow = {},
                                )
                            }
                        }
                    }
                }
            }
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        fun px(dp: Int) = with(compose.density) { dp.dp.roundToPx() }
        for ((tag, dp) in listOf("watched" to 114, "in-progress" to 114, "with-text" to 142)) {
            assertNear("$tag row", px(dp), compose.onNodeWithTag(tag).fetchSemanticsNode().size.height)
        }
    }

    private fun assertNear(message: String, expected: Int, actual: Int) =
        assertTrue("$message: $actual px, not $expected", kotlin.math.abs(expected - actual) <= 1)

    /** The card's art, 208 by 117, as a Continue card has it, once it settles. */
    private fun art(entry: Entry, until: (androidx.compose.ui.graphics.PixelMap) -> Boolean = { true }): androidx.compose.ui.graphics.PixelMap {
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    Box(Modifier.size(208.dp, 117.dp).testTag("art")) { BannerArt(entry, Modifier.size(208.dp, 117.dp)) }
                }
            }
        }
        // The cover decodes off the main thread, then fades in on the fade-in token.
        var last = compose.onNodeWithTag("art").captureToImage().toPixelMap()
        compose.waitUntil(5_000) {
            compose.mainClock.advanceTimeBy(250)
            last = compose.onNodeWithTag("art").captureToImage().toPixelMap()
            until(last)
        }
        return last
    }

    private fun near(a: Color, b: Color) = abs(a.red - b.red) < 0.02f && abs(a.green - b.green) < 0.02f && abs(a.blue - b.blue) < 0.02f

    @Test
    fun a_card_with_no_cover_is_a_plate_in_its_colour() {
        val pixels = art(entry.copy(coverUrl = null))
        val plate = CoverPlate.muted(CoverPlate.parse(PLATE))
        // The top end corner, clear of the title at the foot.
        assertTrue("the plate", near(pixels[pixels.width * 9 / 10, pixels.height / 6], plate))
    }

    @Test
    fun a_card_with_no_banner_fills_its_frame_with_the_cover() {
        val plate = CoverPlate.muted(CoverPlate.parse(PLATE))
        val xs = { p: androidx.compose.ui.graphics.PixelMap -> listOf(2, p.width / 2, p.width - 3) }
        // The cover reaches both sides of the frame, fully in: no plate shows beside it or through it.
        val pixels = art(entry) { p -> xs(p).all { x -> p[x, p.height / 2].green > 0.4f } }
        for (x in xs(pixels)) assertTrue("the cover at $x", !near(pixels[x, pixels.height / 2], plate))
    }

    private companion object {
        const val COVER = "https://img.example/cover/900005-xl.png"
        const val PLATE = "#C04040"
    }
}
