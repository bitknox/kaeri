package app.kaeri.ui

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTouchInput
import java.io.File
import java.time.Clock
import java.time.ZoneId
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
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
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.TitleDetail
import app.kaeri.store.AppClock
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.Scene
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.test.captureToImage

/**
 * A season change on the phone's title page, frame by frame: the clock stepped 16 ms at a time
 * from the finger landing on a season, with the engine stood in for as it answers with a real
 * AniList's latency. A page is answered [DETAIL_MS] after it is asked for, its episodes
 * [EPISODES_MS] after; the franchise's heads come as cards [DETAIL_MS] after the first page, and
 * from then on a page asked for has its head at once, from the engine's memory, as the engine
 * does. Only with KAERI_SEASON_FRAMES=1; frames go to KAERI_SEASON_FRAMES_OUT.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class SeasonSwitchFrames {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val recorded by lazy { Scene.named("watching") }
    private val out = File(System.getenv("KAERI_SEASON_FRAMES_OUT") ?: "build/season-frames")

    @Before
    fun setUp() {
        assumeTrue(HAS_TESTING_ENGINE)
        assumeTrue(System.getenv("KAERI_SEASON_FRAMES") == "1")
    }

    @After
    fun tearDown() {
        scope.cancel()
        AppClock.clock = Clock.systemDefaultZone()
    }

    @Test
    fun season_1_to_season_8() = record("far", from = 920001u, pill = "Season 8")

    @Test
    fun season_1_to_season_2() = record("neighbour", from = 920001u, pill = "Season 2")

    private fun whole(id: AnilistId): Entry? = recorded.events.filterIsInstance<Event.Detail>()
        .lastOrNull { it.v1.id == id && it.v1.detail?.partial != true }?.v1 // after-only: the partial check

    private fun rows(id: AnilistId): Event.Episodes? = recorded.events.filterIsInstance<Event.Episodes>().lastOrNull { it.id == id }

    // after-only from here
    /** The heads the engine says as cards once a page of the franchise knows its seasons. */
    private val cards: List<Entry> by lazy {
        recorded.events.filterIsInstance<Event.Cards>().flatMap { it.v1 }.filter { it.id in 920001u..920012u }.distinctBy { it.id }
    }

    /** [id]'s head sent ahead of its page, as the engine builds it from the card it kept. */
    private fun head(id: AnilistId): Entry? {
        val card = cards.firstOrNull { it.id == id } ?: return null
        val seasons = whole(920001u)?.detail?.seasons
        return card.copy(detail = TitleDetail(seasons = seasons, partial = true))
    }
    // after-only to here

    /** The engine, answering what the store asks for after the latency it would take. */
    private inner class Engine(private val store: AppStore) {
        private val due = mutableListOf<Pair<Long, Event>>()
        private var seen = 0
        private var headsKept = false

        fun headsAt(at: Long) {
            // after-only from here
            due += at to Event.Cards(cards)
            // after-only to here
        }

        fun step(now: Long) {
            val waiting = store.waiting
            for (command in waiting.drop(seen)) when (command) {
                is Command.LoadDetail -> {
                    // after-only from here
                    if (headsKept) head(command.v1)?.let { due += now to Event.Detail(it) }
                    // after-only to here
                    whole(command.v1)?.let { due += now + DETAIL_MS to Event.Detail(it) }
                }
                is Command.LoadEpisodes -> rows(command.v1)?.let { due += now + EPISODES_MS to it }
                else -> {}
            }
            seen = waiting.size
            val ready = due.filter { it.first <= now }
            if (ready.isEmpty()) return
            due.removeAll(ready)
            // after-only from here
            if (ready.any { it.second is Event.Cards }) headsKept = true
            // after-only to here
            compose.runOnUiThread { store.apply(ready.map { it.second }) }
        }
    }

    private fun record(name: String, from: AnilistId, pill: String) {
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZoneId.of("Europe/Copenhagen"))
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val engine = Engine(store)
        store.apply(listOfNotNull(whole(from)?.let { Event.Detail(it) }, rows(from)))
        engine.headsAt(DETAIL_MS)
        compose.setContent { KaeriTheme { AppRoot(store, startTitle = from) } }
        compose.mainClock.autoAdvance = false
        // The page opens and settles, its neighbours asked for and answered.
        var now = 0L
        while (now < SETTLE_MS) {
            compose.mainClock.advanceTimeBy(FRAME_MS)
            now += FRAME_MS
            engine.step(now)
        }
        // The pill brought into view, the picker's own scroll let run to its end.
        compose.mainClock.autoAdvance = true
        compose.onNodeWithText(pill).performScrollTo()
        compose.waitForIdle()
        compose.mainClock.autoAdvance = false
        repeat(60) {
            compose.mainClock.advanceTimeBy(FRAME_MS)
            now += FRAME_MS
            engine.step(now)
        }
        val dir = File(out, name).apply { deleteRecursively(); mkdirs() }
        // The finger lands, and lifts 100 ms later: a tap as a thumb makes it.
        compose.onNodeWithText(pill).performTouchInput { down(center) }
        var at = 0L
        while (at <= CLIP_MS) {
            if (at == PRESS_MS) compose.onNodeWithText(pill).performTouchInput { up() }
            val frame = compose.onRoot().captureToImage().asAndroidBitmap()
            File(dir, "frame-%04d.png".format(at)).outputStream().use { frame.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) }
            compose.mainClock.advanceTimeBy(FRAME_MS)
            at += FRAME_MS
            now += FRAME_MS
            engine.step(now)
        }
    }

    private companion object {
        const val FRAME_MS = 16L
        const val DETAIL_MS = 600L
        const val EPISODES_MS = 400L
        const val SETTLE_MS = 3_000L
        const val PRESS_MS = 96L
        const val CLIP_MS = 1_200L
    }
}
