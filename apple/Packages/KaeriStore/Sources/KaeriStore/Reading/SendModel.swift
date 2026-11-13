// Send to e-reader and the OPDS feed as the engine tells them. A send is asked for from a sheet,
// told as progress, may wait for Wi-Fi (the engine says why and keeps it across a restart), and
// ends with files for the share sheet. The feed is the Mac's catalogue for e-readers at home.
// Nothing here decides anything: the formats a target takes, the words of a failure and the line
// of a held send all come from the engine.

import KaeriKit
import KaeriStrings
import Foundation
import Observation

/// One send, from the moment the sheet asks for it until the files are shared or it is dropped.
public struct EReaderSend: Identifiable, Equatable, Sendable {
    public enum Phase: Equatable, Sendable {
        /// Under way. `fraction` is how far, when the engine has said; `done` of `total` chapters
        /// are gathered (zero `total` until the engine has said).
        case preparing(fraction: Double?, done: Int, total: Int)
        /// Held for Wi-Fi, in the engine's words. The next progress says it is going again.
        case waiting(line: String)
        case ready([ExportFile])
        case failed(String)
        /// Ready, but the files were cleared before they were shared: the engine keeps them for a
        /// day. Send again makes them anew.
        case gone
    }

    /// Ours, from 1, so a send is the same row before the engine has named its job.
    public let id: Int
    public internal(set) var job: JobId?
    /// Empty for a send the engine started again after a restart, until it says which.
    public internal(set) var work: String
    public internal(set) var target: String
    /// The chapters asked for, in reading order; empty for a restarted send.
    public let chapters: [String]
    public internal(set) var phase: Phase
    /// The format asked for, so a send whose files are gone can be asked for again; nil for a
    /// restarted send.
    var format: ExportFormat?
    /// Stopped before the engine named its job: stopped as soon as it does.
    var cancelOnBind = false

    /// Which phase it is in, whatever it carries: for animating between them.
    public var kind: Int {
        switch phase {
        case .preparing: 0
        case .waiting: 1
        case .ready: 2
        case .failed: 3
        case .gone: 4
        }
    }

    public var isWaiting: Bool {
        if case .waiting = phase { return true }
        return false
    }

    public var isRunning: Bool {
        switch phase {
        case .preparing, .waiting: true
        case .ready, .failed, .gone: false
        }
    }

    /// Whether Send again can ask for it once more: the engine said what it is.
    public var canSendAgain: Bool { !work.isEmpty && !chapters.isEmpty && format != nil }
}

@Observable @MainActor
public final class SendModel {
    /// The e-readers the engine knows, once it has said.
    public private(set) var targets: [ExportTarget] = []
    public private(set) var sends: [EReaderSend] = []
    /// The Mac's feed, as last told.
    public private(set) var feed: FeedView?
    /// The e-reader the sheet offers first: the engine keeps it, on this device only.
    public private(set) var chosenTarget: String?
    @ObservationIgnored var command: ((Command) -> Void)?
    @ObservationIgnored private var lastId = 0
    /// A shot's feed, told by hand: the engine's own view no longer replaces it.
    @ObservationIgnored private var feedStaged = false

    public init() {}

    // MARK: Targets

    public func loadTargets() { command?(.reading(.loadExportTargets)) }

    /// The target the sheet starts on: the one chosen here, else the engine's first.
    public var defaultTarget: ExportTarget? {
        targets.first { $0.id == chosenTarget } ?? targets.first
    }

    public func choose(target id: String) {
        guard chosenTarget != id else { return }
        chosenTarget = id
        command?(.reading(.setSendTarget(target: id)))
    }

    /// The format a target is sent in for pages or text: the first it takes, as the engine lists
    /// them. Nil when it takes none of them.
    public static func format(of target: ExportTarget, novel: Bool) -> ExportFormat? {
        (novel ? target.novelFormats : target.mangaFormats).first
    }

    // MARK: Sending

    /// Asks for files of `chapters` for `target`. The send is the one returned, so a sheet can
    /// follow it.
    @discardableResult
    public func begin(work: String, chapters: [String], target: ExportTarget, novel: Bool) -> Int? {
        guard let format = Self.format(of: target, novel: novel) else { return nil }
        lastId += 1
        var send = EReaderSend(
            id: lastId, job: nil, work: work, target: target.id, chapters: chapters,
            phase: .preparing(fraction: nil, done: 0, total: 0))
        send.format = format
        sends.append(send)
        choose(target: target.id)
        command?(.reading(.prepare(work: work, pick: .chapters(ids: chapters), target: target.id, format: format)))
        return lastId
    }

    /// Stops a send that is under way, or drops one that has ended. A held send is cancelled in
    /// the engine too, which forgets it for the next launch.
    public func cancel(_ id: Int) {
        guard let index = sends.firstIndex(where: { $0.id == id }) else { return }
        let send = sends[index]
        if send.isRunning {
            if let job = send.job {
                command?(.reading(.cancelPrepare(job: job)))
            } else {
                sends[index].cancelOnBind = true
                return
            }
        }
        sends.remove(at: index)
    }

    /// The files of a ready send, to hand the share sheet, when every one is still there. The
    /// engine clears them a day after they were made: then the send says they are gone, and
    /// nothing is shared or counted as sent.
    public func shareable(_ id: Int, exists: (String) -> Bool = { FileManager.default.fileExists(atPath: $0) }) -> [URL]? {
        guard let index = sends.firstIndex(where: { $0.id == id }),
              case .ready(let files) = sends[index].phase else { return nil }
        guard files.allSatisfy({ exists($0.path) }) else {
            sends[index].phase = .gone
            return nil
        }
        return files.map { URL(fileURLWithPath: $0.path) }
    }

    /// Asks again for the files of a send whose files are gone. The send is the one returned.
    @discardableResult
    public func sendAgain(_ id: Int) -> Int? {
        guard let send = sends.first(where: { $0.id == id }), send.canSendAgain, let format = send.format else { return nil }
        sends.removeAll { $0.id == id }
        lastId += 1
        var again = EReaderSend(
            id: lastId, job: nil, work: send.work, target: send.target, chapters: send.chapters,
            phase: .preparing(fraction: nil, done: 0, total: 0))
        again.format = format
        sends.append(again)
        command?(.reading(.prepare(work: send.work, pick: .chapters(ids: send.chapters), target: send.target, format: format)))
        return lastId
    }

    /// The files went through the share sheet: the engine keeps that the chapters were sent, and
    /// the series page asks where you stopped.
    public func shared(_ id: Int) {
        guard let send = sends.first(where: { $0.id == id }) else { return }
        if case .gone = send.phase { return }
        if !send.work.isEmpty {
            command?(.reading(.shared(work: send.work, chapters: send.chapters, target: send.target)))
        }
        sends.removeAll { $0.id == id }
    }

    public func dismiss(_ id: Int) {
        sends.removeAll { $0.id == id }
    }

    public func send(_ id: Int) -> EReaderSend? { sends.first { $0.id == id } }

    /// The send a series has under way, for its page.
    public func running(work: String) -> EReaderSend? {
        sends.first { $0.work == work && $0.isRunning }
    }

    // MARK: The answer to where you stopped

    public func stoppedAt(work: String, chapter: String?) {
        command?(.reading(.stoppedAt(work: work, chapter: chapter, page: nil)))
    }

    // MARK: The feed

    public func loadFeed() { command?(.reading(.loadFeed)) }

    public func setFeed(on: Bool, password: String? = nil) {
        command?(.reading(.setFeed(on: on, password: password)))
    }

    public func setShare(_ shelf: FeedShelf, on: Bool) {
        command?(.reading(.setFeedShare(shelf: shelf, on: on)))
    }

    /// A password for e-readers: eight digits, which a person can type on a device with no
    /// keyboard.
    public static func newPassword() -> String {
        (0..<8).map { _ in String(Int.random(in: 0...9)) }.joined()
    }

    // MARK: Hearing

    func apply(_ event: ReadingEvent) {
        switch event {
        case .exportTargets(let list):
            targets = list
            if let kept = list.first(where: \.chosen) { chosenTarget = kept.id }
        case .exportProgress(let job, let done, let total, let chaptersDone, let chapters):
            let fraction = total > 0 ? min(1, max(0, Double(done) / Double(total))) : nil
            update(job: job) { $0.phase = .preparing(fraction: fraction, done: Int(chaptersDone), total: Int(chapters)) }
        case .exportFailed(let job, let chapter, let failure):
            let words = chapter.map { L.sendFailedChapter(chapter: $0, cause: failure.sendWords) } ?? L.sendFailed(cause: failure.sendWords)
            update(job: job) { $0.phase = .failed(words) }
        case .exportWaiting(let job, let work, let target, let line, _):
            update(job: job, work: work, target: target) { $0.phase = .waiting(line: line.text) }
        case .exportReady(let job, let files):
            update(job: job) { $0.phase = .ready(files) }
        case .feed(let view):
            if !feedStaged { feed = view }
        case .failed(.export, let failure):
            // A send the engine could not even start names no job: it is the one that was
            // waiting for an answer, else the latest still going.
            let index = sends.firstIndex { $0.job == nil && $0.isRunning }
                ?? sends.lastIndex { $0.isRunning }
            if let index { sends[index].phase = .failed(failure.sendWords) }
        default:
            break
        }
    }

    /// The send a job belongs to: the one already bound to it, else the first one asked for
    /// that the engine had not named yet, else one the app never started (a send held across a
    /// restart), which shows all the same.
    private func update(job: JobId, work: String? = nil, target: String? = nil, _ change: (inout EReaderSend) -> Void) {
        let index: Int
        if let found = sends.firstIndex(where: { $0.job == job }) {
            index = found
        } else if let unbound = sends.firstIndex(where: { $0.job == nil && $0.isRunning }) {
            sends[unbound].job = job
            index = unbound
        } else {
            lastId += 1
            sends.append(EReaderSend(
                id: lastId, job: job, work: work ?? "", target: target ?? "", chapters: [],
                phase: .preparing(fraction: nil, done: 0, total: 0)))
            index = sends.count - 1
        }
        if let work, sends[index].work.isEmpty { sends[index].work = work }
        if let target, sends[index].target.isEmpty { sends[index].target = target }
        if sends[index].cancelOnBind, let bound = sends[index].job {
            command?(.reading(.cancelPrepare(job: bound)))
            sends.remove(at: index)
            return
        }
        change(&sends[index])
    }

    #if FIXTURES
    /// For shots: an event as the engine would send it.
    public func showcase(_ event: ReadingEvent) { apply(event) }
    #endif

    /// A shot's send in a state, never asked of the engine: -KaeriSend preparing, waiting,
    /// failed or ready. Preparing is two of three chapters done.
    @discardableResult
    public func stage(_ state: String, work: String, chapters: [String], target: String) -> Int {
        #if FIXTURES
        lastId += 1
        let phase: EReaderSend.Phase = switch state {
        case "waiting": .waiting(line: "\(max(1, chapters.count - 1)) chapters aren't downloaded yet, so this send waits for Wi-Fi and goes on by itself.")
        case "failed": .failed(L.sendFailedChapter(chapter: chapters.last ?? "1", cause: ReadingFailure.offline.sendWords))
        case "ready": .ready([])
        default: .preparing(fraction: 0.62, done: 2, total: min(3, max(chapters.count, 1)))
        }
        sends.append(EReaderSend(id: lastId, job: nil, work: work, target: target, chapters: chapters, phase: phase))
        return lastId
        #else
        return 0
        #endif
    }

    /// A shot's send that went through the share sheet, so the series asks where you stopped.
    public func stageShared(work: String, chapters: [String], target: String) {
        #if FIXTURES
        command?(.reading(.shared(work: work, chapters: chapters, target: target)))
        #endif
    }

    /// A shot of the feed pane with the feed on, told as the engine would, never switched on:
    /// -KaeriFeed on. True when it was staged, and the pane then leaves the view alone.
    public func stageFeed(_ arguments: [String] = ProcessInfo.processInfo.arguments) -> Bool {
        #if FIXTURES
        guard let index = arguments.firstIndex(of: "-KaeriFeed"), arguments.indices.contains(index + 1) else { return false }
        feedStaged = true
        let paused = arguments[index + 1] == "paused"
        let on = arguments[index + 1] == "on" || paused
        let now = Int64(Date().timeIntervalSince1970)
        feed = FeedView(
            on: on, addresses: on && !paused ? ["http://192.168.1.24:8484"] : [], port: 8484, passwordSet: true,
            serving: 126, password: "48219930",
            shares: [
                FeedShare(shelf: .downloads, on: true, count: 126),
                FeedShare(shelf: .mangaFolders, on: true, count: 14),
                FeedShare(shelf: .novelFolders, on: false, count: 5),
            ],
            clients: on ? [
                FeedClient(name: "Kobo Libra", address: "192.168.1.31", last: now - 240, files: 3),
                FeedClient(name: "", address: "192.168.1.40", last: now - 3 * 86_400, files: 12),
            ] : [],
            pause: paused ? .vpnOnly : nil)
        return true
        #else
        return false
        #endif
    }

    public static func launchState(_ arguments: [String] = ProcessInfo.processInfo.arguments) -> String? {
        #if FIXTURES
        guard let index = arguments.firstIndex(of: "-KaeriSend"), arguments.indices.contains(index + 1) else { return nil }
        return arguments[index + 1]
        #else
        return nil
        #endif
    }
}
