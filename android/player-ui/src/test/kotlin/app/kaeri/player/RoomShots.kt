package app.kaeri.player

import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import app.kaeri.design.KaeriTheme
import app.kaeri.design.LocalRoomClock
import app.kaeri.design.RoomClock
import app.kaeri.engine.model.Event
import app.kaeri.player.ui.room.BeforePlayPanel
import app.kaeri.player.ui.room.ChoosingPanel
import app.kaeri.player.ui.room.InvitedPanel
import app.kaeri.player.ui.room.LoadingPanel
import app.kaeri.player.ui.room.NoSourcePanel
import app.kaeri.player.ui.room.RoomBar
import app.kaeri.player.ui.room.RoomOpen
import app.kaeri.player.ui.room.RoomPhone
import app.kaeri.player.ui.room.RoomSourceChoice
import app.kaeri.store.AppStore
import app.kaeri.engine.model.MemberState
import app.kaeri.engine.model.MoveCandidate
import app.kaeri.engine.model.PausedFor
import app.kaeri.engine.model.RelayHealth
import app.kaeri.engine.model.RoomChatBody
import app.kaeri.engine.model.RoomChatLine
import app.kaeri.engine.model.RoomChatState
import app.kaeri.engine.model.RoomConnection
import app.kaeri.engine.model.RoomConversation
import app.kaeri.engine.model.RoomEpisode
import app.kaeri.engine.model.RoomEpisodeInfo
import app.kaeri.engine.model.RoomMember
import app.kaeri.engine.model.RoomMoveOffer
import app.kaeri.engine.model.RoomMoved
import app.kaeri.engine.model.RoomView
import app.kaeri.engine.model.WaitReason
import app.kaeri.store.room.RoomInvited
import app.kaeri.testkit.FixtureArt
import app.kaeri.testkit.captureRoboImage
import app.kaeri.ui.LocalAppStore
import com.github.takahirom.roborazzi.captureScreenRoboImage
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

/**
 * The room board's Android phone frames, drawn on the JVM against the stand-in room: upright at
 * 412 × 915 dp, sideways at 915 × 412, every state the board lists and the sheets behind ⋮.
 * Made-up names and the board's made-up show; the clock is held still.
 *
 * `./gradlew :player-ui:testFixturesDebugUnitTest --tests '*RoomShots*' -Proborazzi.test.record=true`
 */
@OptIn(com.github.takahirom.roborazzi.ExperimentalRoborazziApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = PlayerShots.PHONE_PORTRAIT)
class RoomShots {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    @After
    fun tearDown() = scope.cancel()

    private val now = 1_800_000_000_000L
    private val pos = 724.0
    private fun ago(s: Int) = now - s * 1000L

    private val relays = listOf(
        RelayHealth("wss://relay-one.example", true, 40u),
        RelayHealth("wss://relay-two.example", true, 40u),
        RelayHealth("wss://relay-three.example", true, 40u),
    )

    private fun view(
        waiting: List<PausedFor> = emptyList(),
        canCarryOn: Boolean = false,
        beforePlay: Boolean = false,
        playing: Boolean = true,
        connection: RoomConnection = RoomConnection.Connected,
        relays: List<RelayHealth> = this.relays,
    ) = RoomView(
        roomId = "room-1",
        name = "Mika's room",
        episode = RoomEpisode(900001u, "9"),
        roomEpisode = RoomEpisodeInfo(900001u, "9", "Natsukaze Relay", "The Long Leg", null, null),
        position = pos,
        positionAtMs = now,
        connection = connection,
        link = "https://kaeri.app/r#v1.Q2m4xR9dK7tWbH3nPzL8sYe",
        beforePlay = beforePlay,
        playing = playing,
        waiting = waiting,
        canCarryOn = canCarryOn,
        relays = relays,
        moveCandidates = listOf(
            MoveCandidate("mika", "Mika", muted = false, ticked = true),
            MoveCandidate("ren", "Ren", muted = false, ticked = true),
            MoveCandidate("taro", "Taro", muted = true, ticked = false),
        ),
    )

    private fun ada(state: MemberState = MemberState.WATCHING) = RoomMember("ada", "Ada", you = true, state = state)
    private fun mika(state: MemberState = MemberState.WATCHING) = RoomMember("mika", "Mika", you = false, state = state)
    private fun ren(state: MemberState = MemberState.WATCHING, secs: Int = 0, behind: Float? = null) =
        RoomMember("ren", "Ren", you = false, state = state, sinceMs = ago(secs), behindS = behind)

    private val three = listOf(ada(), mika(), ren())

    private fun waitRen(secs: Int, reason: WaitReason = WaitReason.LOADING) = listOf(PausedFor(id = "ren", name = "Ren", you = false, reason = reason, sinceMs = ago(secs)))

    private fun said(id: ULong, from: String, text: String, you: Boolean = false) = RoomChatLine(id, from, you, RoomChatBody.Said(text), false)

    private val talk = listOf(
        said(1u, "Mika", "everyone here?"),
        said(2u, "Ren", "yes, go"),
        said(3u, "Ren", "back in two, kettle"),
        said(4u, "Ada", "the relay handoff at the bridge", you = true),
        said(5u, "Mika", "every week this song"),
    )

    private fun shoot(
        name: String,
        view: RoomView,
        members: List<RoomMember>,
        full: Boolean = false,
        controls: Boolean = true,
        opened: RoomOpen = RoomOpen.None,
        invited: RoomInvited? = null,
        offer: RoomMoveOffer? = null,
        chat: List<RoomChatLine> = talk,
        screen: Boolean = false,
        panel: (@androidx.compose.runtime.Composable (Modifier) -> Unit)? = null,
    ) {
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        compose.runOnUiThread {
            store.apply(Event.RoomChatState(RoomChatState(maxLength = 150u)))
            store.room.show(view, members, invited = invited, offer = offer, conversation = RoomConversation(lines = chat, marks = emptyList(), failed = null))
        }
        val art = FixtureArt.bytes("${FixtureArt.HOST}banner/900001.jpg")!!
        val still = android.graphics.BitmapFactory.decodeByteArray(art, 0, art.size).asImageBitmap()
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store, LocalRoomClock provides RoomClock({ now }, ticks = false)) {
                    RoomPhone(
                        store.room,
                        full = full,
                        positionS = pos,
                        controls = controls,
                        video = { m -> Image(still, null, m, contentScale = ContentScale.Crop) },
                        panel = panel,
                        opened = opened,
                    )
                }
            }
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        val path = "${roborazziSystemPropertyOutputDirectory()}/room-$name.png"
        if (screen) captureScreenRoboImage(path) else compose.onRoot().captureRoboImage(path)
    }

    // The board's eight Android frames.

    @Test
    fun before_play() = shoot(
        "before-play",
        view(beforePlay = true),
        listOf(ada(), mika(), ren(MemberState.LOADING, 4)),
        chat = listOf(said(1u, "Mika", "everyone here?"), said(2u, "Ren", "two minutes, making tea"), said(3u, "Ada", "ready when you are", you = true)),
        panel = { m -> BeforePlayPanel(started = false, modifier = m) {} },
    )

    @Test
    fun watching() = shoot("watching", view(), three)

    @Test
    fun paused_for_ren() = shoot("paused-for-ren", view(waitRen(6), playing = false), listOf(ada(), mika(), ren(MemberState.LOADING, 6)))

    @Test
    fun twenty_seconds() = shoot("twenty-seconds", view(waitRen(20), canCarryOn = true, playing = false), listOf(ada(), mika(), ren(MemberState.LOADING, 20)))

    @Test
    fun more_menu() = shoot("more-menu", view(), three, opened = RoomOpen.Menu, screen = true)

    @Test
    fun leave_asks() = shoot("leave-asks", view(), three, opened = RoomOpen.Leave, screen = true)

    @Test
    @Config(qualifiers = PlayerShots.PHONE_LANDSCAPE)
    fun sideways_buffering() = shoot("sideways-buffering", view(waitRen(3, WaitReason.BUFFERING), playing = false), listOf(ada(), mika(), ren(MemberState.BUFFERING, 3)), full = true, controls = false)

    @Test
    @Config(qualifiers = PlayerShots.PHONE_LANDSCAPE)
    fun sideways_twenty_seconds() = shoot("sideways-twenty-seconds", view(waitRen(20), canCarryOn = true, playing = false), listOf(ada(), mika(), ren(MemberState.LOADING, 20)), full = true)

    // Every other state the board names for every platform.

    @Test
    fun invited() = shoot(
        "invited",
        view(),
        listOf(mika(), ren()),
        invited = RoomInvited("https://kaeri.app/r#v1.Q2m4xR9dK7tWbH3nPzL8sYe", "Mika", "Natsukaze Relay", "9", listOf("Mika", "Ren")),
        panel = { m -> InvitedPanel("Natsukaze Relay", "Episode 9 · The Long Leg", m) },
    )

    @Test
    fun invited_over_cap() = shoot(
        "invited-over-cap",
        view(),
        listOf("Mika", "Ren", "Sora", "Kenji", "Noa", "Yui", "Taro", "Hana", "Kai", "Rio").map { RoomMember(it.lowercase(), it, false, MemberState.WATCHING) },
        invited = RoomInvited("https://kaeri.app/r#v1.Q2m4xR9dK7tWbH3nPzL8sYe", "Mika", "Natsukaze Relay", "9", listOf("Mika", "Ren", "Sora", "Kenji", "Noa", "Yui", "Taro", "Hana", "Kai", "Rio")),
        panel = { m -> InvitedPanel("Natsukaze Relay", "Episode 9 · The Long Leg", m) },
    )

    @Test
    fun six_people() = shoot(
        "six-people",
        view(),
        three + listOf(
            RoomMember("sora", "Sora", false, MemberState.WATCHING),
            RoomMember("kenji", "Kenji", false, MemberState.PLAYER_CLOSED),
            RoomMember("noa", "Noa", false, MemberState.RECONNECTING),
        ),
    )

    @Test
    fun everyone() = shoot(
        "everyone",
        view(),
        three + listOf(
            RoomMember("sora", "Sora", false, MemberState.WATCHING),
            RoomMember("kenji", "Kenji", false, MemberState.PLAYER_CLOSED),
            RoomMember("noa", "Noa", false, MemberState.RECONNECTING),
        ),
        opened = RoomOpen.Everyone,
        screen = true,
    )

    @Test
    fun paused_by_mika() = shoot(
        "paused-by-mika",
        view(playing = false).copy(pausedBy = "Mika"),
        listOf(ada(MemberState.PAUSED), mika(MemberState.PAUSED).copy(pausedRoom = true), ren(MemberState.PAUSED)),
    )

    @Test
    fun ren_behind() = shoot("ren-behind", view(), listOf(ada(), mika(), ren(MemberState.BEHIND, behind = 4f)))

    @Test
    fun you_loading() = shoot(
        "you-loading",
        view(listOf(PausedFor(id = "ada", name = "Ada", you = true, reason = WaitReason.LOADING, sinceMs = ago(5))), playing = false).copy(canDontWait = true),
        listOf(ada(MemberState.LOADING).copy(sinceMs = ago(5)), mika(), ren()),
        panel = { m -> LoadingPanel(5, canDontWait = true, modifier = m) },
    )

    @Test
    fun no_source() = shoot(
        "no-source",
        view(),
        listOf(ada(MemberState.NO_SOURCE), mika(), ren()),
        panel = { m -> NoSourcePanel("9", m) },
    )

    @Test
    fun choosing() = shoot(
        "choosing",
        view(),
        listOf(ada(MemberState.CHOOSING), mika(), ren()),
        panel = { m -> ChoosingPanel("9", listOf(RoomSourceChoice("Local files", "1080p · 1.2 GB"), RoomSourceChoice("night-index", "720p · 640 MB")), 0, m) },
    )

    @Test
    fun ren_other_episode() = shoot("ren-other-episode", view(), listOf(ada(), mika(), ren(MemberState.OTHER_EPISODE).copy(episode = RoomEpisode(900001u, "8"))))

    @Test
    fun reconnecting() = shoot("reconnecting", view(connection = RoomConnection.Reconnecting(0u)), three)

    @Test
    fun relay_down() = shoot(
        "relay-down",
        view(relays = listOf(relays[0], relays[1].copy(healthy = false, rttMs = null), relays[2])),
        three,
    )

    @Test
    fun go_along() = shoot(
        "go-along",
        view(),
        listOf(ada(), ren(), RoomMember("taro", "Taro", false, MemberState.WATCHING), mika(MemberState.LEFT).copy(sinceMs = ago(1))),
        offer = RoomMoveOffer("offer-1", "Mika", listOf("Ren"), "Natsukaze Relay", "9", now + 60_000),
    )

    @Test
    fun after_move() = shoot(
        "after-move",
        view().copy(moved = RoomMoved("Mika", listOf("Ada", "Ren"))),
        listOf(RoomMember("taro", "Taro", true, MemberState.WATCHING), ren(MemberState.LEFT, 1), RoomMember("ada", "Ada", false, MemberState.LEFT, sinceMs = ago(1))),
    )

    @Test
    fun invite_sheet() = shoot("invite", view(), three, opened = RoomOpen.Invite, screen = true)

    @Test
    fun details_sheet() = shoot(
        "details",
        view(relays = listOf(relays[0], relays[1].copy(healthy = false, rttMs = null), relays[2])),
        three,
        opened = RoomOpen.Details,
        screen = true,
    )

    @Test
    fun edit_relays_sheet() = shoot("edit-relays", view(), three, opened = RoomOpen.EditRelays, screen = true)

    @Test
    fun move_sheet() = shoot("move", view(), three, opened = RoomOpen.Move, screen = true)

    @Test
    fun room_bar() {
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        compose.runOnUiThread { store.room.show(view(waitRen(6), playing = false), listOf(ada(), mika(), ren(MemberState.LOADING, 6))) }
        val art = FixtureArt.bytes("${FixtureArt.HOST}banner/900001.jpg")!!
        val still = android.graphics.BitmapFactory.decodeByteArray(art, 0, art.size).asImageBitmap()
        compose.setContent {
            KaeriTheme {
                CompositionLocalProvider(LocalAppStore provides store, LocalRoomClock provides RoomClock({ now }, ticks = false)) {
                    androidx.compose.foundation.layout.Box(
                        Modifier.fillMaxSize().background(app.kaeri.design.ObiSilk.Ground).padding(start = 12.dp, end = 12.dp, bottom = 96.dp),
                        contentAlignment = androidx.compose.ui.Alignment.BottomCenter,
                    ) {
                        RoomBar(store.room, pos, still = { m -> Image(still, null, m, contentScale = ContentScale.Crop) })
                    }
                }
            }
        }
        compose.mainClock.advanceTimeBy(1_000)
        compose.waitForIdle()
        compose.onRoot().captureRoboImage("${roborazziSystemPropertyOutputDirectory()}/room-bar.png")
    }
}
