package app.kaeri.player

import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import app.kaeri.testkit.captureRoboImage
import com.github.takahirom.roborazzi.roborazziSystemPropertyOutputDirectory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import app.kaeri.design.KaeriTheme
import app.kaeri.engine.core.Translation
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import app.kaeri.engine.model.CommentBlock
import app.kaeri.engine.model.CommentSpan
import app.kaeri.engine.model.CommentView
import app.kaeri.engine.model.DiscussionState
import app.kaeri.engine.model.DiscussionView
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.PostState
import app.kaeri.ui.Destination
import app.kaeri.ui.DiscussionSheet
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Navigator
import app.kaeri.engine.model.NextEpisode
import app.kaeri.engine.model.PlayerOp
import app.kaeri.engine.model.PlayerUi
import app.kaeri.engine.model.SkipKind
import app.kaeri.engine.model.SkipMarkFixture
import app.kaeri.engine.model.SkipNotice
import app.kaeri.engine.model.SkipOrigin
import app.kaeri.engine.model.SkipPrompt
import app.kaeri.engine.ffi.skipMarkFixture
import app.kaeri.engine.model.TimeRange
import app.kaeri.engine.model.TimelineMark
import app.kaeri.player.ui.PlayerScreen
import app.kaeri.store.AppStore
import app.kaeri.testkit.FixtureArt
import app.kaeri.ui.LocalAppStore
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.performTextInput
import app.kaeri.engine.model.PartyChatBody
import app.kaeri.engine.model.PartyChatLine
import app.kaeri.engine.model.PartyChatRefusal
import app.kaeri.engine.model.PartyChatState
import app.kaeri.engine.model.PartyConversation
import app.kaeri.engine.model.PartyReaction
import app.kaeri.engine.model.PartyReactionMark
import app.kaeri.engine.model.Reaction
import app.kaeri.testkit.PartyStates

/**
 * The watch together chat boards for the phone, drawn on the JVM at 915 × 412 dp: the column and
 * glyphs over the video, the typing bar, the people sheet, and every state the board lists.
 *
 * `./gradlew :player:recordRoborazziFixturesDebug` writes them to android/.shots/roborazzi/.
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = PlayerShots.PHONE_LANDSCAPE)
class PartyChatShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var time = 0L
    private lateinit var store: AppStore
    private lateinit var model: PlayerModel

    @After
    fun tearDown() = scope.cancel()

    private fun said(id: ULong, from: String, text: String, you: Boolean = false, muted: Boolean = false) =
        PartyChatLine(id, from, you, PartyChatBody.Said(text), muted)

    private fun lines(vararg l: PartyChatLine, marks: List<PartyReactionMark> = emptyList(), failed: String? = null) =
        Event.PartyChat(PartyConversation(lines = l.toList(), marks = marks, failed = failed))

    private val marks = listOf(
        PartyReactionMark(212.0, "Ren", Reaction.LAUGH),
        PartyReactionMark(540.0, "Mio", Reaction.GASP),
        PartyReactionMark(1020.0, "Ren", Reaction.CLAP),
    )

    private val talk = lines(
        said(1u, "Ren", "That lantern shot again"),
        said(2u, "Mio", "I was not ready for this part"),
        marks = marks,
    )

    private fun allowed(max: UInt = 150u, wait: UInt = 0u, muted: List<String> = emptyList()) =
        Event.PartyChatState(PartyChatState(allowed = true, maxLength = max, muted = muted, waitSeconds = wait))

    private fun room() = PartyStates.inRoom()

    @Test
    fun chat_controls() = shoot("chat-controls", listOf(allowed(), talk), after = {
        compose.runOnUiThread { store.apply(Event.PartyReaction(PartyReaction("Ren", false, Reaction.LAUGH, 151.0))) }
        compose.mainClock.advanceTimeBy(100)
    })

    @Test
    fun chat_column_hidden_controls() = shoot("chat-column", listOf(allowed(), talk)) { model, _ ->
        time = 1_000
        model.hideControls()
    }

    @Test
    fun chat_more_in_chat() = shoot(
        "chat-more",
        listOf(
            allowed(),
            lines(said(1u, "Ren", "one"), said(2u, "Mio", "two"), said(3u, "Ren", "three"), said(4u, "Mio", "four"), said(5u, "Ren", "five")),
        ),
    ) { model, _ ->
        time = 1_000
        model.hideControls()
    }

    @Test
    fun chat_typing() = shoot("chat-typing", listOf(allowed(), talk), after = {
        compose.onNodeWithContentDescription("Chat").performClick()
        compose.waitForIdle()
        compose.onNode(hasSetTextAction()).performTextInput("Same here")
        compose.waitForIdle()
    })

    @Test
    fun chat_count() = shoot("chat-count", listOf(allowed(max = 40u), talk), after = {
        compose.onNodeWithContentDescription("Chat").performClick()
        compose.waitForIdle()
        compose.onNode(hasSetTextAction()).performTextInput("This is a long line about the lantern")
        compose.waitForIdle()
    })

    @Test
    fun chat_wait() = shoot("chat-wait", listOf(allowed(), talk), after = {
        compose.onNodeWithContentDescription("Chat").performClick()
        compose.waitForIdle()
        store.apply(Event.PartyChatRefused(PartyChatRefusal.Wait(4u)))
        compose.waitForIdle()
    })

    @Test
    fun chat_failed() = shoot("chat-failed", listOf(allowed(), lines(said(1u, "Ren", "That lantern shot again"), failed = "Same here"))) { model, _ ->
        time = 1_000
        model.hideControls()
    }

    @Test
    fun chat_echo_yours() = shoot("chat-echo", listOf(allowed(), lines(said(1u, "Ren", "That lantern shot again"), said(2u, "You", "Same here", you = true)))) { model, _ ->
        time = 1_000
        model.hideControls()
    }

    @Test
    fun chat_muted() = shoot(
        "chat-muted",
        listOf(allowed(muted = listOf("Mio")), lines(said(1u, "Ren", "That lantern shot again"), said(2u, "Mio", "hidden words", muted = true))),
        after = {
            compose.onNodeWithContentDescription("Watch together").performClick()
            compose.waitForIdle()
        },
    )

    @Test
    fun chat_people_sheet() = shoot("chat-people", listOf(allowed(), talk), after = {
        compose.onNodeWithContentDescription("Watch together").performClick()
        compose.waitForIdle()
    })

    @Test
    fun chat_off() = shoot("chat-off", listOf(Event.PartyChatState(PartyChatState(allowed = false, maxLength = 150u))), after = {
        compose.onNodeWithContentDescription("Watch together").performClick()
        compose.waitForIdle()
    })

    @Test
    fun chat_hidden() = shoot("chat-hidden", listOf(allowed(), talk), hide = true)

    private fun shoot(
        name: String,
        events: List<Event>,
        hide: Boolean = false,
        after: () -> Unit = {},
        at: (PlayerModel, ScriptedMpv) -> Unit = { _, _ -> },
    ) {
        store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        var mpv: ScriptedMpv? = null
        model = PlayerModel(send = {}, backend = { ScriptedMpv().also { mpv = it } }, scope = scope, now = { time }, hwdec = "no")
        compose.runOnUiThread {
            model.apply(Event.PlaybackStaged(900_001u, PlayerShots.TITLE, "7", PlayerShots.EPISODE, 1.0))
            model.apply(Event.PlayerOps(1u, listOf(PlayerOp.Open(true, null, null), PlayerOp.Load("/media/episode.mkv", emptyList()))))
            model.apply(Event.PlayerUi(1u, PlayerUi.Episode("7", "Episode 7", PlayerShots.EPISODE, PlayerShots.TITLE, Translation.SUB)))
            room().forEach { store.apply(it) }
            if (hide) store.party.setHideChat(compose.activity, true)
            events.forEach { store.apply(it) }
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
            at(model, m)
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        awaitArt()
        after()
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/$name.png")
    }

    private fun awaitArt() {
        val deadline = System.currentTimeMillis() + 10_000
        while (store.artwork.hasPending && System.currentTimeMillis() < deadline) {
            compose.waitForIdle()
            Thread.sleep(20)
        }
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
    }
}
