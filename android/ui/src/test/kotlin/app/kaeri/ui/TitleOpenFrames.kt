package app.kaeri.ui

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
 * A title page opened from a list, frame by frame: the list's row in the store, then its episodes
 * and its page landing after a fast or a slow AniList. Only with KAERI_TITLE_FRAMES=1; frames go to
 * KAERI_TITLE_FRAMES_OUT, one folder a run, a frame every 32 ms.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class TitleOpenFrames : TitleOpenRecording("phone")

/** [TitleOpenFrames] on a tablet across. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w1280dp-h800dp-land-xhdpi")
class TabletTitleOpenFrames : TitleOpenRecording("tablet")

abstract class TitleOpenRecording(private val device: String) {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val out = File(System.getenv("KAERI_TITLE_FRAMES_OUT") ?: "build/title-frames")

    @Before
    fun setUp() {
        assumeTrue(HAS_TESTING_ENGINE)
        assumeTrue(System.getenv("KAERI_TITLE_FRAMES") == "1")
    }

    @After
    fun tearDown() {
        scope.cancel()
        AppClock.clock = Clock.systemDefaultZone()
    }

    /** The page and its episodes inside 300 ms. */
    @Test
    fun fast() = record("fast", episodesMs = 160L, detailMs = 224L, clipMs = 1_400L)

    /** The page after 1.5 s, its episodes after 0.4 s. */
    @Test
    fun slow() = record("slow", episodesMs = 400L, detailMs = 1_504L, clipMs = 2_600L)

    private fun record(name: String, episodesMs: Long, detailMs: Long, clipMs: Long) {
        val recorded = Scene.named("watching")
        AppClock.clock = Clock.fixed(recorded.recordedAt, ZoneId.of("Europe/Copenhagen"))
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        val page = recorded.events.filterIsInstance<Event.Detail>().last { it.v1.id == TITLE && it.v1.detail?.partial != true }
        val rows = recorded.events.filterIsInstance<Event.Episodes>().last { it.id == TITLE }
        // Everything else the scene knew, the lists with this title's row among them.
        store.apply(
            recorded.events.filter {
                !(it is Event.Detail && it.v1.id == TITLE) && !(it is Event.Episodes && it.id == TITLE) && !(it is Event.Availability && it.v1.id == TITLE)
            },
        )
        // Where it is listed comes with its page, as the engine answers both from one fetch.
        val listed = TitleAvailability(
            id = TITLE,
            case = AvailabilityCase.PLAYS_ALSO_ON,
            alsoOn = listOf(ServiceLink(service = "Lantern Library", url = "https://lantern.example/title")), // i18n-skip
        )
        compose.setContent { KaeriTheme { AppRoot(store, startTitle = TITLE) } }
        compose.mainClock.autoAdvance = false
        val dir = File(out, "$device-$name").apply { deleteRecursively(); mkdirs() }
        var now = 0L
        while (now <= clipMs) {
            if (now == episodesMs) compose.runOnUiThread { store.apply(listOf(rows)) }
            if (now == detailMs) compose.runOnUiThread { store.apply(listOf(Event.Detail(page.v1), Event.Availability(listed))) }
            if (now % (2 * FRAME_MS) == 0L) {
                val frame = compose.onRoot().captureToImage().asAndroidBitmap()
                File(dir, "frame-%04d.png".format(now)).outputStream().use { frame.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) }
            }
            compose.mainClock.advanceTimeBy(FRAME_MS)
            now += FRAME_MS
        }
    }

    private companion object {
        val TITLE: AnilistId = 900001u
        const val FRAME_MS = 16L
    }
}
