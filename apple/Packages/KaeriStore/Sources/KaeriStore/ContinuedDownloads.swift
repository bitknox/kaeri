// Downloads that go on after the app leaves: iOS 26's continued-processing task, which the system
// runs with its own progress line while the person is elsewhere. Episodes and chapters share one
// window, which counts them together. The window is asked for when a download starts, or as the
// app leaves with some queued; while it is open the engine lets the
// downloads run in the background, with the connection check as on screen; and it closes when
// nothing is left, when it runs out, or when the person stops it, and the torrents wait again.
// The Mac has no such window and never asks.

import KaeriKit
import KaeriStrings
import Foundation
import Observation

/// The window the system opened, as the app reports to it.
@MainActor
public protocol DownloadWindow: AnyObject {
    /// How far the downloads are: `completed` of `total` units, what downloads, and a line saying
    /// what is left.
    func report(completed: Int64, total: Int64, title: String, subtitle: String)
    /// Close the window: `success` when everything finished, not when it ran out or was stopped.
    func end(success: Bool)
}

/// Whether downloads go on in the background, and what that tells the engine.
@Observable @MainActor
public final class ContinuedDownloads {
    public enum Stage: Equatable, Sendable {
        /// No window.
        case idle
        /// Asked for, not opened yet.
        case asked
        /// Open: the downloads go on, and the system shows its own progress for them.
        case running
    }

    public private(set) var stage: Stage = .idle

    /// Whether the system's own progress for the downloads is showing, or about to: the app's
    /// own Live Activity stays out of the way meanwhile.
    public var isRunning: Bool { stage != .idle }

    /// Ask the system for a window, titled `title` with `subtitle`: false when it refused. `nil`
    /// where there is no such window, as on the Mac: nothing is asked.
    @ObservationIgnored public var ask: (@MainActor (_ title: String, _ subtitle: String) -> Bool)?
    /// Tell the engine where the app is: the store's own sending, unless the app hands it to
    /// `SystemPhases`, which then tells the engine once for the scene and the window both.
    @ObservationIgnored public var send: (@MainActor (Command) -> Void)?
    /// `isRunning` changed: the store keeps the downloads' Live Activity out of the way with it.
    @ObservationIgnored var runningChanged: (@MainActor (Bool) -> Void)?
    /// Whether the episode on is playing, for the phase the app goes to when it leaves.
    @ObservationIgnored public var playing: @MainActor () -> Bool = { false }

    @ObservationIgnored private var window: (any DownloadWindow)?
    @ObservationIgnored private(set) var inForeground = true
    /// What the window has counted since it opened, by download: how far each is, nought to one.
    @ObservationIgnored private var counted: [Item.Key: Double] = [:]
    /// The episodes and the chapters as the engine last said them.
    @ObservationIgnored private var episodes: [DownloadRow] = []
    @ObservationIgnored private var chapters: [BookDownloadRow] = []
    /// How many downloads waited at the last count, to tell a download started from one ended.
    @ObservationIgnored private var lastPending = 0

    public init() {}

    /// One download the window counts: an episode or a chapter.
    struct Item {
        enum Key: Hashable {
            case episode(Int64)
            case chapter(WorkId, ChapterId)
        }

        let key: Key
        /// Waits or runs, so the window has it to go on with.
        let pending: Bool
        /// How far it is, nought to one; `nil` keeps what was counted, as for a chapter that
        /// failed, which says no pages.
        let fraction: Double?

        init(_ row: DownloadRow) {
            key = .episode(row.id)
            pending = row.state == .queued || row.state == .active
            fraction = row.state == .done ? 1 : min(max(row.fraction, 0), 1)
        }

        /// A chapter waiting for Wi-Fi is not one to go on with: the window would only wait.
        init(_ row: BookDownloadRow) {
            key = .chapter(row.work, row.chapter.id)
            switch row.state {
            case .queued:
                pending = true
                fraction = 0
            case .downloading(let done, let total):
                pending = true
                // A novel's chapter has no pages to count: it counts once done.
                fraction = total > 0 ? min(Double(done) / Double(total), 1) : 0
            case .done:
                pending = false
                fraction = 1
            case .waitsForWifi, .failed:
                pending = false
                fraction = nil
            }
        }
    }

    /// The episodes, then the chapters.
    private var items: [Item] {
        episodes.map(Item.init) + chapters.map(Item.init)
    }

    /// What the window says downloads: episodes, chapters or both, by what waits or runs.
    static func title(episodes: Int, chapters: Int) -> String {
        switch (episodes > 0, chapters > 0) {
        case (true, true): L.downloadsContinuedBothTitle
        case (false, true): L.downloadsContinuedChaptersTitle
        default: L.downloadsContinuedTitle
        }
    }

    /// The line under it: how many of each are left.
    static func left(episodes: Int, chapters: Int) -> String {
        switch (episodes > 0, chapters > 0) {
        case (true, true):
            [L.downloadsContinuedLeft(count: episodes), L.downloadsContinuedChaptersLeft(count: chapters)]
                .joined(separator: " · ")
        case (false, true): L.downloadsContinuedChaptersLeft(count: chapters)
        default: L.downloadsContinuedLeft(count: episodes)
        }
    }

    /// The title and the line for what waits or runs now.
    private func words() -> (title: String, left: String) {
        let episodes = episodes.filter { Item($0).pending }.count
        let chapters = chapters.filter { Item($0).pending }.count
        return (Self.title(episodes: episodes, chapters: chapters), Self.left(episodes: episodes, chapters: chapters))
    }

    /// Ask for a window when there are downloads to go on and none is asked or open.
    private func want() {
        guard stage == .idle, items.contains(where: \.pending), let ask else { return }
        let words = words()
        if ask(words.title, words.left) {
            move(to: .asked)
        }
    }

    /// The episodes as the engine said them: a download started asks for a window, and an open
    /// one reports how far they are and closes once nothing is left.
    func update(_ rows: [DownloadRow]) {
        episodes = rows
        changed()
    }

    /// The chapters as the engine said them, which go on in the same window as the episodes.
    func update(chapters rows: [BookDownloadRow]) {
        chapters = rows
        changed()
    }

    private func changed() {
        let items = items
        let pending = items.filter(\.pending)
        defer { lastPending = pending.count }
        if stage == .idle, inForeground, pending.count > lastPending {
            want()
        }
        guard stage == .running, let window else { return }
        for item in items where item.pending || counted[item.key] != nil {
            if let fraction = item.fraction { counted[item.key] = fraction }
        }
        if pending.isEmpty {
            finish(success: true)
            return
        }
        let total = Int64(counted.count) * 1_000
        let completed = Int64((counted.values.reduce(0, +) * 1_000).rounded())
        let words = words()
        window.report(completed: min(completed, total), total: total, title: words.title, subtitle: words.left)
    }

    /// The system opened the window it was asked for. `rows` are the episodes; the chapters are
    /// the ones last said.
    public func began(_ window: any DownloadWindow, rows: [DownloadRow]) {
        episodes = rows
        guard items.contains(where: \.pending) else {
            window.end(success: true)
            move(to: .idle)
            return
        }
        self.window = window
        move(to: .running)
        counted = [:]
        if !inForeground { send?(.appPhase(phase: .backgroundDownloads)) }
        changed()
    }

    /// The system ended the window: it ran out, or the person stopped it. The downloads wait
    /// where they got to.
    public func expired() {
        finish(success: false)
    }

    /// The system would not open the window it was asked for.
    public func refused() {
        if stage == .asked { move(to: .idle) }
    }

    /// The app came forward, or left: the phase the engine is told, and a window asked for as it
    /// leaves with downloads queued. `rows` are the episodes; the chapters are the ones last said.
    public func phase(foreground: Bool, rows: [DownloadRow]) {
        inForeground = foreground
        episodes = rows
        if foreground {
            send?(.appPhase(phase: .foreground))
        } else {
            want()
            send?(.appPhase(phase: stage == .running ? .backgroundDownloads : .background(playing: playing())))
        }
    }

    /// The app is about to leave: still in front, so the system can open a window for it.
    /// `rows` are the episodes; the chapters are the ones last said.
    public func leaving(rows: [DownloadRow]) {
        episodes = rows
        want()
    }

    /// Whether any episode or chapter would go on after the app leaves, for the note that says so.
    public static func goesOn(_ rows: [DownloadRow], chapters: [BookDownloadRow]) -> Bool {
        rows.contains { Item($0).pending } || chapters.contains { Item($0).pending }
    }

    private func move(to next: Stage) {
        let wasRunning = isRunning
        stage = next
        if isRunning != wasRunning { runningChanged?(isRunning) }
    }

    private func finish(success: Bool) {
        window?.end(success: success)
        window = nil
        counted = [:]
        let was = stage
        move(to: .idle)
        if was == .running, !inForeground {
            send?(.appPhase(phase: .background(playing: playing())))
        }
    }
}
