// Watch party chat: the conversation the engine keeps for the party, the field's draft, what the
// column over the video holds and for how long, and the reaction moment. The engine owns the
// rules (the server's length and chat switch, the waits, the echo, the five-second failure); this
// keeps what it said for the screens.

import KaeriKit
import KaeriStrings
import Foundation
import Observation

/// A chat line the column over the video holds: it was shown at `shown`, once it was in view.
public struct ChatColumnLine: Identifiable, Equatable, Sendable {
    public let id: UInt64
    public let from: String
    public let text: String
    public let you: Bool
    /// How long it stays once in view.
    let hold: Duration
    public internal(set) var shown: ContinuousClock.Instant?
}

/// The reaction moment over the video: one glyph and a name, for a moment.
public struct ReactionMoment: Identifiable, Equatable, Sendable {
    public let id: Int
    public let from: String
    public let you: Bool
    public let reaction: Reaction
}

/// A row of the conversation as a list shows it.
public struct ChatRow: Identifiable, Equatable, Sendable {
    public enum Kind: Equatable, Sendable {
        case said(from: String, text: String, you: Bool)
        /// "Ren and Mika laughed at 12:04".
        case reacted(String)
    }

    public let id: UInt64
    public let kind: Kind
}

@Observable @MainActor
public final class PartyChat {
    /// The rows the column over the video shows at once, notices included.
    public static let columnRows = 3
    /// What the field takes at most when the server has said nothing.
    public static let defaultLength = 150
    /// Characters left at which the field shows its count.
    public static let countFrom = 20
    /// The key the Hide chat switch is kept under on this device.
    public static let hiddenKey = "kaeri.hideChat"
    /// The key that says the lobby's privacy line has been seen: a message was sent.
    public static let privacySeenKey = "kaeri.chatPrivacySeen"
    /// A reaction moment's time on the video.
    /// Mutable for the shot scenes, which hold it up longer; nothing else sets it.
    nonisolated(unsafe) public static var momentStays: Duration = .milliseconds(1500)

    public private(set) var conversation = PartyConversation(lines: [], marks: [], failed: nil)
    /// What the server lets this device do, once the engine has said.
    public private(set) var state: PartyChatState?
    /// The lines the column holds, oldest first.
    public private(set) var column: [ChatColumnLine] = []
    /// The newest typed line, for the party bar on iPhone's Home.
    public private(set) var latest: ChatColumnLine?
    /// The reaction moment up now.
    public private(set) var moment: ReactionMoment?
    /// The field's text, shared by the player's field, the popover and the lobby.
    public var draft = ""
    /// Hide chat: lines, reactions, ticks and the field stay off the screens, in every party.
    public var hidden: Bool {
        didSet { defaults.set(hidden, forKey: Self.hiddenKey) }
    }

    /// The lobby says once, until a first message is sent from this device, that messages go
    /// through the room's server.
    public private(set) var privacySeen: Bool
    @ObservationIgnored private var seen: UInt64 = 0
    @ObservationIgnored private var moments = 0
    @ObservationIgnored private var restored: String?
    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored var send: @MainActor (Command) -> Void = { _ in }

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        hidden = defaults.bool(forKey: Self.hiddenKey)
        privacySeen = defaults.bool(forKey: Self.privacySeenKey)
    }

    // MARK: What the screens ask

    /// Whether the server takes chat. True until the engine says otherwise.
    public var allowed: Bool { state?.allowed ?? true }
    /// Whether anything of chat shows: the setting is off and the server takes it.
    public var shown: Bool { !hidden && allowed }
    /// The characters one message may have.
    public var maxLength: Int {
        let length = Int(state?.maxLength ?? 0)
        return length > 0 ? length : Self.defaultLength
    }
    /// Seconds until a message may be sent, or 0.
    public var wait: Int { Int(state?.waitSeconds ?? 0) }
    /// Seconds until a reaction may be sent, or 0.
    public var reactWait: Int { Int(state?.reactWaitSeconds ?? 0) }
    /// Characters left, once few enough to show.
    public var remaining: Int? {
        let left = maxLength - draft.count
        return left <= Self.countFrom ? max(left, 0) : nil
    }
    /// Whether Return would send.
    public var canSend: Bool {
        allowed && wait == 0 && !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
    /// The send that came back from the server with nothing: its text, until it is sent again.
    public var failed: String? { conversation.failed }
    /// Whether anyone is muted.
    public func isMuted(_ name: String) -> Bool { state?.muted.contains(name) ?? false }

    /// The conversation as rows: said lines and reactions, those of muted members left out.
    public var rows: [ChatRow] {
        conversation.lines.compactMap { line in
            guard !line.muted else { return nil }
            switch line.body {
            case .said(let text):
                return ChatRow(id: line.id, kind: .said(from: line.from, text: text, you: line.you))
            case .reacted(let reaction, let at, let with):
                let names = PartyChat.names([line.you ? L.partyChatYou : line.from] + with)
                return ChatRow(id: line.id, kind: .reacted(L.partyReactedAt(
                    line: Self.reacted(reaction, names), time: PartyChat.clock(at))))
            }
        }
    }

    /// The seek bar's ticks: where the room reacted, in seconds, merged within `merge` seconds of
    /// each other, with the names for the time label.
    public func ticks(duration: Double, width: Double, merge points: Double = 4) -> [ReactionTick] {
        guard duration > 0, width > 0 else { return [] }
        var ticks: [ReactionTick] = []
        for mark in conversation.marks.sorted(by: { $0.at < $1.at }) where mark.at >= 0 && mark.at <= duration {
            let x = mark.at / duration * width
            if var last = ticks.last, x - last.x <= points {
                if !last.names.contains(mark.from) { last.names.append(mark.from) }
                ticks[ticks.count - 1] = last
            } else {
                ticks.append(ReactionTick(at: mark.at, x: x, names: [mark.from]))
            }
        }
        return ticks
    }

    // MARK: Asking

    /// Return in the field: the text goes to the room and the field clears; the line shows when
    /// the server echoes it.
    public func submit() {
        guard canSend else { return }
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        draft = ""
        restored = nil
        if !privacySeen {
            privacySeen = true
            defaults.set(true, forKey: Self.privacySeenKey)
        }
        send(.party(.say(text: text)))
    }

    /// Try again: the text that did not come back is sent once more.
    public func retry() {
        let text = (failed ?? draft).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, allowed else { return }
        draft = ""
        restored = nil
        send(.party(.say(text: text)))
    }

    /// A reaction glyph pressed.
    public func react(_ reaction: Reaction) {
        guard allowed, reactWait == 0 else { return }
        send(.party(.react(reaction: reaction)))
    }

    /// Mute, or Unmute, a member for this party on this device.
    public func mute(_ name: String, _ muted: Bool) {
        send(.party(.mute(name: name, muted: muted)))
    }

    /// The lobby's privacy line shows again, as on a device that has sent nothing yet.
    public func forgetPrivacySeen() {
        privacySeen = false
        defaults.removeObject(forKey: Self.privacySeenKey)
    }

    /// Show chat: the setting goes off.
    public func show() { hidden = false }

    // MARK: The column

    /// Put the column's lines in view, as far as `room` rows allow, and take away the ones whose
    /// time is up. Returns when to look again, or nil when there is nothing to wait for.
    @discardableResult
    public func settle(room: Int, now: ContinuousClock.Instant = .now) -> ContinuousClock.Instant? {
        var kept: [ChatColumnLine] = []
        var inView = 0
        var next: ContinuousClock.Instant?
        for var line in column {
            if let shown = line.shown, now >= shown + line.hold { continue }
            if line.shown == nil, inView < room { line.shown = now }
            if let shown = line.shown {
                inView += 1
                let due = shown + line.hold
                if next == nil || due < next! { next = due }
            }
            kept.append(line)
        }
        if kept != column { column = kept }
        return next
    }

    /// The lines waiting out of view: "2 more in chat".
    public func folded(room: Int) -> Int {
        max(0, column.count - max(room, 0))
    }

    /// Take the reaction moment down, if it is still `id`.
    public func clearMoment(_ id: Int) {
        if moment?.id == id { moment = nil }
    }

    // MARK: The engine

    func apply(_ event: Event) {
        switch event {
        case .partyChat(let conversation):
            heard(conversation)
        case .partyChatState(let state):
            self.state = state
        case .partyReaction(let reaction):
            guard shown else { break }
            moments += 1
            moment = ReactionMoment(id: moments, from: reaction.from, you: reaction.you, reaction: reaction.reaction)
        default:
            break
        }
    }

    /// The party is over: nothing of it stays.
    func left() {
        conversation = PartyConversation(lines: [], marks: [], failed: nil)
        state = nil
        column = []
        latest = nil
        moment = nil
        draft = ""
        seen = 0
        restored = nil
    }

    private func heard(_ next: PartyConversation) {
        if next.lines.isEmpty || (next.lines.last?.id ?? 0) < seen { seen = 0 }
        conversation = next
        // The text of a send that never came back goes back into the field, once.
        if let failed = next.failed, restored != failed {
            restored = failed
            if draft.isEmpty { draft = failed }
        } else if next.failed == nil {
            restored = nil
        }
        for line in next.lines where line.id > seen {
            seen = max(seen, line.id)
            guard !line.muted, case .said(let text) = line.body else { continue }
            let item = ChatColumnLine(
                id: line.id, from: line.from, text: text, you: line.you, hold: Self.hold(text), shown: nil)
            column.append(item)
            latest = item
        }
        // Lines of a member muted since leave the column.
        let live = Set(next.lines.filter { !$0.muted }.map(\.id))
        let trimmed = column.filter { live.contains($0.id) }
        if trimmed.count != column.count { column = trimmed }
    }

    // MARK: Words

    /// How long a line holds: 4 s, plus 1 s for every 10 words past 12, as notices do.
    static func hold(_ text: String) -> Duration {
        let words = text.split(whereSeparator: \.isWhitespace).count
        return .seconds(4 + max(0, words - 12) / 10)
    }

    /// "Ren and Mika": the names of one moment.
    static func names(_ names: [String]) -> String {
        switch names.count {
        case 0: ""
        case 1: names[0]
        case 2: L.partyNamesTwo(first: names[0], second: names[1])
        case 3: L.partyNamesThree(first: names[0], second: names[1], third: names[2])
        default: L.partyNamesOthers(first: names[0], second: names[1], count: names.count - 2)
        }
    }

    /// "Ren and Mika laughed".
    public static func reacted(_ reaction: Reaction, _ name: String) -> String {
        switch reaction {
        case .laugh: L.partyReactedLaugh(name: name)
        case .gasp: L.partyReactedGasp(name: name)
        case .clap: L.partyReactedClap(name: name)
        case .cry: L.partyReactedCry(name: name)
        }
    }

    /// 12:04, or 1:02:03.
    public static func clock(_ seconds: Double) -> String {
        let total = max(0, Int(seconds))
        let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60)
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%d:%02d", m, s)
    }

    /// Whether `text` is the engine's own notice for a typed line ("Mika: starting soon"), which
    /// the column shows itself.
    public func isChatNotice(_ text: String, members: [String]) -> Bool {
        guard let colon = text.range(of: ": ") else { return false }
        let name = String(text[..<colon.lowerBound])
        return members.contains(name) || conversation.lines.contains { $0.from == name }
    }
}

/// A tick on the seek bar: where one or more reactions were made.
public struct ReactionTick: Equatable, Sendable {
    public var at: Double
    public var x: Double
    public var names: [String]

    /// "12:04 · Ren and Mika", for the time label.
    @MainActor public var label: String {
        L.partyTickLabel(time: PartyChat.clock(at), names: PartyChat.names(names))
    }
}
