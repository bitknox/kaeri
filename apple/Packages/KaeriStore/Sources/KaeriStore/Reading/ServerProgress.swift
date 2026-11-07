// Did my server get it? What became of the reading progress sent back to a Komga or Kavita server,
// as one line on the page between chapters and one on the server's row in Your sources. The
// engine keeps the queue and says the facts; this decides what the lines say and when, following
// the waiting rule: nothing for 600 ms, then Sending for at least 0.3 s.

import Foundation
import KaeriKit
import Observation

/// Why a send was not saved, as the line words it.
public enum ProgressWhyNot: Sendable, Equatable {
    case key, password, gone, gaveUp
}

extension WriteBackProblem {
    /// The reason a problem is held for, or nil for a problem that passes.
    var whyNot: ProgressWhyNot? {
        switch self {
        case .refusedKey: .key
        case .refusedPassword: .password
        case .seriesGone: .gone
        case .noAnswerForADay: .gaveUp
        case .offline, .noAnswer, .serverError, .notSetToReceive: nil
        }
    }
}

/// The line on the page between chapters. `host` is the server's host, as the line names it.
public enum ServerLine: Sendable, Equatable {
    case saved(host: String)
    case sending(host: String)
    case offline(host: String)
    case retrying(host: String)
    case failed(host: String, why: ProgressWhyNot)

    public var host: String {
        switch self {
        case .saved(let host), .sending(let host), .offline(let host), .retrying(let host), .failed(let host, _): host
        }
    }

    /// Retrying and Failed carry Try again.
    public var canRetry: Bool {
        switch self {
        case .retrying, .failed: true
        default: false
        }
    }

    /// What a result says on the line. Nil for a server set not to receive progress: that was a
    /// choice, so the page says nothing.
    static func of(_ result: WriteBackResult, host: String) -> ServerLine? {
        switch result {
        case .saved: .saved(host: host)
        case .waiting(let problem), .notSaved(let problem): of(problem, host: host)
        }
    }

    static func of(_ problem: WriteBackProblem, host: String) -> ServerLine? {
        switch problem {
        case .notSetToReceive: nil
        case .offline: .offline(host: host)
        default: problem.whyNot.map { .failed(host: host, why: $0) } ?? .retrying(host: host)
        }
    }
}

/// The second line of a server's row.
public enum ServerRowLine: Sendable, Equatable {
    /// Not set to receive: "Progress stays on this iPhone".
    case off
    case saved(at: Int64)
    case sending(count: Int)
    case offline(count: Int)
    case retrying(count: Int)
    case failed(count: Int, why: ProgressWhyNot)

    /// Retrying and Failed carry Try again, which a press dims until the answer.
    public var canRetry: Bool {
        switch self {
        case .retrying, .failed: true
        default: false
        }
    }
}

/// Where a server's queue stands, read from its row.
enum ServerStand: Equatable {
    /// Nothing to say: no queue and nothing saved yet, or the server is gone.
    case none
    /// Set not to receive progress.
    case off
    /// Chapters wait with no problem: the queue is sending them, or about to.
    case going(Int)
    case saved(Int64)
    case offline(Int)
    case retrying(Int)
    case failed(Int, ProgressWhyNot)

    init(_ row: BookServerRow?) {
        guard let row else { self = .none; return }
        if !row.reportProgress || row.progressProblem == .notSetToReceive { self = .off; return }
        let count = Int(row.progressPending)
        if count == 0 {
            self = row.progressSavedAt.map { .saved($0) } ?? .none
            return
        }
        guard let problem = row.progressProblem else { self = .going(count); return }
        if problem == .offline {
            self = .offline(count)
        } else if let why = problem.whyNot {
            self = .failed(count, why)
        } else {
            self = .retrying(count)
        }
    }

    /// What the row says once settled; nil while its queue is going.
    var rowLine: ServerRowLine? {
        switch self {
        case .none, .going: nil
        case .off: .off
        case .saved(let at): .saved(at: at)
        case .offline(let count): .offline(count: count)
        case .retrying(let count): .retrying(count: count)
        case .failed(let count, let why): .failed(count: count, why: why)
        }
    }
}

extension BookServerRow {
    fileprivate var stamp: [Int64?] { [progressSavedAt, progressProblemAt] }

    /// The host the lines name: "books.home.example".
    public var progressHost: String { URL(string: address)?.host() ?? address }
}

/// The waiting rule's timings, shortened by the tests.
public enum ProgressWaiting {
    @MainActor public static var wait: Duration = .milliseconds(600)
    @MainActor public static var hold: Duration = .milliseconds(300)
}

/// One waiting line: it shows `ProgressWaiting.wait` after `begin`, and an answer given through
/// `end` waits until it has shown for `ProgressWaiting.hold`.
@MainActor
final class WaitingLine {
    private(set) var shown = false
    private var gate: Task<Void, Never>?

    init(onShow: @escaping @MainActor () -> Void) {
        gate = Task { @MainActor [weak self] in
            try? await Task.sleep(for: ProgressWaiting.wait)
            guard !Task.isCancelled, let self else { return }
            self.shown = true
            onShow()
            try? await Task.sleep(for: ProgressWaiting.hold)
        }
    }

    /// The answer came: `deliver` runs at once, or after the waiting line's hold when it showed.
    func end(_ deliver: @escaping @MainActor () -> Void) {
        guard let held = gate else { deliver(); return }
        gate = nil
        if !shown {
            held.cancel()
            deliver()
            return
        }
        Task { @MainActor in
            await held.value
            deliver()
        }
    }

    func cancel() {
        gate?.cancel()
        gate = nil
        shown = false
    }
}

/// A Try again waiting for the engine's answer. The engine publishes the row at once on the press,
/// still with the old facts, then again as the send starts and once it is over. A press is answered
/// when the facts moved, when a send seen since is over, or when nothing can be sent now.
private final class ProgressPress {
    private let at: [Int64?]
    private var sent = false

    init(_ at: [Int64?]) { self.at = at }

    func answered(by row: BookServerRow?) -> Bool {
        guard let row else { return true }
        if row.progressSending {
            sent = true
            return false
        }
        switch ServerStand(row) {
        case .offline, .off, .none, .saved: return true
        default: return sent || row.stamp != at
        }
    }
}

/// The page between chapters' server line per chapter, and each server row's line.
///
/// The engine says a chapter's result at once when it is known before any send (offline, held
/// for Try again, not set to receive), and that comes before the chapter's ChapterRead; otherwise
/// it comes once the server answers. A chapter read with no result yet starts a wait on the
/// server its work reads from (`hostFor`): Sending shows after 600 ms with no answer, and the
/// answer then waits until Sending has shown 0.3 s. A line that is not Saved follows its server's
/// row from there. With progress sent on closing the reader (`quiet`) the page has no line.
@Observable @MainActor
public final class ServerProgressModel {
    /// The line each chapter read in this reader has, when it has one.
    public private(set) var lines: [ChapterId: ServerLine] = [:]
    /// Each server's row line by its id; a server with nothing to say has none.
    public private(set) var rowLines: [String: ServerRowLine] = [:]
    /// Servers whose row Try again was pressed and is not answered yet: the button is at 38%.
    public private(set) var retrying: Set<String> = []

    @ObservationIgnored var send: ((Command) -> Void)?
    /// The server host a work reads from, when that server takes progress.
    @ObservationIgnored var hostFor: (WorkId) -> String? = { _ in nil }
    /// Progress goes out when the reader closes: the page between chapters has no line.
    @ObservationIgnored var quiet: () -> Bool = { false }

    // The page between chapters.
    @ObservationIgnored private var waits: [ChapterId: WaitingLine] = [:]
    @ObservationIgnored private var servers: [ChapterId: String] = [:]
    @ObservationIgnored private var hosts: [ChapterId: String] = [:]
    @ObservationIgnored private var answered: Set<ChapterId> = []
    @ObservationIgnored private var follows: Set<ChapterId> = []
    @ObservationIgnored private var linePressed: [String: ProgressPress] = [:]

    // The rows.
    @ObservationIgnored private var rows: [String: BookServerRow] = [:]
    @ObservationIgnored private var rowWaits: [String: WaitingLine] = [:]
    @ObservationIgnored private var rowPressed: [String: ProgressPress] = [:]
    @ObservationIgnored private var counts: [String: Int] = [:]

    public init() {}

    /// The engine's events these lines follow.
    func apply(_ event: Event) {
        switch event {
        case .reader(.chapterRead(let work, let chapter, _)):
            chapterRead(work: work, chapter: chapter)
        case .reader(.progressSent(_, let chapter, let server, let host, let result)):
            sent(chapter: chapter, server: server, host: host, result: result)
        case .bookSources(.view(let view)):
            update(view.servers)
        default:
            break
        }
    }

    // MARK: The page between chapters

    /// Whether the page between chapters holds the line's two-line space for `chapter` of
    /// `work`: it has a line, or one may come. Next and Continue never move.
    public func holds(work: WorkId, chapter: ChapterId) -> Bool {
        lines[chapter] != nil || (!quiet() && (waits[chapter] != nil || hostFor(work) != nil))
    }

    /// The read rule fired for `chapter` of `work`.
    func chapterRead(work: WorkId, chapter: ChapterId) {
        if quiet() || answered.contains(chapter) || waits[chapter] != nil { return }
        guard let host = hostFor(work) else { return }
        hosts[chapter] = host
        wait(chapter)
    }

    /// The engine said what became of `chapter`'s send to `server`.
    func sent(chapter: ChapterId, server: String, host: String, result: WriteBackResult) {
        answered.insert(chapter)
        servers[chapter] = server
        hosts[chapter] = host
        let said = quiet() ? nil : ServerLine.of(result, host: host)
        if said == nil || said == .saved(host: host) { follows.remove(chapter) } else { follows.insert(chapter) }
        show(chapter, said)
    }

    /// Try again on the line: every waiting chapter of its server goes out now. The engine answers
    /// a retry on the server's row, so the line follows the row from here.
    public func retry(chapter: ChapterId) {
        guard let server = servers[chapter], lines[chapter]?.canRetry == true, linePressed[server] == nil else { return }
        linePressed[server] = ProgressPress(rows[server]?.stamp ?? [nil, nil])
        for other in follows where servers[other] == server && waits[other] == nil { wait(other) }
        send?(.bookSources(.retryProgress(id: server)))
    }

    private func followRows() {
        linePressed = linePressed.filter { server, press in !press.answered(by: rows[server]) }
        for chapter in follows {
            guard let server = servers[chapter], linePressed[server] == nil, let host = hosts[chapter] else { continue }
            switch ServerStand(rows[server]) {
            case .going: if waits[chapter] == nil { wait(chapter) }
            case .saved: settle(chapter, .saved(host: host))
            case .offline: settle(chapter, .offline(host: host))
            case .retrying: settle(chapter, .retrying(host: host))
            case .failed(_, let why): settle(chapter, .failed(host: host, why: why))
            case .off, .none: settle(chapter, nil)
            }
        }
    }

    private func settle(_ chapter: ChapterId, _ line: ServerLine?) {
        if line == nil || line.map({ if case .saved = $0 { true } else { false } }) == true { follows.remove(chapter) }
        if waits[chapter] == nil, lines[chapter] == line { return }
        show(chapter, line)
    }

    private func wait(_ chapter: ChapterId) {
        waits.removeValue(forKey: chapter)?.cancel()
        waits[chapter] = WaitingLine { [weak self] in
            guard let self, let host = self.hosts[chapter] else { return }
            self.lines[chapter] = .sending(host: host)
        }
    }

    /// `line` on `chapter`'s page, after the waiting line's hold when one showed.
    private func show(_ chapter: ChapterId, _ line: ServerLine?) {
        let put: @MainActor () -> Void = { [weak self] in self?.lines[chapter] = line }
        if let wait = waits.removeValue(forKey: chapter) { wait.end(put) } else { put() }
    }

    /// The reader closed: nothing is waited for any more.
    public func clearLines() {
        waits.values.forEach { $0.cancel() }
        waits = [:]
        servers = [:]
        hosts = [:]
        answered = []
        follows = []
        linePressed = [:]
        lines = [:]
    }

    // MARK: The rows

    /// The book sources' view changed.
    func update(_ now: [BookServerRow]) {
        let ids = Set(now.map(\.id))
        for gone in rows.keys where !ids.contains(gone) { forget(gone) }
        for row in now {
            rows[row.id] = row
            one(row.id)
        }
        followRows()
    }

    private func one(_ id: String) {
        let row = rows[id]
        if let press = rowPressed[id] {
            if !press.answered(by: row) {
                going(id, count: Int(row?.progressPending ?? 0))
                return
            }
            rowPressed[id] = nil
        }
        let stand = ServerStand(row)
        if case .going(let count) = stand { going(id, count: count) } else { settleRow(id, stand.rowLine) }
    }

    /// The queue is going: Sending after 600 ms, its count kept current.
    private func going(_ id: String, count: Int) {
        if count > 0 { counts[id] = count }
        if let wait = rowWaits[id] {
            if wait.shown { rowLines[id] = .sending(count: counts[id] ?? 1) }
        } else {
            rowWaits[id] = WaitingLine { [weak self] in
                guard let self else { return }
                self.rowLines[id] = .sending(count: self.counts[id] ?? 1)
            }
        }
    }

    private func settleRow(_ id: String, _ line: ServerRowLine?) {
        let put: @MainActor () -> Void = { [weak self] in
            guard let self else { return }
            self.retrying.remove(id)
            self.rowLines[id] = line
        }
        if let wait = rowWaits.removeValue(forKey: id) { wait.end(put) } else { put() }
    }

    /// Try again on a server's row.
    public func retryRow(_ id: String) {
        guard rowPressed[id] == nil, rowLines[id]?.canRetry == true else { return }
        rowPressed[id] = ProgressPress(rows[id]?.stamp ?? [nil, nil])
        retrying.insert(id)
        send?(.bookSources(.retryProgress(id: id)))
        one(id)
    }

    private func forget(_ id: String) {
        rowWaits.removeValue(forKey: id)?.cancel()
        rows[id] = nil
        rowPressed[id] = nil
        counts[id] = nil
        retrying.remove(id)
        rowLines[id] = nil
    }

    // MARK: Shots

    /// For shots: `chapter`'s line as it would stand, from `server`.
    public func stage(chapter: ChapterId, line: ServerLine?, server: String = "") {
        servers[chapter] = server
        answered.insert(chapter)
        if let line { hosts[chapter] = line.host }
        lines[chapter] = line
    }

    /// For shots: `id`'s row line as it would stand, its Try again pressed when `pressing`.
    public func stage(row id: String, line: ServerRowLine?, pressing: Bool = false) {
        rowLines[id] = line
        if pressing { retrying.insert(id) } else { retrying.remove(id) }
    }
}

extension AppStore {
    /// The host of the book server `work` reads from, when that server takes progress.
    func progressHost(_ work: WorkId) -> String? {
        guard let source = reading.chapters[work]?.edition?.sourceId,
              let server = bookSources.view?.servers.first(where: { $0.id == source && $0.reportProgress && !$0.off })
        else { return nil }
        return server.progressHost
    }
}
