package app.kaeri.tv

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
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
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.TitleDetail
import app.kaeri.player.PlayerModel
import app.kaeri.player.ScriptedMpv
import app.kaeri.store.AppClock
import app.kaeri.store.AppStore
import app.kaeri.store.HAS_TESTING_ENGINE
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.Scene
import app.kaeri.tv.shell.Destination
import app.kaeri.tv.shell.TvNavigator
import app.kaeri.tv.shell.TvRoot
import app.kaeri.tv.ui.TV_ART_BUDGET
import app.kaeri.tv.ui.TvArtDecoder
import app.kaeri.tv.ui.TvTheme

/**
 * A season change on the TV's title page, frame by frame, as the phone's SeasonSwitchFrames: the
 * clock stepped 16 ms at a time from the season chip's press, with the engine stood in for as it
 * answers with a real AniList's latency. Only with KAERI_SEASON_FRAMES=1; frames go to
 * KAERI_SEASON_FRAMES_OUT/tv-*.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = TvShots.TV)
class TvSeasonSwitchFrames {
    val compose = createAndroidComposeRule<ComponentActivity>()

    @get:Rule
    val rules: RuleChain = RuleChain.outerRule(
        object : ExternalResource() {
            override fun before() {
                val app = RuntimeEnvironment.getApplication()
                shadowOf(app.packageManager).addActivityIfNotPresent(ComponentName(app, ComponentActivity::class.java))
            }
        },
    ).around(compose)

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
    fun season_1_to_season_8() = record("tv-far", from = 920001u, to = 920008u)

    @Test
    fun season_1_to_season_2() = record("tv-neighbour", from = 920001u, to = 920002u)

    private fun whole(id: AnilistId): Entry? = recorded.events.filterIsInstance<Event.Detail>()
        .lastOrNull { it.v1.id == id && it.v1.detail?.partial != true }?.v1

    private fun rows(id: AnilistId): Event.Episodes? = recorded.events.filterIsInstance<Event.Episodes>().lastOrNull { it.id == id }

    /** The heads the engine says as cards once a page of the franchise knows its seasons. */
    private val cards: List<Entry> by lazy {
        recorded.events.filterIsInstance<Event.Cards>().flatMap { it.v1 }.filter { it.id in 920001u..920012u }.distinctBy { it.id }
    }

    /** [id]'s head sent ahead of its page, as the engine builds it from the card it kept. */
    private fun head(id: AnilistId): Entry? {
        val card = cards.firstOrNull { it.id == id } ?: return null
        return card.copy(detail = TitleDetail(seasons = whole(920001u)?.detail?.seasons, partial = true))
    }

    /** The engine, answering what the store asks for after the latency it would take. */
    private inner class Engine(private val store: AppStore) {
        private val due = mutableListOf<Pair<Long, Event>>()
        private var seen = 0
        private var headsKept = false

        fun headsAt(at: Long) {
            due += at to Event.Cards(cards)
        }

        fun step(now: Long) {
            val waiting = store.waiting
            for (command in waiting.drop(seen)) when (command) {
                is Command.LoadDetail -> {
                    if (headsKept) head(command.v1)?.let { due += now to Event.Detail(it) }
                    whole(command.v1)?.let { due += now + DETAIL_MS to Event.Detail(it) }
                }
                is Command.LoadEpisodes -> rows(command.v1)?.let { due += now + EPISODES_MS to it }
                else -> {}
            }
            seen = waiting.size
            val ready = due.filter { it.first <= now }
            if (ready.isEmpty()) return
            due.removeAll(ready)
            if (ready.any { it.second is Event.Cards }) headsKept = true
            compose.runOnUiThread { store.apply(ready.map { it.second }) }
        }
    }

    private fun record(name: String, from: AnilistId, to: AnilistId) {
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZoneId.of("Europe/Copenhagen"))
        val store = AppStore(scope, decoder = TvArtDecoder, artworkBudget = TV_ART_BUDGET, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val engine = Engine(store)
        store.apply(listOfNotNull(whole(from)?.let { Event.Detail(it) }, rows(from)))
        engine.headsAt(DETAIL_MS)
        val player = PlayerModel(send = {}, backend = { ScriptedMpv() }, scope = scope, now = { 0L }, hwdec = "no")
        val nav = TvNavigator(Destination.Home).apply { openTitle(from) }
        compose.setContent { TvTheme { TvRoot(store, player, nav, play = { _, _ -> }) } }
        compose.mainClock.autoAdvance = false
        var now = 0L
        while (now < SETTLE_MS) {
            compose.mainClock.advanceTimeBy(FRAME_MS)
            now += FRAME_MS
            engine.step(now)
        }
        val dir = File(out, name).apply { deleteRecursively(); mkdirs() }
        var at = 0L
        while (at <= CLIP_MS) {
            // What Center on the season's chip does.
            if (at == PRESS_MS) compose.runOnUiThread { nav.switchTitle(to) }
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
