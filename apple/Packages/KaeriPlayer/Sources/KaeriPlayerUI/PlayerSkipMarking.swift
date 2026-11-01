// Skip intro, the player's half: marking an opening, ending or recap, the prompts about skip
// times, and the Skip times panel. The engine owns every step; this keeps what it last said and
// sends the viewer's presses back.

import KaeriKit
import KaeriStore
import KaeriStrings
import Foundation

extension PlayerTiming {
    /// "Was that the opening?" and "Wrong opening times?" stay this long.
    public static let promptStays: Duration = .seconds(8)
    /// How far the arrow keys step an edge, and the shifted ones.
    public static let nudge: Int64 = 1000
    public static let nudgeBig: Int64 = 5000
}

/// A prompt about the skip times and when it came up.
public struct SkipPromptOffer: Equatable, Sendable {
    public var prompt: SkipPrompt
    public var since: ContinuousClock.Instant
}

extension SkipPrompt {
    /// Whether it sits in the corner as a card; the sharing question is a sheet.
    var isCard: Bool {
        if case .share = self { return false }
        return true
    }
}

extension SkipKind {
    /// "Opening", for the segmented control and the review's title.
    public var title: String {
        switch self {
        case .opening: L.playerMarkKindOpening
        case .ending: L.playerMarkKindEnding
        case .recap: L.playerMarkKindRecap
        }
    }

    /// "opening", for a sentence.
    public var inline: String {
        switch self {
        case .opening: L.playerMarkKindOpeningInline
        case .ending: L.playerMarkKindEndingInline
        case .recap: L.playerMarkKindRecapInline
        }
    }

    /// "openings", for the limit line.
    var plural: String {
        switch self {
        case .opening: L.playerMarkKindOpeningPlural
        case .ending: L.playerMarkKindEndingPlural
        case .recap: L.playerMarkKindRecapPlural
        }
    }

    public static let marked: [SkipKind] = [.opening, .ending, .recap]
}

extension SkipNotice {
    /// What the player says of it, once, over the video.
    var words: String {
        switch self {
        case .saved(let kind): L.playerMarkSaved(kind: kind.inline)
        case .savedHere(let kind, let most): L.playerMarkTooLong(kind: kind.plural, most: SkipWords.span(most))
        case .confirmed: L.playerMarkConfirmed
        case .reported: L.playerSkipReported
        case .hiddenHere: L.playerSkipHiddenHere
        }
    }
}

/// The words of the marking bar that come from numbers.
public enum SkipWords {
    /// "1 min 30 s", "3 min", "45 s".
    public static func span(_ seconds: Double) -> String {
        let total = Int(max(0, seconds).rounded())
        let (minutes, secs) = (total / 60, total % 60)
        switch (minutes, secs) {
        case (0, _): return "\(secs) s"
        case (_, 0): return "\(minutes) min"
        default: return "\(minutes) min \(secs) s"
        }
    }

    /// The step line: "Play to where the opening starts, then mark it."
    public static func step(_ view: SkipMarkView) -> String {
        switch view.step {
        case .start: L.playerMarkStepStart(kind: view.kind.inline)
        case .end: L.playerMarkStepEnd(kind: view.kind.inline)
        case .review: L.playerMarkReviewTitle(kind: view.kind.title, length: span(view.length))
        }
    }

    /// The review's lines under the edges, in order, with the check first when it has one.
    public static func lines(_ view: SkipMarkView) -> [String] {
        var lines: [String] = []
        switch view.check {
        case .fine: break
        case .tooShort: if view.step == .review { lines.append(L.playerMarkTooShort) }
        case .tooLong(let most): lines.append(L.playerMarkTooLong(kind: view.kind.plural, most: span(most)))
        }
        for note in view.notes {
            switch note {
            case .shares: lines.append(L.playerMarkShares)
            case .replaces(let start, let end):
                lines.append(L.playerMarkReplaces(start: PlayerClock.label(start), end: PlayerClock.label(end)))
            case .noEntry: lines.append(L.playerMarkNoMal)
            }
        }
        return lines
    }
}

extension PlayerModel {
    // MARK: The engine's words

    func applySkipMark(_ view: SkipMarkView?) {
        skipMark = view
        // The bar keeps the controls up while it is open.
        hold("skip-mark", view != nil)
        if view != nil { skipTimesOpen = false }
    }

    func applySkipPrompt(_ prompt: SkipPrompt?) {
        skipPrompt = prompt.map { SkipPromptOffer(prompt: $0, since: time.now()) }
        if prompt != nil { undo = nil }
    }

    func expireSkipPrompt(at now: ContinuousClock.Instant) {
        guard let offer = skipPrompt, offer.prompt.isCard, now - offer.since >= PlayerTiming.promptStays else { return }
        dismissSkipPrompt()
    }

    // MARK: The viewer's presses

    /// Whether marking can start now.
    public var canMarkSkips: Bool { isPresented && !marksBlocked && playback != nil }

    /// "Mark skip times": the bar opens on the kind the playhead suggests.
    public func beginSkipMark(_ kind: SkipKind? = nil) {
        input()
        guard canMarkSkips else { return }
        skipTimesOpen = false
        control(.beginSkipMark(kind: kind))
    }

    public func setSkipMarkKind(_ kind: SkipKind) {
        input()
        control(.setSkipMarkKind(kind: kind))
    }

    /// Return: mark the edge the step names.
    public func markSkipEdge() {
        input()
        control(.markSkipEdge)
    }

    public func nudgeSkipEdge(_ edge: SkipEdge, by milliseconds: Int64) {
        input()
        control(.nudgeSkipEdge(edge: edge, byMs: milliseconds))
    }

    public func previewSkipEdges() {
        input()
        control(.previewSkipEdges)
    }

    public func saveSkipMark() {
        input()
        guard skipMark?.canSave == true else { return }
        skipSaves += 1
        control(.saveSkipMark)
    }

    /// Esc: one step back, and from the first, closed.
    public func cancelSkipMark() {
        input()
        control(.cancelSkipMark)
    }

    public func clearMySkip(_ kind: SkipKind) {
        input()
        control(.clearMySkip(kind: kind))
    }

    public func reportSkip(_ kind: SkipKind) {
        input()
        skipPrompt = nil
        control(.reportSkip(kind: kind))
    }

    public func dismissSkipPrompt() {
        skipPrompt = nil
        control(.dismissSkipPrompt)
    }

    public func neverAskWasOpening() {
        input()
        skipPrompt = nil
        control(.neverAskWasOpening)
    }

    /// "Mark" on either prompt: the bar opens, on the stretch just skipped or on the kind.
    public func markFromPrompt() {
        input()
        guard let offer = skipPrompt else { return }
        skipPrompt = nil
        switch offer.prompt {
        case .wasOpening: control(.beginSkipMark(kind: .opening))
        case .wrongTimes(let kind): control(.beginSkipMark(kind: kind))
        case .share: break
        }
    }

    public func answerShareSkipTimes(_ share: Bool) {
        input()
        skipPrompt = nil
        control(.answerShareSkipTimes(share: share))
    }

    // MARK: The Skip times panel

    /// What the panel lists: each kind with its time and where it came from.
    public var skipTimeRows: [SkipTimeRow] {
        let marks = current.flatMap { timelines[$0] } ?? []
        return SkipKind.marked.map { kind in
            SkipTimeRow(kind: kind, mark: marks.first { $0.kind == kind })
        }
    }

    public func toggleSkipTimes() {
        input()
        skipTimesOpen.toggle()
    }

    public func closeSkipTimes() { skipTimesOpen = false }
}

/// One line of the Skip times panel.
public struct SkipTimeRow: Equatable, Sendable, Identifiable {
    public var kind: SkipKind
    public var mark: TimelineMark?
    public var id: String { kind.inline }

    /// "1:31 to 3:01", or nothing.
    public var times: String? {
        mark.map { "\(PlayerClock.label($0.start)) – \(PlayerClock.label($0.end))" }
    }

    public var origin: String {
        switch mark?.origin {
        case .file: L.playerSkipFromFile
        case .yours: L.playerSkipYours
        case .aniSkip: L.playerSkipFromAniskip
        case nil: L.playerSkipNone
        }
    }

    public var canReport: Bool { mark?.origin == .aniSkip }
    public var canClear: Bool { mark?.origin == .yours }
}
