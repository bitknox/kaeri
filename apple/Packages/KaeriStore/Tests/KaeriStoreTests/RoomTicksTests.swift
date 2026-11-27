// The room's reaction ticks on the seek bar, and Watch together's invite card naming its show once.

@testable import KaeriStore
import KaeriKit
import Testing

@MainActor
struct RoomTicksTests {
    private func joined() -> RoomModel {
        let room = RoomModel()
        room.apply(.roomJoined(RoomView(
            roomId: "room-1", name: "Room", connection: .connected, playing: true, beforePlay: false,
            waiting: [], canCarryOn: false, relays: [])))
        return room
    }

    private func reacted(_ room: RoomModel, _ marks: [(Double, String)]) {
        room.apply(.roomChat(RoomConversation(
            lines: [], marks: marks.map { RoomReactionMark(at: $0.0, from: $0.1, reaction: .laugh) }, failed: nil)))
    }

    @Test func reactionsCloseTogetherShareATick() {
        let room = joined()
        reacted(room, [(31, "Mika"), (8, "Mika"), (8.3, "Ren"), (47, "Ren")])
        // 60 s on a 600 pt line: 8 s and 8.3 s sit 3 pt apart.
        let ticks = room.ticks(duration: 60, width: 600)
        #expect(ticks.map(\.at) == [8, 31, 47])
        #expect(ticks[0].names == ["Mika", "Ren"])
        #expect(ticks[0].x == 80)
    }

    @Test func noTicksWithChatHiddenOrOutsideARoom() {
        let room = joined()
        reacted(room, [(8, "Mika")])
        room.chatHidden = true
        #expect(room.ticks(duration: 60, width: 600).isEmpty, "Hide chat takes the ticks off the seek bar")
        room.chatHidden = false
        room.apply(.roomLeft(why: .left))
        #expect(room.marks.isEmpty)
        #expect(room.ticks(duration: 60, width: 600).isEmpty)
    }

    @Test func aMarkPastTheEpisodeHasNoTick() {
        let room = joined()
        reacted(room, [(-1, "Mika"), (70, "Ren")])
        #expect(room.ticks(duration: 60, width: 600).isEmpty)
        #expect(room.ticks(duration: 0, width: 600).isEmpty)
    }

    @Test func theTickLabelNamesWhoReacted() {
        #expect(RoomCopy.reactors(["Ren"]) == "Ren")
        #expect(RoomCopy.reactors(["Ren", "Mika"]).contains("Mika"))
        #expect(RoomCopy.reactors(["Ren", "Mika", "Sora", "Noa"]).contains("2"))
        let tick = ReactionTick(at: 724, x: 0, names: ["Ren", "Mika"])
        #expect(tick.label.hasPrefix("12:04"))
    }

    @Test func anInviteCardNamesItsShowOnce() {
        let room = RoomModel()
        room.open(invite: "kaeri://room#invite.example")
        room.apply(.roomGlimpse(RoomGlimpse(
            by: "Mika", roomEpisode: RoomEpisodeInfo(id: 900_001, episode: "9", title: "Stardust Wayfarer"),
            invite: "kaeri://room#invite.example")))
        let status = room.status(now: 0)
        #expect(status.detail.contains("Stardust Wayfarer"), "the player's panel has no title above it")
        #expect(room.cardDetail(status) == room.episodeName("9"))
        #expect(!room.cardDetail(status).contains("Stardust Wayfarer"))
    }

    // MARK: Settings ▸ Relays

    private func relays(
        chosen: [String] = [], spares: [String] = ["wss://spare-1.example", "wss://spare-2.example"]
    ) -> (RoomModel, () -> [Command]) {
        let room = RoomModel()
        var sent: [Command] = []
        room.send = { sent.append($0) }
        room.apply(.roomRelays(RoomRelaySettings(
            defaults: ["wss://one.example", "wss://two.example"], spares: spares,
            defaultSpares: ["wss://spare-1.example", "wss://spare-2.example"], chosen: chosen)))
        return (room, { sent })
    }

    @Test func newRoomsUseTheDefaultsUntilSettingsHasAList() {
        let (room, _) = relays()
        #expect(room.newRoomRelays == ["wss://one.example", "wss://two.example"])
        #expect(room.relaysAreDefault)
        let (own, _) = relays(chosen: ["wss://home.example"])
        #expect(own.newRoomRelays == ["wss://home.example"])
        #expect(!own.relaysAreDefault)
    }

    @Test func addingARelaySendsTheWholeList() {
        let (room, sent) = relays()
        #expect(room.addRelay("relay.home.example"))
        #expect(sent() == [.room(.setRelays(relays: ["wss://one.example", "wss://two.example", "wss://relay.home.example"]))])
        #expect(!room.addRelay("ws://far.example"), "plain ws:// only on this machine or its network")
        #expect(room.addRelay("wss://one.example"))
        #expect(sent().count == 1, "one already listed is left as it is")
    }

    @Test func theLastRelayStaysAndRestoreSendsNoList() {
        let (room, sent) = relays(chosen: ["wss://home.example", "wss://one.example"])
        room.removeRelay("wss://home.example")
        room.removeRelay("wss://one.example")
        #expect(room.newRoomRelays == ["wss://one.example"])
        room.restoreRelays()
        #expect(sent() == [.room(.setRelays(relays: ["wss://one.example"])), .room(.setRelays(relays: []))])
        #expect(room.relaysAreDefault)
    }

    @Test func listingTheDefaultsAgainIsNoListOfItsOwn() {
        let (room, sent) = relays(chosen: ["wss://one.example"])
        #expect(room.addRelay("wss://two.example"))
        #expect(sent() == [.room(.setRelays(relays: []))])
        #expect(room.relaysAreDefault)
    }

    @Test func aSpareComesOffAndRestorePutsTheBuiltInOnesBack() {
        let (room, sent) = relays()
        room.removeSpare("wss://spare-1.example")
        #expect(room.relaySettings?.spares == ["wss://spare-2.example"])
        #expect(!room.relaysAreDefault)
        room.restoreRelays()
        #expect(sent() == [
            .room(.setSpareRelays(relays: ["wss://spare-2.example"])),
            .room(.setSpareRelays(relays: ["wss://spare-1.example", "wss://spare-2.example"])),
        ], "the list for new rooms was already the defaults")
        #expect(room.relaysAreDefault)
    }

    @Test func restoreSetsBothListsBack() {
        let (room, sent) = relays(chosen: ["wss://home.example"], spares: [])
        room.restoreRelays()
        #expect(sent() == [
            .room(.setRelays(relays: [])),
            .room(.setSpareRelays(relays: ["wss://spare-1.example", "wss://spare-2.example"])),
        ])
    }

    // MARK: Looks

    private let invite = "kaeri://room#invite.example"

    @Test func onlyTheInviteLookedIntoBecomesAnInvitation() {
        let room = RoomModel()
        room.open(invite: invite)
        room.apply(.roomGlimpse(RoomGlimpse(by: "Mika", invite: "kaeri://room#other.example")))
        #expect(room.invitation == nil, "another look's glimpse")
        room.apply(.roomGlimpse(RoomGlimpse(by: "Mika")))
        #expect(room.invitation == nil, "a recent room's glimpse")
        room.apply(.roomGlimpse(RoomGlimpse(by: "Mika", invite: invite)))
        #expect(room.invitation?.by == "Mika")
        #expect(room.invitation?.link == invite)
    }

    @Test func anInviteLookThatFailsSaysWhy() {
        let room = RoomModel()
        room.open(invite: invite)
        room.apply(.roomLookFailed(roomId: "", reason: .roomLookOldInvite, invite: "kaeri://room#other.example", why: .oldInvite))
        #expect(room.joinFailed == nil, "another look's failure")
        room.apply(.roomLookFailed(roomId: "", reason: .roomLookOldInvite, invite: invite, why: .oldInvite))
        #expect(room.joinFailed == invite)
        #expect(room.joinFailure == .oldInvite)
        #expect(room.invitation == nil)
    }

    @Test func aLookFailingNeverLeavesTheRoomYouAreIn() {
        let room = joined()
        room.open(invite: invite)
        room.apply(.roomLookFailed(roomId: "", reason: .roomLookUnreachable, invite: invite, why: .unreachable))
        room.apply(.roomLookFailed(roomId: "r9", reason: .roomLookNotKept, invite: nil, why: nil))
        #expect(room.inRoom)
    }

    @Test func aLeaveWithNoRoomKeepsTheInvitation() {
        let room = RoomModel()
        room.open(invite: invite)
        room.apply(.roomGlimpse(RoomGlimpse(by: "Mika", invite: invite)))
        room.apply(.roomLeft(why: .unreachable))
        #expect(room.invitation != nil, "nothing was joined, so nothing is left")
    }

    // MARK: Neon's second pass

    @Test func noRelayIsAddedBeforeTheEngineHasSaidTheList() {
        let room = RoomModel()
        var sent: [Command] = []
        room.send = { sent.append($0) }
        #expect(!room.addRelay("wss://home.example"), "the defaults aren't known yet")
        #expect(sent.isEmpty)
    }

    @Test func relayAddressesAreKeptInLowerCase() {
        #expect(RoomRelays.address("WSS://Relay.Home.Example") == "wss://relay.home.example")
        #expect(RoomRelays.address("Relay.Home.Example/Path") == "wss://relay.home.example/Path")
        #expect(RoomRelays.shown(RoomRelays.address("WSS://one.example")!) == "one.example")
        let (room, sent) = relays()
        #expect(room.addRelay("wss://ONE.example"))
        #expect(sent().isEmpty, "the same relay in other letters is already listed")
    }

    @Test func questionsOverThePlayerHoldTheKeys() {
        let room = joined()
        #expect(!room.holdsKeys)
        room.moving = true
        #expect(room.holdsKeys)
        room.moving = false
        room.popover = .editRelays
        #expect(room.holdsKeys)
        #expect(!room.popoverClosesOutside, "Edit relays keeps its edits")
        room.popover = .details
        #expect(room.popoverClosesOutside)
    }
}
