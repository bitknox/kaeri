// The player's screen on the JVM, as the frame shows it: up while the model is, and gone after.

package app.kaeri.player

import android.content.pm.ActivityInfo
import android.os.Looper
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.time.Duration
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.model.Ending
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.PlayerOp
import app.kaeri.player.ui.PlayerScreen
import app.kaeri.store.AppStore
import app.kaeri.testkit.FixtureArt
import app.kaeri.ui.LocalAppStore

/** A phone held upright, as the browse screens are. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], qualifiers = "w412dp-h915dp-port-420dpi")
class PlayerScreenTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

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

    /** The screen while the model is up, as the frame's player route has it. */
    private fun shown() {
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    if (model.isPresented) PlayerScreen(model, video = { _, modifier, _ -> Box(modifier) })
                }
            }
        }
        compose.waitForIdle()
    }

    private val orientation: Int get() = compose.activity.requestedOrientation

    // A phone on its side, as the player holds it: the column keeps room for the lines and buttons to come.
    @Config(qualifiers = "w915dp-h412dp-land-420dpi")
    @Test
    fun aPlayThatNeverPlayedSaysSoWithCloseWhichClosesIt() {
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
            model.apply(Event.PlaybackEnded(false, Ending.Failed, false))
        }
        shown()
        compose.onNodeWithText("Couldn't play episode 7").assertIsDisplayed()
        compose.onNodeWithText("Choose another source").assertIsDisplayed()
        compose.onNodeWithText("Close").performClick()
        compose.waitForIdle()
        assertFalse(model.isPresented)
    }

    @Test
    fun aPlayerTheEngineClosesLeavesThePhoneUprightAgain() {
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
        }
        shown()
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE, orientation)
        compose.runOnUiThread {
            val m = mpv!!
            m.emit(MpvEvent.of(MpvNative.Event.START_FILE))
            m.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
        }
        compose.waitForIdle()
        // The episode played to its end, and the engine closed the player: nobody pressed close.
        compose.runOnUiThread { model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Close))) }
        compose.waitForIdle()
        assertFalse(model.isPresented)
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_PORTRAIT, orientation)
    }

    @Test
    fun theControlsHideWhilePlayingAndStayHiddenAsThePlayheadMoves() {
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, "Stardust Wayfarer", "7", null, 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
        }
        shown()
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
        compose.onNodeWithContentDescription("Episodes").assertIsDisplayed()

        // Three seconds untouched, and the fade after them: bare video.
        play(3_500)
        assertFalse(model.controlsVisible)
        compose.onNodeWithContentDescription("Episodes").assertDoesNotExist()

        // Playing on with nobody touching: the controls that left do not call themselves back.
        play(6_000)
        assertFalse("the controls came back with nobody there", model.controlsVisible)
        compose.onNodeWithContentDescription("Episodes").assertDoesNotExist()

        // A tap on the video still brings them up.
        compose.runOnUiThread { model.poke() }
        compose.waitForIdle()
        assertTrue(model.controlsVisible)
        compose.onNodeWithContentDescription("Episodes").assertIsDisplayed()
    }
}
