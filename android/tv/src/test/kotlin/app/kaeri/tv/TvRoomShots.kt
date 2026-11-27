package app.kaeri.tv

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import app.kaeri.design.LocalRoomClock
import app.kaeri.design.RoomClock
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.MemberState
import app.kaeri.engine.model.PausedFor
import app.kaeri.engine.model.RoomChatBody
import app.kaeri.engine.model.RoomChatLine
import app.kaeri.engine.model.RoomChatState
import app.kaeri.engine.model.RoomConnection
import app.kaeri.engine.model.RoomConversation
import app.kaeri.engine.model.RoomEpisode
import app.kaeri.engine.model.RoomEpisodeInfo
import app.kaeri.engine.model.RoomMember
import app.kaeri.engine.model.RoomMoveOffer
import app.kaeri.engine.model.RoomView
import app.kaeri.engine.model.WaitReason
import app.kaeri.store.AppStore
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.captureRoboImage
import app.kaeri.tv.player.room.TvRoom
import app.kaeri.tv.player.room.TvRoomMode
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

/**
 * The room board's Android TV frames at 960 × 540 dp against the stand-in room: the full picture
 * with its strip, Up on Carry on, the rail, before play, and Back's question. The clock is held.
 *
 * `./gradlew :tv:testFixturesDebugUnitTest --tests '*TvRoomShots*' -Proborazzi.test.record=true`
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = TvShots.TV)
class TvRoomShots {
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

    @After
    fun tearDown() = scope.cancel()

    private val now = 1_800_000_000_000L
    private fun ago(s: Int) = now - s * 1000L

    private fun view(waiting: List<PausedFor> = emptyList(), canCarryOn: Boolean = false, beforePlay: Boolean = false, playing: Boolean = true) = RoomView(
        roomId = "room-1",
        name = "Mika's room",
        connection = RoomConnection.Connected,
        episode = RoomEpisode(900001u, "9"),
        roomEpisode = RoomEpisodeInfo(900001u, "9", "Natsukaze Relay", null, null, null),
        beforePlay = beforePlay,
        playing = playing,
        waiting = waiting,
        canCarryOn = canCarryOn,
    )

    private fun members(ren: MemberState = MemberState.WATCHING, secs: Int = 0, behind: Float? = null) = listOf(
        RoomMember("ada", "Ada", false, MemberState.WATCHING),
        RoomMember("mika", "Mika", false, MemberState.WATCHING),
        RoomMember("ren", "Ren", false, ren, sinceMs = ago(secs), behindS = behind),
        RoomMember("tv", "Living room", true, MemberState.WATCHING),
    )

    private fun waitRen(s: Int) = listOf(PausedFor(id = "ren", name = "Ren", you = false, reason = WaitReason.LOADING, sinceMs = ago(s)))

    private fun said(id: ULong, from: String, text: String) = RoomChatLine(id, from, false, RoomChatBody.Said(text), false)

    private fun shoot(name: String, view: RoomView, members: List<RoomMember>, start: TvRoomMode = TvRoomMode.Full, focusCarry: Boolean = false, strip: Boolean = true, offer: RoomMoveOffer? = null, hideChat: Boolean = false, left: app.kaeri.engine.model.RoomLeftWhy? = null) {
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        compose.runOnUiThread {
            store.room.setHideChat(compose.activity, hideChat)
            store.apply(Event.RoomChatState(RoomChatState(maxLength = 150u)))
            val talk = listOf(said(1u, "Ren", "yes, go"), said(2u, "Ada", "the relay handoff at the bridge"), said(3u, "Mika", "every week this song"))
            store.room.show(view, members, offer = offer, conversation = RoomConversation(lines = talk, marks = emptyList(), failed = null))
        }
        val art = FixtureArt.bytes("${FixtureArt.HOST}banner/900001.jpg")!!
        val still = android.graphics.BitmapFactory.decodeByteArray(art, 0, art.size).asImageBitmap()
        compose.setContent {
            TvTheme {
                CompositionLocalProvider(LocalAppStore provides store, LocalRoomClock provides RoomClock({ now }, ticks = false)) {
                    TvRoom(
                        store.room,
                        positionS = 724.0,
                        episodeLine = "Episode 9",
                        durationS = 1420.0,
                        video = { m -> Image(still, null, m, contentScale = ContentScale.Crop) },
                        start = start,
                        focusCarry = focusCarry,
                        forceStrip = strip,
                    )
                    left?.let { app.kaeri.tv.player.room.TvRoomLeftAlert(it) {} }
                }
            }
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/tv-room-$name.png")
    }

    @Test
    fun before_play() = shoot("before-play", view(beforePlay = true), members(MemberState.LOADING, 4), start = TvRoomMode.Rail)

    /** Before play on the full picture, where the TV opens: Play for everyone, focused. */
    @Test
    fun before_play_full() = shoot("before-play-full", view(beforePlay = true), members(MemberState.LOADING, 4))

    /** Hide chat in rooms: the rail keeps the room's own lines; what people said and Write from your phone go. */
    @Test
    fun rail_hide_chat() = shoot("rail-hide-chat", view(), members(), start = TvRoomMode.Rail, hideChat = true)

    /** A room a phone handed the TV that could not be reached: said over the screen, with Close. */
    @Test
    fun room_left_alert() = shoot("room-left-alert", view(), members(), strip = false, left = app.kaeri.engine.model.RoomLeftWhy.Unreachable)

    @Test
    fun at_rest() = shoot("at-rest", view(), members(), strip = false)

    @Test
    fun paused_for_ren() = shoot("paused-for-ren", view(waitRen(6), playing = false), members(MemberState.LOADING, 6))

    @Test
    fun twenty_seconds() = shoot("twenty-seconds", view(waitRen(20), canCarryOn = true, playing = false), members(MemberState.LOADING, 20))

    @Test
    fun up_on_carry() = shoot("up-on-carry", view(waitRen(20), canCarryOn = true, playing = false), members(MemberState.LOADING, 20), focusCarry = true)

    @Test
    fun rail() = shoot("rail", view(), members(MemberState.BEHIND, behind = 4f), start = TvRoomMode.Rail)

    @Test
    fun rail_twenty_seconds() = shoot("rail-twenty-seconds", view(waitRen(20), canCarryOn = true, playing = false), members(MemberState.LOADING, 20), start = TvRoomMode.Rail)

    @Test
    fun leave_asks() = shoot("leave-asks", view(), members(), start = TvRoomMode.Leave, strip = false)

    @Test
    fun go_along() = shoot("go-along", view(), members(), offer = RoomMoveOffer("offer-1", "Mika", listOf("Ren"), "Natsukaze Relay", "9", now + 60_000))
}
