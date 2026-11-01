// Notices: what the work tells the user beyond the answer it was asked for.

import KaeriKit
import Foundation
import Observation

/// One notice as the app keeps it: when it came, and a number to tell it from one with the same
/// words.
public struct NoticeItem: Identifiable, Equatable, Sendable {
    public let id: Int
    public let notice: Notice
    public let at: Date
    /// A button the app gave the notice, such as Undo, beside the engine's own hint.
    public let action: NoticeButton?

    init(id: Int, notice: Notice, at: Date, action: NoticeButton? = nil) {
        self.id = id
        self.notice = notice
        self.at = at
        self.action = action
    }

    public var isAlert: Bool { notice.kind == .alert }
}

/// A button on a notice that the app made itself: its words, and what it does.
public struct NoticeButton: Equatable, Sendable {
    public let title: String
    public let run: @MainActor @Sendable () -> Void
    private let token = UUID()

    public init(title: String, run: @escaping @MainActor @Sendable () -> Void) {
        self.title = title
        self.run = run
    }

    public static func == (a: NoticeButton, b: NoticeButton) -> Bool { a.token == b.token }
}

extension Notice {
    /// What the notice says in the person's language: the engine's message through the
    /// catalogue, or for a notice the app made itself, its text (already from the catalogue).
    public var words: String {
        msg?.text ?? text
    }
}

/// The notice capsule's current notice, and Messages, the log of every notice this session.
@Observable @MainActor
public final class NoticesModel {
    /// How many notices Messages keeps.
    public static let kept = 200

    /// What the capsule shows, until it is dismissed or another arrives.
    public private(set) var current: NoticeItem?
    /// Every notice, newest first.
    public private(set) var log: [NoticeItem] = []

    @ObservationIgnored private var count = 0
    @ObservationIgnored private let clock: @Sendable () -> Date

    public init(clock: @escaping @Sendable () -> Date = { Date() }) {
        self.clock = clock
    }

    func post(_ notice: Notice, action: NoticeButton? = nil) {
        count += 1
        let item = NoticeItem(id: count, notice: notice, at: clock(), action: action)
        current = item
        log.insert(item, at: 0)
        if log.count > Self.kept { log.removeLast(log.count - Self.kept) }
    }

    /// Keep `notice` in Messages without putting it up: the whole reason a screen failed for,
    /// which the screen says in a few plain words.
    func record(_ notice: Notice) {
        count += 1
        log.insert(NoticeItem(id: count, notice: notice, at: clock(), action: nil), at: 0)
        if log.count > Self.kept { log.removeLast(log.count - Self.kept) }
    }

    /// Take the capsule down, if it still shows `item`.
    public func dismiss(_ item: NoticeItem) {
        if current?.id == item.id { current = nil }
    }
}
