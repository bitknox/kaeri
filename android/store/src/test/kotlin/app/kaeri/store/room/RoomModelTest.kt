// The room model between rooms: nothing of a room you left or moved from carries into the next,
// and a failure after an invite is an alert, not the Watch together line an earlier paste set up.
// A look shows only for the look waited on, and never leaves the room this device is in.

package app.kaeri.store.room

import app.kaeri.engine.model.Event
import app.kaeri.engine.model.MemberState
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.RoomChatBody
import app.kaeri.engine.model.RoomChatLine
import app.kaeri.engine.model.RoomChatState
import app.kaeri.engine.model.RoomCommand
import app.kaeri.engine.model.RoomConnection
import app.kaeri.engine.model.RoomConversation
import app.kaeri.engine.model.RoomEpisode
import app.kaeri.engine.model.RoomGlimpse
import app.kaeri.engine.model.RoomLeftWhy
import app.kaeri.engine.model.RoomMember
import app.kaeri.engine.model.RoomMoveOffer
import app.kaeri.engine.model.RoomRelaySettings
import app.kaeri.engine.model.RoomTarget
import app.kaeri.engine.model.RoomView
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RoomModelTest {
    private fun view(id: String) = RoomView(
        roomId = id,
        name = "Mika's room",
        episode = RoomEpisode(900001u, "9"),
        position = 12.0,
        positionAtMs = 0,
        connection = RoomConnection.Connected,
        link = "https://kaeri.app/r#v1.$id",
    )

    private val mika = RoomMember("mika", "Mika", you = false, state = MemberState.WATCHING)
    private val said = RoomChatLine(1u, "Mika", false, RoomChatBody.Said("everyone here?"), false)

    /** A room with people, a line said, the chat's limits and a move offer, as a room in use has. */
    private fun inUse(room: RoomModel, id: String) {
        room.apply(Event.RoomJoined(view(id)))
        room.apply(Event.RoomMembers(listOf(mika)))
        room.apply(Event.RoomChat(RoomConversation(lines = listOf(said), marks = emptyList(), failed = null)))
        room.apply(Event.RoomChatState(RoomChatState(maxLength = 150u)))
        room.apply(Event.RoomMoveOffer(RoomMoveOffer("offer-1", "Mika", listOf("Ren"), "Natsukaze Relay", "9", 60_000)))
    }

    private fun assertForgotten(room: RoomModel) {
        assertTrue(room.members.value.isEmpty())
        assertNull(room.conversation.value)
        assertNull(room.chat.value)
        assertNull(room.offer.value)
    }

    @Test
    fun anotherRoomStartsWithNothingOfTheLast() {
        val room = RoomModel()
        inUse(room, "room-a")
        room.apply(Event.RoomJoined(view("room-b")))
        assertEquals("room-b", room.view.value?.roomId)
        assertForgotten(room)
    }

    @Test
    fun theSameRoomAgainKeepsItsChat() {
        val room = RoomModel()
        inUse(room, "room-a")
        room.apply(Event.RoomJoined(view("room-a")))
        assertEquals(listOf(mika), room.members.value)
        assertEquals(listOf(said), room.conversation.value?.lines)
    }

    @Test
    fun leavingForgetsTheRoom() {
        val sent = mutableListOf<RoomCommand>()
        val room = RoomModel { sent += it }
        inUse(room, "room-a")
        room.send(RoomCommand.Leave)
        assertNull(room.view.value)
        assertForgotten(room)
        assertEquals(listOf<RoomCommand>(RoomCommand.Leave), sent)
    }

    @Test
    fun aRoomThatEndsForgetsItsChat() {
        val room = RoomModel()
        inUse(room, "room-a")
        room.apply(Event.RoomLeft(RoomLeftWhy.Unreachable))
        assertForgotten(room)
        assertEquals(RoomLeftWhy.Unreachable, room.left.value)
    }

    @Test
    fun anInviteAfterAPastedLinkFailsAsAnAlert() {
        val room = RoomModel()
        room.joinLink("https://kaeri.app/r#v1.pasted")
        room.invitedBy(RoomTarget(link = "https://kaeri.app/r#v1.invite", relays = emptyList(), by = "Mika", title = null, episode = null))
        room.join()
        room.apply(Event.RoomLeft(RoomLeftWhy.Unreachable))
        assertNull(room.joinFailed.value)
        assertEquals(RoomLeftWhy.Unreachable, room.left.value)
    }

    @Test
    fun aJoinFromTheInviteScreensGlimpseIsTheRoomsOwn() {
        // The invite screen draws through a glimpse model that hands its commands to the room's.
        val sent = mutableListOf<RoomCommand>()
        val room = RoomModel { sent += it }
        val glimpse = RoomModel { room.send(it) }
        glimpse.send(RoomCommand.Lobby(inviteA, null, null, false))
        room.apply(Event.RoomLeft(RoomLeftWhy.Full))
        assertEquals(RoomLeftWhy.Full, room.left.value)
        assertEquals(1, sent.size)
    }

    @Test
    fun aStartThatFailsIsAnAlert() {
        // Watch together from the player starts the room through the room's model.
        val room = RoomModel()
        room.send(RoomCommand.Lobby(null, null, RoomEpisode(900001u, "9"), false))
        room.apply(Event.RoomLeft(RoomLeftWhy.Unreachable))
        assertEquals(RoomLeftWhy.Unreachable, room.left.value)
    }

    @Test
    fun aPastedLinkThatFailsStaysOnItsLine() {
        val room = RoomModel()
        room.joinLink("https://kaeri.app/r#v1.pasted")
        room.apply(Event.RoomLeft(RoomLeftWhy.Unreachable))
        assertEquals("https://kaeri.app/r#v1.pasted", room.joinFailed.value)
        assertNull(room.left.value)
    }

    private val inviteA = "https://kaeri.app/r#v1.invite-a"
    private val inviteB = "https://kaeri.app/r#v1.invite-b"

    private fun target(link: String) = RoomTarget(link = link, relays = emptyList(), by = "Mika", title = null, episode = null)

    private fun glimpse(invite: String?, names: List<String>) = RoomGlimpse(
        roomId = null,
        by = "Mika",
        members = names.map { RoomMember(it.lowercase(), it, you = false, state = MemberState.WATCHING) },
        roomEpisode = null,
        playing = true,
        full = false,
        invite = invite,
    )

    private fun lookFailed(roomId: String, invite: String?) =
        Event.RoomLookFailed(roomId = roomId, reason = Msg.RoomLookUnreachable, invite = invite, why = RoomLeftWhy.Unreachable)

    @Test
    fun aSlowGlimpseNeverStandsInForANewerInvite() {
        val room = RoomModel()
        room.invitedBy(target(inviteA))
        room.invitedBy(target(inviteB))
        room.apply(Event.RoomGlimpse(glimpse(inviteA, listOf("Sora"))))
        assertNull(room.invited.value?.glimpse)
        room.apply(Event.RoomGlimpse(glimpse(inviteB, listOf("Ren"))))
        assertEquals(listOf("Ren"), room.invited.value?.names)
    }

    @Test
    fun aRecentRoomsGlimpseIsNoInvites() {
        val room = RoomModel()
        room.invitedBy(target(inviteA))
        room.apply(Event.RoomGlimpse(glimpse(null, listOf("Sora"))))
        assertNull(room.invited.value?.glimpse)
    }

    @Test
    fun anInvitesFailedLookStaysWithTheInviteAndKeepsTheRoom() {
        val room = RoomModel()
        inUse(room, "room-a")
        room.invitedBy(target(inviteB))
        room.apply(lookFailed("room-b", inviteA))
        assertNull(room.invited.value?.failed)
        room.apply(lookFailed("room-b", inviteB))
        assertEquals(RoomLeftWhy.Unreachable, room.invited.value?.failed)
        assertEquals("room-a", room.view.value?.roomId)
        assertEquals(listOf(mika), room.members.value)
        assertNull(room.left.value)
        room.notNow()
        assertNull(room.invited.value)
    }

    @Test
    fun aRoomNoLongerKeptFailsUnderItsRowOnlyWhenAsked() {
        val room = RoomModel()
        room.apply(lookFailed("room-a", null))
        assertNull(room.rejoinFailed.value)
        room.rejoin("room-b")
        room.apply(lookFailed("room-a", null))
        assertNull(room.rejoinFailed.value)
        room.apply(lookFailed("room-b", null))
        assertEquals("room-b", room.rejoinFailed.value)
        assertNull(room.left.value)
    }

    @Test
    fun aLeftWithNoRoomAndNoJoinIsNobodys() {
        val room = RoomModel()
        room.invitedBy(target(inviteA))
        room.apply(Event.RoomLeft(RoomLeftWhy.Unreachable))
        assertNull(room.left.value)
        assertEquals(inviteA, room.invited.value?.invite)
    }

    @Test
    fun relaySettingsComeAndGoBackToTheDefaults() {
        val sent = mutableListOf<RoomCommand>()
        val room = RoomModel { sent += it }
        val defaults = listOf("wss://relay-1.example", "wss://relay-2.example")
        val spares = listOf("wss://relay-3.example")
        room.apply(Event.RoomRelays(RoomRelaySettings(defaults = defaults, spares = emptyList(), defaultSpares = spares, chosen = listOf("ws://box.local"))))
        assertEquals(listOf("ws://box.local"), room.relays.value?.chosen)
        room.setRelays(defaults)
        room.restoreRelays()
        assertEquals(
            listOf<RoomCommand>(RoomCommand.SetRelays(emptyList()), RoomCommand.SetRelays(emptyList()), RoomCommand.SetSpareRelays(spares)),
            sent,
        )
    }
}
