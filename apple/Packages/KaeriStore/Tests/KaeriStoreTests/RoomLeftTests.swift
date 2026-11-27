// What a room leaves behind: nothing of one room carries into the next, a relay down never hides
// what the room waits on, and a relay is checked as the room protocol takes it.

@testable import KaeriStore
import KaeriKit
import Testing

@MainActor
struct RoomLeftTests {
    private func view(relays: [RelayHealth] = [], waiting: [PausedFor] = [], canCarryOn: Bool = false) -> RoomView {
        RoomView(
            roomId: "room-1", name: "Room", connection: .connected, playing: true, beforePlay: false,
            waiting: waiting, canCarryOn: canCarryOn, relays: relays)
    }

    @Test func leavingClearsWhatTheRoomLeftBehind() {
        let room = RoomModel()
        room.apply(.roomJoined(view()))
        room.skipTo = 84
        room.draft = "back in two"
        room.chosen = "source-a"
        room.chatHidden = true
        room.apply(.roomLeft(why: .left))
        #expect(room.skipTo == nil, "the old room's opening offers no Skip in the next")
        #expect(room.draft.isEmpty)
        #expect(room.chosen == nil)
        #expect(room.choices.isEmpty)
        #expect(!room.chatHidden)
        #expect(!room.active)
    }

    @Test func anInviteThatFailsLeavesNoInvitation() {
        let room = RoomModel()
        room.open(invite: "kaeri://room#invite.example")
        room.apply(.roomGlimpse(RoomGlimpse(by: "Ren", invite: "kaeri://room#invite.example")))
        #expect(room.invitation != nil)
        room.accept()
        room.apply(.roomLeft(why: .unreachable))
        #expect(room.invitation == nil, "a failed join shows its failure, not an empty room")
        #expect(!room.active)
        #expect(room.joinFailure == .unreachable)
    }

    @Test func aNewRoomAsksBeforeQuittingAgain() {
        let room = RoomModel()
        room.apply(.roomJoined(view()))
        room.leaveToQuit()
        room.apply(.roomLeft(why: .left))
        // The quit was cancelled after all; the next room asks again.
        room.apply(.roomJoined(view()))
        room.playerShown = true
        #expect(!room.leftToQuit)
        #expect(room.asksBeforeQuitting())
    }

    @Test func aRelayDownKeepsCarryOn() {
        let room = RoomModel()
        let ren = PausedFor(id: "m-ren", name: "Ren", reason: .loading, sinceMs: 0)
        room.apply(.roomJoined(view(
            relays: [RelayHealth(url: "wss://one.example", healthy: false), RelayHealth(url: "wss://two.example", healthy: true)],
            waiting: [ren], canCarryOn: true)))
        let status = room.status(now: 25_000)
        #expect(status.action == .carryOn(["Ren"]))
        #expect(status.waits)
    }

    @Test func aRelayDownIsSaidWhenNothingElseIs() {
        let room = RoomModel()
        room.apply(.roomJoined(view(relays: [
            RelayHealth(url: "wss://one.example", healthy: false), RelayHealth(url: "wss://two.example", healthy: true),
        ])))
        #expect(room.status(now: 0).detail == RoomCopy.relaysDown(1, 2))
    }

    @Test func openSourcesClosesThePlayerFirstAndKeepsTheRoom() {
        let room = RoomModel()
        var sent: [Command] = []
        var opened = 0
        var searched: [String] = []
        room.send = { sent.append($0) }
        room.showSources = { opened += 1 }
        room.showSearch = { searched.append($0) }
        room.closesPlayer = {}
        room.apply(.roomJoined(view()))
        room.playerShown = true
        room.openSources()
        #expect(sent == [.room(.closePlayer)], "the room is kept, not left")
        #expect(opened == 0, "nothing opens under a player still going")
        room.playerShown = false
        #expect(opened == 1)
        room.searchTitles()
        #expect(searched == ["Room"])
        #expect(sent.count == 1, "with the player gone, Search titles opens at once")
    }

    @Test func hiddenChatKeepsOnlyTheRoomsOwnLines() {
        let room = RoomModel()
        room.lines = [
            RoomLine(id: 1, kind: .system("Ren paused at 3:10")),
            RoomLine(id: 2, kind: .chat(from: "Mika", text: "that bridge again", you: false)),
            RoomLine(id: 3, kind: .reaction(from: "Ren", reaction: .laugh, at: "3:12")),
        ]
        #expect(room.shownLines.count == 3)
        room.chatHidden = true
        #expect(room.shownLines.map(\.id) == [1], "people's lines and reactions stay off the screen")
    }

    @Test(arguments: [
        ("relay.example", "wss://relay.example"),
        ("wss://relay.example/path", "wss://relay.example/path"),
        ("  wss://relay.example  ", "wss://relay.example"),
        ("ws://localhost:8080", "ws://localhost:8080"),
        ("ws://box.local", "ws://box.local"),
        ("localhost:7777", "wss://localhost:7777"),
    ])
    func relaysTheProtocolTakes(_ typed: String, _ sent: String) {
        #expect(RoomRelays.address(typed) == sent)
    }

    @Test(arguments: ["", "not-a-url", "ws://relay.example", "https://relay.example", "wss://", "wss://a b.example", "wss://user@relay.example"])
    func relaysItRefuses(_ typed: String) {
        #expect(RoomRelays.address(typed) == nil)
    }

    @Test func theListShowsAddressesWithoutWss() {
        #expect(RoomRelays.shown("wss://relay.example") == "relay.example")
        #expect(RoomRelays.shown("ws://localhost:8080") == "ws://localhost:8080")
    }
}
