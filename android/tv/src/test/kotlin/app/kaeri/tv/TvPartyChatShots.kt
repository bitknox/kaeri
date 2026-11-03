// The TV's watch together chat on the JVM at the board's 960 × 540: the column at the trailing
// edge, the React pill after Play and the reaction tray.

package app.kaeri.tv

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.onLast
import app.kaeri.engine.core.Translation
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.PartyChatBody
import app.kaeri.engine.model.PartyChatLine
import app.kaeri.engine.model.PartyChatState
import app.kaeri.engine.model.PartyConversation
import app.kaeri.engine.model.PartyReaction
import app.kaeri.engine.model.PartyReactionMark
import app.kaeri.engine.model.PlayerOp
import app.kaeri.engine.model.PlayerUi
import app.kaeri.engine.model.Reaction
import app.kaeri.player.MpvEvent
import app.kaeri.player.MpvNative
import app.kaeri.player.MpvValue
import app.kaeri.player.PlayerModel
import app.kaeri.player.ScriptedMpv
import app.kaeri.store.AppStore
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.PartyStates
import app.kaeri.testkit.captureRoboImage
import app.kaeri.tv.player.TvPlayerScreen
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.TvTheme
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
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
import org.robolectric.annotation.GraphicsMode

@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = TvShots.TV)
class TvPartyChatShots {
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
    private var time = 0L
    private var mpv: ScriptedMpv? = null
    private lateinit var model: PlayerModel

    @After
    fun tearDown() = scope.cancel()

    private fun said(id: ULong, from: String, text: String) = PartyChatLine(id, from, false, PartyChatBody.Said(text), false)

    private val talk = Event.PartyChat(
        PartyConversation(
            lines = listOf(said(1u, "Ren", "That lantern shot again"), said(2u, "Mio", "I was not ready for this part")),
            marks = listOf(PartyReactionMark(212.0, "Ren", Reaction.LAUGH)),
        ),
    )

    @Test
    fun tv_chat_chrome_up() = shoot("tv-chat-controls", after = {
        compose.runOnUiThread { moment() }
        compose.mainClock.advanceTimeBy(100)
    })

    @Test
    fun tv_chat_column() = shoot("tv-chat-column", hide = true)

    @Test
    fun tv_chat_tray() = shoot("tv-chat-tray", after = {
        key(androidx.compose.ui.input.key.Key.DirectionRight)
        key(androidx.compose.ui.input.key.Key.DirectionCenter)
    })

    private fun key(key: androidx.compose.ui.input.key.Key) {
        compose.onAllNodes(androidx.compose.ui.test.isRoot()).onLast().performKeyInput { pressKey(key) }
        compose.waitForIdle()
    }

    private lateinit var current: AppStore

    private fun moment() = current.apply(Event.PartyReaction(PartyReaction("Ren", false, Reaction.LAUGH, 151.0)))

    private fun shoot(name: String, hide: Boolean = false, after: () -> Unit = {}, at: (AppStore) -> Unit = {}) {
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        current = store
        model = PlayerModel(send = {}, backend = { ScriptedMpv().also { mpv = it } }, scope = scope, now = { time }, hwdec = "no")
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, "Hoshikuzu no Tabibito", "7", "The Lantern Road", 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
            model.apply(Event.PlayerUi(1u, PlayerUi.Episode("7", "Episode 7", "The Lantern Road", "Hoshikuzu no Tabibito", Translation.SUB)))
            PartyStates.inRoom().forEach { store.apply(it) }
            store.apply(Event.PartyChatState(PartyChatState(allowed = true, maxLength = 150u)))
            store.apply(talk)
        }
        val art = FixtureArt.bytes("${FixtureArt.HOST}banner/900001.jpg")!!
        val still = android.graphics.BitmapFactory.decodeByteArray(art, 0, art.size).asImageBitmap()
        compose.setContent {
            TvTheme {
                CompositionLocalProvider(LocalAppStore provides store) {
                    TvPlayerScreen(model, video = { modifier -> Image(still, null, modifier, contentScale = ContentScale.Crop) })
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
            m.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
            m.emit(MpvEvent.property("video-params", MpvValue.parse("""{"w":1920,"h":1080,"dw":1920,"dh":1080}""")!!))
            m.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
            model.poke()
            if (hide) model.hideControls()
            at(store)
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        after()
        compose.mainClock.advanceTimeBy(400)
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }
}
