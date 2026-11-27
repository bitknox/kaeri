// ⌘Q in a room: the question's Leave leaves the room and then quits, and the quit that follows
// asks nothing more. Outside a room nothing is asked.

@testable import KaeriStore
import KaeriKit
import Testing

@MainActor
struct RoomQuitTests {
    @Test func outsideARoomQuittingAsksNothing() {
        let room = RoomModel()
        room.playerShown = true
        #expect(!room.asksBeforeQuitting())
        #expect(!room.asksInAlertBeforeQuitting)
        #expect(room.leaving == nil)
    }

    @Test func leaveFromTheQuitQuestionLeavesThenQuits() {
        let room = RoomModel()
        var sent: [Command] = []
        var quit = 0
        var closed = 0
        room.send = { sent.append($0) }
        room.quits = { quit += 1 }
        room.closesPlayer = { closed += 1 }
        room.leaving = .quit
        room.leave()
        #expect(sent == [.room(.leave)])
        #expect(quit == 1)
        #expect(closed == 0, "the quit takes the player with it")
        #expect(room.leaving == nil)
        #expect(room.leftToQuit)
        #expect(!room.asksBeforeQuitting(), "the quit that follows asks nothing more")
    }

    @Test func leaveFromTheRoomStillOnlyLeaves() {
        let room = RoomModel()
        var sent: [Command] = []
        var quit = 0
        room.send = { sent.append($0) }
        room.quits = { quit += 1 }
        room.leaving = .leave
        room.leave()
        #expect(sent == [.room(.leave)])
        #expect(quit == 0)
        #expect(!room.leftToQuit)
    }
}
