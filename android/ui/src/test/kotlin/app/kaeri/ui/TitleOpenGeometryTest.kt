package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onRoot
import java.time.Clock
import java.time.ZoneId
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
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.Event
import app.kaeri.store.AppClock
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.Scene

/**
 * A title page opened from a list while its page is on its way (T8): the rest of the page stands
 * where it will from the first frame it shows, and nothing on it moves when the page lands. The
 * list's row is in the store, as Home leaves it; the page and the episodes come later, as a slow
 * AniList and a source would send them. On a phone here, and on a tablet below. Unlike other
 * screens, the title page shows its placeholders from the first frame and drops them the moment the
 * page lands, with no 600 ms wait and no 0.3 s hold.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class TitleOpenGeometryTest : TitleOpenGeometry()

/** [TitleOpenGeometryTest] on a tablet across, whose page lays out beside the cover. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w1280dp-h800dp-land-xhdpi")
class TabletTitleOpenGeometryTest : TitleOpenGeometry()

abstract class TitleOpenGeometry {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @Before
    fun setUp() {
        assumeTrue(HAS_TESTING_ENGINE)
    }

    @After
    fun tearDown() {
        scope.cancel()
        AppClock.clock = Clock.systemDefaultZone()
    }

    @Test
    fun nothing_moves_when_a_slow_page_lands() = assertStill(detailMs = 1_504L)

    @Test
    fun nothing_moves_when_the_page_lands_before_its_placeholders() = assertStill(detailMs = 304L)

    @Test
    fun nothing_moves_when_the_page_lands_between_600_ms_and_the_old_hold() = assertStill(detailMs = 704L)

    @Test
    fun placeholders_show_at_the_first_frame_and_go_when_the_page_lands() {
        // Past 600 ms, where other screens would only now show their placeholders, and before
        // 900 ms, where a 0.3 s hold would have let the page in.
        val detailMs = 704L
        val (more, frames) = run(detailMs)
        val first = frames.first()
        assertTrue("placeholders at the first frame", first.placeholders > 0)
        assertTrue("placeholders drawn at the first frame", first.drawn)
        assertTrue("no More about before the page", frames.filter { it.at <= detailMs }.all { it.more == null })
        val landed = frames.first { it.more != null }.at
        assertTrue("the page replaces its placeholders at once, not after a hold: $landed ms", landed <= detailMs + 2 * FRAME_MS)
        assertTrue("More about this show stands: $more", more.isNotEmpty())
    }

    private fun assertStill(detailMs: Long) {
        val (_, frames) = run(detailMs)
        val tops = frames.mapNotNull { f -> f.more?.let { f.at to it } }
        assertTrue("More about this show shows", tops.isNotEmpty())
        val last = tops.last().second
        // From the first frame it is on the page, it stands where it ends.
        for ((at, top) in tops) assertEquals("More about this show at $at ms", last, top, 0.5f)
    }

    /** One frame of the page: when, how many placeholders, whether one drew, and More about's top. */
    private data class Frame(val at: Long, val placeholders: Int, val drawn: Boolean, val more: Float?)

    /** Opens the title from its list's row and steps it frame by frame, the page landing at [detailMs]. */
    private fun run(detailMs: Long): Pair<String, List<Frame>> {
        val recorded = Scene.named("watching")
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZoneId.of("Europe/Copenhagen"))
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val page = recorded.events.filterIsInstance<Event.Detail>().last { it.v1.id == TITLE && it.v1.detail?.partial != true }
        val rows = recorded.events.filterIsInstance<Event.Episodes>().last { it.id == TITLE }
        // Everything else the scene knew, the lists with this title's row among them.
        store.apply(recorded.events.filter { !(it is Event.Detail && it.v1.id == TITLE) && !(it is Event.Episodes && it.id == TITLE) })
        assertTrue("the list's row is here", store.titles[TITLE] != null)
        compose.setContent { KaeriTheme { AppRoot(store, startTitle = TITLE) } }
        compose.mainClock.autoAdvance = false
        val more = compose.activity.getString(app.kaeri.i18n.R.string.title_more_about)
        val frames = mutableListOf<Frame>()
        var now = 0L
        while (now < SETTLE_MS) {
            if (now == EPISODES_MS) compose.runOnUiThread { store.apply(listOf(rows)) }
            if (now == detailMs) compose.runOnUiThread { store.apply(listOf(Event.Detail(page.v1))) }
            compose.mainClock.advanceTimeBy(FRAME_MS)
            now += FRAME_MS
            val held = compose.onAllNodesWithTag(TITLE_PLACEHOLDER, useUnmergedTree = true)
            val window = compose.onRoot().fetchSemanticsNode().boundsInRoot
            // Only the page on screen: the seasons beside it are composed ahead, out of sight.
            val seen = held.fetchSemanticsNodes().withIndex().filter { (_, node) ->
                val b = node.boundsInRoot
                b.width > 0f && b.height > 0f && b.left >= window.left && b.right <= window.right && b.top >= window.top && b.bottom <= window.bottom
            }
            // A placeholder drawn shows its bar on the ground: more than one colour in its bounds.
            val drawn = frames.isEmpty() && seen.isNotEmpty() && held[seen.first().index].captureToImage().toPixelMap().let { px ->
                (0 until px.height).flatMap { y -> (0 until px.width).map { x -> px[x, y] } }.distinct().size > 1
            }
            val count = seen.size
            val top = compose.onAllNodesWithText(more).fetchSemanticsNodes().firstOrNull()?.boundsInRoot?.top
            frames += Frame(now, count, drawn, top)
        }
        return more to frames
    }

    private companion object {
        val TITLE: AnilistId = 900001u
        const val FRAME_MS = 16L
        const val EPISODES_MS = 400L
        const val SETTLE_MS = 3_200L
    }
}
