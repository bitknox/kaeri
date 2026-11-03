// The party's chat as the store holds it: the column over the video, the rows of the conversation,
// the field's limits and the seek bar's ticks.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct PartyChatTests {
    func chat() -> PartyChat {
        PartyChat(defaults: UserDefaults(suiteName: "chat-tests-\(UUID().uuidString)")!)
    }

    func said(_ id: UInt64, _ from: String, _ text: String, you: Bool = false, muted: Bool = false) -> PartyChatLine {
        PartyChatLine(id: id, from: from, you: you, body: .said(text: text), muted: muted)
    }

    func hear(_ chat: PartyChat, _ lines: [PartyChatLine], marks: [PartyReactionMark] = [], failed: String? = nil) {
        chat.apply(.partyChat(PartyConversation(lines: lines, marks: marks, failed: failed)))
    }

    @Test func theColumnHoldsThreeRowsAndFoldsTheRest() {
        let chat = chat()
        hear(chat, (1...5).map { said(UInt64($0), "Mika", "line \($0)") })
        chat.settle(room: 3, now: ContinuousClock.now)
        #expect(chat.column.filter { $0.shown != nil }.count == 3)
        #expect(chat.folded(room: 3) == 2)
    }

    @Test func aLineLeavesWhenItsTimeIsUp() {
        let chat = chat()
        hear(chat, [said(1, "Mika", "hello")])
        let now = ContinuousClock.now
        chat.settle(room: 3, now: now)
        chat.settle(room: 3, now: now + .seconds(5))
        #expect(chat.column.isEmpty)
    }

    @Test func mutedLinesStayOutOfRowsAndColumn() {
        let chat = chat()
        hear(chat, [said(1, "Ren", "psst", muted: true), said(2, "Mika", "hi")])
        #expect(chat.rows.count == 1)
        #expect(chat.column.map(\.id) == [2])
    }

    @Test func aFailedSendGoesBackIntoTheFieldOnce() {
        let chat = chat()
        hear(chat, [], failed: "the music")
        #expect(chat.draft == "the music")
        chat.draft = ""
        hear(chat, [], failed: "the music")
        #expect(chat.draft.isEmpty)
    }

    @Test func theCountShowsNearTheLimit() {
        let chat = chat()
        #expect(chat.remaining == nil)
        chat.draft = String(repeating: "a", count: 135)
        #expect(chat.remaining == 15)
    }

    @Test func hideChatTurnsEverythingOff() {
        let chat = chat()
        #expect(chat.shown)
        chat.hidden = true
        #expect(!chat.shown)
    }

    @Test func nearReactionsMergeIntoOneTick() {
        let chat = chat()
        hear(chat, [], marks: [
            PartyReactionMark(at: 10, from: "Ren", reaction: .laugh),
            PartyReactionMark(at: 10.3, from: "Mika", reaction: .laugh),
            PartyReactionMark(at: 40, from: "Sol", reaction: .clap),
        ])
        let ticks = chat.ticks(duration: 60, width: 600)
        #expect(ticks.count == 2)
        #expect(ticks[0].names == ["Ren", "Mika"])
    }

    @Test func theClockReadsMinutesAndHours() {
        #expect(PartyChat.clock(724) == "12:04")
        #expect(PartyChat.clock(3723) == "1:02:03")
    }
}
