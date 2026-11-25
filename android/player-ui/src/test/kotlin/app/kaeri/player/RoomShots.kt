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
import app.kaeri.engine.model.PartyChatBody
import app.kaeri.engine.model.PartyChatLine
import app.kaeri.engine.model.PartyChatState
import app.kaeri.engine.model.PartyConversation
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
import app.kaeri.store.room.MemberState
import app.kaeri.store.room.MoveCandidate
import app.kaeri.store.room.PausedFor
import app.kaeri.store.room.RelayHealth
import app.kaeri.store.room.RoomConnection
import app.kaeri.store.room.RoomEpisode
import app.kaeri.store.room.RoomInvited
import app.kaeri.store.room.RoomMember
import app.kaeri.store.room.RoomMoveOffer
import app.kaeri.store.room.RoomMoved
import app.kaeri.store.room.RoomView
import app.kaeri.store.room.WaitReason
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
        RelayHealth("wss://relay-one.example", true, 40),
        RelayHealth("wss://relay-two.example", true, 40),
        RelayHealth("wss://relay-three.example", true, 40),
    )

    private fun view(
        waiting: List<PausedFor> = emptyList(),
        canCarryOn: Boolean = false,
        beforePlay: Boolean = false,
        playing: Boolean = true,
        connection: RoomConnection = RoomConnection.Connected,
        relays: List<RelayHealth> = this.relays,
    ) = RoomView(
        title = "Natsukaze Relay",
        episode = RoomEpisode(9, "9"),
        name = "Mika's room",
        connection = connection,
        link = "https://kaeri.app/r#v1.Q2m4xR9dK7tWbH3nPzL8sYe",
        beforePlay = beforePlay,
        playing = playing,
        waiting = waiting,
        canCarryOn = canCarryOn,
        relays = relays,
        moveCandidates = listOf(
            MoveCandidate("ada", "Ada", muted = false, ticked = true),
            MoveCandidate("ren", "Ren", muted = false, ticked = true),
            MoveCandidate("taro", "Taro", muted = true, ticked = false),
        ),
    )

    private fun ada(state: MemberState = MemberState.Watching) = RoomMember("ada", "Ada", you = true, state = state)
    private fun mika(state: MemberState = MemberState.Watching) = RoomMember("mika", "Mika", you = false, state = state)
    private fun ren(state: MemberState = MemberState.Watching, secs: Int = 0, behind: Float? = null) =
        RoomMember("ren", "Ren", you = false, state = state, sinceMs = ago(secs), behindS = behind)

    private val three = listOf(ada(), mika(), ren())

    private fun waitRen(secs: Int, reason: WaitReason = WaitReason.Loading) = listOf(PausedFor("Ren", reason, ago(secs)))

    private fun said(id: ULong, from: String, text: String, you: Boolean = false) = PartyChatLine(id, from, you, PartyChatBody.Said(text), false)

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
        moved: RoomMoved? = null,
        chat: List<PartyChatLine> = talk,
        screen: Boolean = false,
        panel: (@androidx.compose.runtime.Composable (Modifier) -> Unit)? = null,
    ) {
        val store = AppStore(scope, clock = { 0L }, artworkWithoutEngine = FixtureArt::bytes)
        compose.runOnUiThread {
            store.apply(Event.PartyChatState(PartyChatState(allowed = true, maxLength = 150u)))
            store.apply(Event.PartyChat(PartyConversation(lines = chat, marks = emptyList())))
            store.room.show(view, members, invited = invited, offer = offer, moved = moved)
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
        listOf(ada(), mika(), ren(MemberState.Loading, 4)),
        chat = listOf(said(1u, "Mika", "everyone here?"), said(2u, "Ren", "two minutes, making tea"), said(3u, "Ada", "ready when you are", you = true)),
        panel = { m -> BeforePlayPanel(started = false, modifier = m) {} },
    )

    @Test
    fun watching() = shoot("watching", view(), three)

    @Test
    fun paused_for_ren() = shoot("paused-for-ren", view(waitRen(6), playing = false), listOf(ada(), mika(), ren(MemberState.Loading, 6)))

    @Test
    fun twenty_seconds() = shoot("twenty-seconds", view(waitRen(20), canCarryOn = true, playing = false), listOf(ada(), mika(), ren(MemberState.Loading, 20)))

    @Test
    fun more_menu() = shoot("more-menu", view(), three, opened = RoomOpen.Menu, screen = true)

    @Test
    fun leave_asks() = shoot("leave-asks", view(), three, opened = RoomOpen.Leave, screen = true)

    @Test
    @Config(qualifiers = PlayerShots.PHONE_LANDSCAPE)
    fun sideways_buffering() = shoot("sideways-buffering", view(waitRen(3, WaitReason.Buffering), playing = false), listOf(ada(), mika(), ren(MemberState.Buffering, 3)), full = true, controls = false)

    @Test
    @Config(qualifiers = PlayerShots.PHONE_LANDSCAPE)
    fun sideways_twenty_seconds() = shoot("sideways-twenty-seconds", view(waitRen(20), canCarryOn = true, playing = false), listOf(ada(), mika(), ren(MemberState.Loading, 20)), full = true)

    // Every other state the board names for every platform.

    @Test
    fun invited() = shoot(
        "invited",
        view(),
        listOf(mika(), ren()),
        invited = RoomInvited("Mika", "Natsukaze Relay", "9", listOf("Mika", "Ren")),
        panel = { m -> InvitedPanel("Natsukaze Relay", "Episode 9 · The Long Leg", m) },
    )

    @Test
    fun invited_over_cap() = shoot(
        "invited-over-cap",
        view(),
        listOf("Mika", "Ren", "Sora", "Kenji", "Noa", "Yui", "Taro", "Hana", "Kai", "Rio").map { RoomMember(it.lowercase(), it, false, MemberState.Watching) },
        invited = RoomInvited("Mika", "Natsukaze Relay", "9", listOf("Mika", "Ren", "Sora", "Kenji", "Noa", "Yui", "Taro", "Hana", "Kai", "Rio")),
        panel = { m -> InvitedPanel("Natsukaze Relay", "Episode 9 · The Long Leg", m) },
    )

    @Test
    fun six_people() = shoot(
        "six-people",
        view(),
        three + listOf(
            RoomMember("sora", "Sora", false, MemberState.Watching),
            RoomMember("kenji", "Kenji", false, MemberState.PlayerClosed),
            RoomMember("noa", "Noa", false, MemberState.Reconnecting),
        ),
    )

    @Test
    fun everyone() = shoot(
        "everyone",
        view(),
        three + listOf(
            RoomMember("sora", "Sora", false, MemberState.Watching),
            RoomMember("kenji", "Kenji", false, MemberState.PlayerClosed),
            RoomMember("noa", "Noa", false, MemberState.Reconnecting),
        ),
        opened = RoomOpen.Everyone,
        screen = true,
    )

    @Test
    fun paused_by_mika() = shoot(
        "paused-by-mika",
        view(playing = false).copy(pausedBy = "Mika"),
        listOf(ada(MemberState.Paused), mika(MemberState.Paused).copy(pausedRoom = true), ren(MemberState.Paused)),
    )

    @Test
    fun ren_behind() = shoot("ren-behind", view(), listOf(ada(), mika(), ren(MemberState.Behind, behind = 4f)))

    @Test
    fun you_loading() = shoot(
        "you-loading",
        view(listOf(PausedFor("Ada", WaitReason.Loading, ago(5))), playing = false).copy(canDontWait = true),
        listOf(ada(MemberState.Loading).copy(sinceMs = ago(5)), mika(), ren()),
        panel = { m -> LoadingPanel(5, canDontWait = true, modifier = m) },
    )

    @Test
    fun no_source() = shoot(
        "no-source",
        view(),
        listOf(ada(MemberState.NoSource), mika(), ren()),
        panel = { m -> NoSourcePanel("9", m) },
    )

    @Test
    fun choosing() = shoot(
        "choosing",
        view(),
        listOf(ada(MemberState.Choosing), mika(), ren()),
        panel = { m -> ChoosingPanel("9", listOf(RoomSourceChoice("Local files", "1080p · 1.2 GB"), RoomSourceChoice("night-index", "720p · 640 MB")), 0, m) },
    )

    @Test
    fun ren_other_episode() = shoot("ren-other-episode", view(), listOf(ada(), mika(), ren(MemberState.OtherEpisode).copy(episode = RoomEpisode(8, "8"))))

    @Test
    fun reconnecting() = shoot("reconnecting", view(connection = RoomConnection.Reconnecting(0)), three)

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
        listOf(ada(), ren(), RoomMember("taro", "Taro", false, MemberState.Watching), mika(MemberState.Left).copy(sinceMs = ago(1))),
        offer = RoomMoveOffer("offer-1", "Mika", listOf("Ren"), "Natsukaze Relay", "9", now + 60_000),
    )

    @Test
    fun after_move() = shoot(
        "after-move",
        view(),
        listOf(RoomMember("taro", "Taro", true, MemberState.Watching), ren(MemberState.Left, 1), RoomMember("ada", "Ada", false, MemberState.Left, sinceMs = ago(1))),
        moved = RoomMoved("Mika", listOf("Ada", "Ren")),
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
        compose.runOnUiThread { store.room.show(view(waitRen(6), playing = false), listOf(ada(), mika(), ren(MemberState.Loading, 6))) }
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
