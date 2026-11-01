// The TV player's chrome on the JVM: it goes after the idle time while playing, and stays gone.

package app.kaeri.tv

import android.content.ComponentName
import android.os.Looper
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Rule
import org.junit.Test
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.time.Duration
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.PlayerOp
import app.kaeri.player.MpvEvent
import app.kaeri.player.MpvNative
import app.kaeri.player.MpvValue
import app.kaeri.player.PlayerModel
import app.kaeri.player.ScriptedMpv
import app.kaeri.store.AppStore
import app.kaeri.testkit.FixtureArt
import app.kaeri.tv.player.CHROME_HIDE
import app.kaeri.tv.player.TvPlayerScreen
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.TvTheme

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = TvShots.TV)
class TvPlayerScreenTest {
    val compose = createAndroidComposeRule<ComponentActivity>()

    /** The app's manifest has no plain activity for the rule to start; it is declared first. */
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

    @After
    fun tearDown() = scope.cancel()

    private var mpv: ScriptedMpv? = null

    /** The model's clock, which only the test moves on. */
    private var time = 0L
    private val model = PlayerModel(send = {}, backend = { ScriptedMpv().also { mpv = it } }, scope = scope, now = { time }, hwdec = "no")

    /** [millis] of playing, a quarter second at a time, with mpv saying the playhead each step. */
    private fun play(millis: Long) {
        repeat((millis / 250).toInt()) {
            time += 250
            compose.runOnUiThread { mpv!!.emit(MpvEvent.property("time-pos", MpvValue.Real(time / 1000.0))) }
            shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(250))
            compose.mainClock.advanceTimeBy(250)
        }
        compose.waitForIdle()
    }

    @Test
    fun theChromeHidesWhilePlayingAndStaysHiddenAsThePlayheadMoves() {
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
        }
        compose.setContent {
            TvTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    TvPlayerScreen(model, video = { modifier -> Box(modifier) })
                }
            }
        }
        compose.waitForIdle()
        compose.runOnUiThread {
            val m = mpv!!
            m.emit(MpvEvent.of(MpvNative.Event.START_FILE))
            m.emit(MpvEvent.of(MpvNative.Event.FILE_LOADED))
            m.emit(MpvEvent.property("duration", MpvValue.Real(1442.0)))
            m.emit(MpvEvent.property("pause", MpvValue.Flag(false)))
            m.emit(MpvEvent.property("video-params", MpvValue.parse("""{"w":1920,"h":1080,"dw":1920,"dh":1080}""")!!))
            m.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
            model.poke()
        }
        compose.waitForIdle()
        compose.onNodeWithText("Episodes").assertIsDisplayed()

        // The idle time and the fade after it: bare video.
        play(CHROME_HIDE + 500)
        compose.onNodeWithText("Episodes").assertDoesNotExist()

        // Playing on with nobody pressing a key: the chrome stays gone.
        play(2 * CHROME_HIDE)
        compose.onNodeWithText("Episodes").assertDoesNotExist()

        // A key brings it back.
        compose.runOnUiThread { model.poke() }
        compose.waitForIdle()
        compose.onNodeWithText("Episodes").assertIsDisplayed()
    }
}
