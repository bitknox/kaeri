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
import app.kaeri.engine.model.AvailabilityCase
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ServiceLink
import app.kaeri.engine.model.TitleAvailability
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

    /** Where the title is listed, landing well after its page: its line takes the room kept for it. */
    @Test
    fun nothing_below_the_actions_moves_when_the_listing_lands() = assertStill(detailMs = 304L, listedMs = 1_504L)

    /**
     * A title no source plays: the episodes say so before the page lands, and where it is listed
     * comes later in two lines, which the placeholder holds from the episodes on.
     */
    @Test
    fun nothing_below_the_actions_moves_when_a_title_no_source_plays_is_listed() =
        assertStill(detailMs = 704L, listedMs = 1_504L, unlisted = true)

    @Test
    fun the_listing_line_keeps_its_room_from_the_first_frame() {
        val (_, frames) = run(detailMs = 304L, listedMs = 1_504L)
        assertTrue("the line's placeholder at the first frame", frames.first().services > 0)
        val waiting = frames.filter { it.at < 1_504L }
        assertTrue("the line's placeholder stands until the listing lands", waiting.all { it.services > 0 })
        assertTrue("the line's placeholder gone once it has landed", frames.last().services == 0)
    }

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

    private fun assertStill(detailMs: Long, listedMs: Long = detailMs, unlisted: Boolean = false) {
        val (_, frames) = run(detailMs, listedMs, unlisted)
        val tops = frames.mapNotNull { f -> f.more?.let { f.at to it } }
        assertTrue("More about this show shows", tops.isNotEmpty())
        val last = tops.last().second
        // From the first frame it is on the page, it stands where it ends.
        for ((at, top) in tops) assertEquals("More about this show at $at ms", last, top, 0.5f)
    }

    /**
     * One frame of the page: when, how many placeholders, whether one drew, More about's top, and
     * how many of the services line's placeholders.
     */
    private data class Frame(val at: Long, val placeholders: Int, val drawn: Boolean, val more: Float?, val services: Int)

    /**
     * Opens the title from its list's row and steps it frame by frame, the page landing at
     * [detailMs] and where it is listed at [listedMs]: one service of the viewer's, a line of its own,
     * or with the episodes AniList's alone, [unlisted], a service it is listed on and the way to
     * choose services.
     */
    private fun run(detailMs: Long, listedMs: Long = detailMs, unlisted: Boolean = false): Pair<String, List<Frame>> {
        val recorded = Scene.named("watching")
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZoneId.of("Europe/Copenhagen"))
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val page = recorded.events.filterIsInstance<Event.Detail>().last { it.v1.id == TITLE && it.v1.detail?.partial != true }
        val recordedRows = recorded.events.filterIsInstance<Event.Episodes>().last { it.id == TITLE }
        val rows = if (unlisted) Event.Episodes(TITLE, recordedRows.rows.map { it.copy(metadataOnly = true) }) else recordedRows
        // Everything else the scene knew, the lists with this title's row among them.
        store.apply(
            recorded.events.filter {
                !(it is Event.Detail && it.v1.id == TITLE) && !(it is Event.Episodes && it.id == TITLE) && !(it is Event.Availability && it.v1.id == TITLE)
            },
        )
        val listed = if (unlisted) {
            TitleAvailability(
                id = TITLE,
                case = AvailabilityCase.NOT_IN_SOURCES,
                others = listOf("Lantern Library"), // i18n-skip
                chooseServices = true,
            )
        } else {
            TitleAvailability(
                id = TITLE,
                case = AvailabilityCase.PLAYS_ALSO_ON,
                alsoOn = listOf(ServiceLink(service = "Lantern Library", url = "https://lantern.example/title")), // i18n-skip
            )
        }
        assertTrue("the list's row is here", store.titles[TITLE] != null)
        compose.setContent { KaeriTheme { AppRoot(store, startTitle = TITLE) } }
        compose.mainClock.autoAdvance = false
        val more = compose.activity.getString(app.kaeri.i18n.R.string.title_more_about)
        val frames = mutableListOf<Frame>()
        var now = 0L
        while (now < SETTLE_MS) {
            if (now == EPISODES_MS) compose.runOnUiThread { store.apply(listOf(rows)) }
            if (now == detailMs) compose.runOnUiThread { store.apply(listOf(Event.Detail(page.v1))) }
            if (now == listedMs) compose.runOnUiThread { store.apply(listOf(Event.Availability(listed))) }
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
            val services = compose.onAllNodesWithTag(SERVICES_PLACEHOLDER, useUnmergedTree = true).fetchSemanticsNodes().count { node ->
                val b = node.boundsInRoot
                b.width > 0f && b.height > 0f && b.left >= window.left && b.right <= window.right
            }
            frames += Frame(now, count, drawn, top, services)
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
