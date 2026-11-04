package app.kaeri.player

import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.MotionDurationScale
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.core.Translation
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.PartyChatBody
import app.kaeri.engine.model.PartyChatLine
import app.kaeri.engine.model.PartyChatState
import app.kaeri.engine.model.PartyConversation
import app.kaeri.engine.model.PlayerOp
import app.kaeri.engine.model.PlayerUi
import app.kaeri.player.ui.PlayerScreen
import app.kaeri.store.AppStore
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.PartyStates
import app.kaeri.ui.Destination
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Navigator
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * The chat column's motion on the phone, frame by frame, for the before and after clips of a
 * review. Only with KAERI_CHAT_FRAMES=1; KAERI_CHAT_REDUCED=1 stands in for Remove animations (the
 * animator scale at 0, as Settings and Compose see it). Frames go to KAERI_CHAT_FRAMES_OUT.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = PlayerShots.PHONE_LANDSCAPE)
class PartyChatFrames {
    private object Removed : MotionDurationScale {
        override val scaleFactor: Float get() = 0f
    }

    private val reduced = System.getenv("KAERI_CHAT_REDUCED") == "1"

    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>(effectContext = if (reduced) Removed else kotlin.coroutines.EmptyCoroutineContext)

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val out = File(System.getenv("KAERI_CHAT_FRAMES_OUT") ?: "build/chat-frames")
    private var count = 0
    private lateinit var dir: File

    @After
    fun tearDown() = scope.cancel()

    private fun said(id: ULong, from: String, text: String) = PartyChatLine(id, from, false, PartyChatBody.Said(text), false)

    private val talk = Event.PartyChat(
        PartyConversation(
            lines = listOf(
                said(1u, "Ren", "That lantern shot again"),
                said(2u, "Mio", "I was not ready for this part"),
                said(3u, "Sol", "The music here is so good"),
                said(4u, "Ren", "Wait for the next scene"),
            ),
            marks = emptyList(),
            failed = null,
        ),
    )

    /** Four lines arrive, the first three fade out together and the fourth takes the fold's place. */
    @Test
    fun column() = record("column") { frame ->
        frames(frame, 900)
        compose.mainClock.advanceTimeBy(2_600)
        frames(frame, 2_600)
    }

    /** Four lines arrive, then the Chat button opens the field: one line, no fold. */
    @Test
    fun typing() = record("typing") { frame ->
        frames(frame, 700)
        compose.onNodeWithContentDescription("Chat").performClick()
        frames(frame, 1_600)
    }

    private fun frames(frame: () -> Unit, ms: Long) {
        var at = 0L
        while (at < ms) {
            frame()
            compose.mainClock.advanceTimeBy(FRAME_MS)
            at += FRAME_MS
        }
    }

    private fun record(name: String, body: (() -> Unit) -> Unit) {
        assumeTrue(System.getenv("KAERI_CHAT_FRAMES") == "1")
        if (reduced) android.provider.Settings.Global.putFloat(compose.activity.contentResolver, android.provider.Settings.Global.ANIMATOR_DURATION_SCALE, 0f)
        dir = File(out, name + if (reduced) "-reduced" else "").apply { deleteRecursively(); mkdirs() }
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        var mpv: ScriptedMpv? = null
        val model = PlayerModel(send = {}, backend = { ScriptedMpv().also { mpv = it } }, scope = scope, now = { 0L }, hwdec = "no")
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, PlayerShots.TITLE, "7", PlayerShots.EPISODE, 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
            model.apply(Event.PlayerUi(1u, PlayerUi.Episode("7", "Episode 7", PlayerShots.EPISODE, PlayerShots.TITLE, Translation.SUB)))
            PartyStates.inRoom().forEach { store.apply(it) }
            store.apply(Event.PartyChatState(PartyChatState(allowed = true, maxLength = 150u, muted = emptyList(), waitSeconds = 0u)))
        }
        compose.waitForIdle()
        val art = FixtureArt.bytes("${FixtureArt.HOST}banner/900001.jpg")!!
        val still = android.graphics.BitmapFactory.decodeByteArray(art, 0, art.size).asImageBitmap()
        compose.setContent {
            KaeriTheme {
                val nav = androidx.navigation.compose.rememberNavController()
                CompositionLocalProvider(
                    LocalAppStore provides store,
                    LocalNavigator provides androidx.compose.runtime.remember(nav) { Navigator(nav, Destination.Home) },
                    LocalInspectionMode provides true,
                ) {
                    PlayerScreen(model, video = { _, modifier, _ ->
                        Image(still, contentDescription = null, contentScale = ContentScale.Crop, modifier = modifier)
                    })
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
            m.emit(MpvEvent.property("time-pos", MpvValue.Real(151.0)))
            m.emit(MpvEvent.of(MpvNative.Event.PLAYBACK_RESTART))
            model.poke()
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        val deadline = System.currentTimeMillis() + 10_000
        while (store.artwork.hasPending && System.currentTimeMillis() < deadline) {
            compose.waitForIdle()
            Thread.sleep(20)
        }
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
        compose.mainClock.autoAdvance = false
        compose.runOnUiThread { store.apply(talk) }
        body {
            val bitmap = compose.onRoot().captureToImage().asAndroidBitmap()
            File(dir, "frame-%04d.png".format(count++)).outputStream().use { bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) }
        }
    }

    private companion object {
        const val FRAME_MS = 33L
    }
}
