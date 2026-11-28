// Return in the player's corner: a stall, a found-times question or Undo before the room's cards.

@testable import KaeriPlayerUI
import Testing

@MainActor
struct RoomKeysTests {
    @Test func aStallOutranksTheRoomsSkipAndUpNext() {
        #expect(ReturnKey.action(corner: .stall, roomUpNext: true, roomSkip: true) == .switchSource)
    }

    @Test func aQuestionOrUndoKeepsReturnForItsOwnButtons() {
        #expect(ReturnKey.action(corner: .prompt, roomUpNext: false, roomSkip: true) == ReturnKey.none)
        #expect(ReturnKey.action(corner: .undo, roomUpNext: true, roomSkip: false) == ReturnKey.none)
        #expect(ReturnKey.cornerTaken(.prompt))
        #expect(!ReturnKey.cornerTaken(.skip))
        #expect(!ReturnKey.cornerTaken(nil))
    }

    @Test func theRoomsCardsComeBeforeThePlayersOwn() {
        #expect(ReturnKey.action(corner: nil, roomUpNext: true, roomSkip: true) == .roomUpNext)
        #expect(ReturnKey.action(corner: .skip, roomUpNext: false, roomSkip: true) == .roomSkip)
        #expect(ReturnKey.action(corner: .upNext, roomUpNext: false, roomSkip: false) == .upNext)
        #expect(ReturnKey.action(corner: nil, roomUpNext: false, roomSkip: false) == ReturnKey.none)
    }
}
