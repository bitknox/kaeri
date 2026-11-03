// Skip intro in the player model: what the engine says of the marking bar and the prompts, and
// the commands the viewer's presses send back.

import KaeriKit
import KaeriStore
import KaeriStrings
import Foundation
import Testing

@testable import KaeriPlayerUI

@MainActor
struct SkipMarkingTests {
    private func view(
        step: SkipMarkStep = .review, start: Double? = 91, end: Double? = 181, check: SkipMarkCheck = .fine,
        notes: [SkipMarkNote] = [], canSave: Bool = true
    ) -> SkipMarkView {
        SkipMarkView(
            kind: .opening, step: step, start: start, end: end, length: (end ?? 0) - (start ?? 0), check: check,
            notes: notes, canSave: canSave, editing: false, previewing: false)
    }

    @Test func theBarOpensAndClosesWithTheEnginesViewAndHoldsTheControls() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.apply(.playerSkipMark(playback: 1, view: view()))
        #expect(rig.model.skipMark?.step == .review)
        rig.step(.seconds(10))
        #expect(rig.model.controlsVisible)
        rig.model.apply(.playerSkipMark(playback: 1, view: nil))
        #expect(rig.model.skipMark == nil)
    }

    @Test func pressesBecomeTheEnginesCommands() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.beginSkipMark()
        rig.model.setSkipMarkKind(.recap)
        rig.model.markSkipEdge()
        rig.model.nudgeSkipEdge(.end, by: -1000)
        rig.model.previewSkipEdges()
        rig.model.cancelSkipMark()
        rig.model.clearMySkip(.opening)
        rig.model.reportSkip(.ending)
        #expect(rig.controls == [
            .beginSkipMark(kind: nil), .setSkipMarkKind(kind: .recap), .markSkipEdge,
            .nudgeSkipEdge(edge: .end, byMs: -1000), .previewSkipEdges, .cancelSkipMark,
            .clearMySkip(kind: .opening), .reportSkip(kind: .ending),
        ])
    }

    @Test func saveIsOffWhenTheEngineSaysSo() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.apply(.playerSkipMark(playback: 1, view: view(end: 94, check: .tooShort, canSave: false)))
        rig.model.saveSkipMark()
        #expect(rig.controls.isEmpty)
        rig.model.apply(.playerSkipMark(playback: 1, view: view()))
        rig.model.saveSkipMark()
        #expect(rig.controls == [.saveSkipMark])
    }

    @Test func marksAreRefusedInAWatchParty() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.marksBlocked = true
        rig.model.beginSkipMark()
        #expect(rig.controls.isEmpty)
        #expect(!rig.model.canMarkSkips)
    }

    @Test func aPromptSitsInTheCornerAndLapsesAfterTwelveSeconds() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.apply(.playerSkipPrompt(playback: 1, prompt: .wrongTimes(kind: .ending)))
        #expect(rig.model.corner == .prompt)
        rig.step(.seconds(9))
        #expect(rig.model.skipPrompt != nil)
        rig.step(.seconds(4))
        #expect(rig.model.skipPrompt == nil)
        #expect(rig.controls == [.dismissSkipPrompt])
    }

    @Test func theFoundTimesCardWaitsWhilePausedAndAnswersAreSent() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.apply(.playerSkipPrompt(playback: 1, prompt: .foundEdges(kind: .opening, episode: 3)))
        rig.model.togglePause()
        rig.step(.seconds(30))
        #expect(rig.model.skipPrompt != nil, "a pause waits")
        rig.model.togglePause()
        rig.step(.seconds(11))
        #expect(rig.model.skipPrompt != nil)
        rig.step(.seconds(2))
        #expect(rig.model.skipPrompt == nil, "12 s of playback, then it goes unanswered")
        #expect(rig.controls.last == .dismissSkipPrompt)
    }

    @Test func theFoundTimesCardsAnswersAreCommandsAndTheAnswerLinesLastFourSeconds() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.apply(.playerSkipPrompt(playback: 1, prompt: .foundEdges(kind: .opening, episode: 3)))
        rig.model.confirmFoundSkip(.opening)
        #expect(rig.model.skipPrompt != nil, "the card answers in place when the engine says so")
        rig.model.apply(.playerSkipPrompt(playback: 1, prompt: .foundSaved(kind: .opening, episode: 3)))
        rig.step(.seconds(3))
        #expect(rig.model.skipPrompt != nil)
        rig.step(.seconds(2))
        #expect(rig.model.skipPrompt == nil)
        rig.model.apply(.playerSkipPrompt(playback: 1, prompt: .foundEdges(kind: .ending, episode: 3)))
        rig.model.adjustFoundSkip(.ending)
        #expect(rig.model.skipPrompt == nil)
        rig.model.apply(.playerSkipPrompt(playback: 1, prompt: .foundEdges(kind: .opening, episode: 3)))
        rig.model.neverAskWasOpening()
        #expect(rig.model.skipPrompt != nil, "kept for the engine's FoundNever, so the card never goes empty")
        #expect(rig.controls.filter { $0 != .dismissSkipPrompt } == [
            .confirmFoundSkip(kind: .opening), .adjustFoundSkip(kind: .ending), .neverAskWasOpening,
        ])
    }

    @Test func theSharingQuestionIsASheetAndItsAnswerIsSent() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.apply(.playerSkipPrompt(playback: 1, prompt: .share))
        #expect(rig.model.corner == nil)
        rig.step(.seconds(30))
        #expect(rig.model.skipPrompt != nil, "it waits for an answer")
        rig.model.answerShareSkipTimes(true)
        #expect(rig.controls == [.answerShareSkipTimes(share: true)])
        #expect(rig.model.skipPrompt == nil)
    }

    @Test func markFromAPromptOpensTheBar() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.apply(.playerSkipPrompt(playback: 1, prompt: .wasOpening(start: 30, end: 120)))
        rig.model.markFromPrompt()
        #expect(rig.controls == [.beginSkipMark(kind: .opening)])
    }

    @Test func noticesAreSaidOnceOverTheVideo() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.apply(.playerSkipNotice(playback: 1, notice: .confirmed))
        #expect(rig.model.alert?.text == L.playerMarkConfirmed)
    }

    @Test func theReviewsLinesFollowTheEnginesNotes() {
        let lines = SkipWords.lines(view(check: .tooLong(most: 180), notes: [.shares, .replaces(start: 88, end: 178)]))
        #expect(lines == [
            L.playerMarkTooLong(kind: L.playerMarkKindOpeningPlural, most: "3 min"),
            L.playerMarkShares,
            L.playerMarkReplaces(start: "1:28", end: "2:58"),
        ])
        #expect(SkipWords.span(90) == "1 min 30 s")
    }

    @Test func theSkipTimesPanelNamesWhereEachTimeCameFrom() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.apply(.playerTimeline(playback: 1, marks: [
            TimelineMark(kind: .opening, start: 40, end: 129, origin: .yours),
            TimelineMark(kind: .ending, start: 1310, end: 1400, origin: .aniSkip),
        ]))
        let rows = rig.model.skipTimeRows
        #expect(rows.map(\.origin) == [L.playerSkipYours, L.playerSkipFromAniskip, L.playerSkipNone])
        #expect(rows.map(\.canClear) == [true, false, false])
        #expect(rows.map(\.canReport) == [false, true, false])
    }

    @Test func foundTimesShowTheirOriginAnAdjustAndWhyNoneWereFoundWhenPaused() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.apply(.playerTimeline(playback: 1, marks: [
            TimelineMark(kind: .opening, start: 91, end: 180, origin: .found),
        ]))
        rig.model.skipFindPause = { .lowPower }
        let rows = rig.model.skipTimeRows
        #expect(rows.map(\.origin) == [L.playerSkipFound, L.playerSkipNone, L.playerSkipNone])
        #expect(rows.map(\.canAdjust) == [true, false, false])
        #expect(rows.map { rig.model.skipFindPauseWords(for: $0) } == [nil, L.playerSkipPausedLowPower, nil])
        #expect(SkipWords.lines(view(notes: [.found, .shares])) == [L.playerMarkFound, L.playerMarkShares])
    }
}
