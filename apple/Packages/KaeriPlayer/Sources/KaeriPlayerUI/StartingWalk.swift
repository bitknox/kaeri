// What a start says while it runs: the engine's steps kept as the starting screen's step line,
// detail line, Details rows and the actions that join the row by time and state.

import KaeriKit
import KaeriStore
import KaeriStrings
import Foundation

/// The buttons of the starting screen's row, in the order they arrive.
public enum StartAction: Hashable, Sendable {
    case cancel
    case playFromBeginning
    case chooseSource
    case details
    case skip(String)
    case showDownloads
}

/// One row of the Details list.
public struct StartRow: Equatable, Identifiable, Sendable {
    public enum Kind: Equatable, Sendable { case done, missed, now, resting, fact }
    public var id: Int
    public var kind: Kind
    public var title: String
    public var began: ContinuousClock.Instant
    /// How long it took, once it is over.
    public var took: Duration?
}

/// The start of one play, as the engine has told it. A value: the model feeds it steps and the
/// clock, and the screen reads lines, facts and actions from it.
public struct StartingWalk: Equatable, Sendable {
    /// Each step text stays this long at least before the next replaces it.
    public static let hold: Duration = .milliseconds(600)
    /// The detail line, and Play from the beginning, from here.
    public static let detailAfter: Duration = .seconds(1)
    /// Choose source and Details, and the seconds on the detail line, from here.
    public static let toolsAfter: Duration = .seconds(3)
    /// Skip, on one source or stream, from here.
    public static let skipAfter: Duration = .seconds(8)
    /// "Still waiting for brook", from here.
    public static let slowAfter: Duration = .seconds(20)

    public let id: AnilistId
    public let episode: String
    public let began: ContinuousClock.Instant

    /// The step line as shown, and when it took its place.
    public private(set) var line: String
    public private(set) var lineSince: ContinuousClock.Instant
    /// The newest step line that arrived during the hold, shown when the hold is over.
    public private(set) var queued: String?

    struct Asked: Equatable, Sendable {
        var name: String
        var place: Int
        var total: Int
        var pinned: Bool
    }

    struct Missed: Equatable, Sendable {
        var name: String
        var miss: SourceMiss
    }

    enum Phase: Equatable, Sendable {
        case lookup, asking, peers(String?), opening, buffering
    }

    var phase = Phase.lookup
    var asked: Asked?
    var missed: Missed?
    var resting: [String] = []
    var from: String?
    var attempt = 0
    var streams = 0
    var noVideo = false
    var peers: Int?
    var bytesPerSecond: UInt64?
    var percent: Int?
    /// The translation the streams found play in.
    public private(set) var translation: Translation?
    /// When the source or stream being tried was first tried.
    var rungSince: ContinuousClock.Instant
    public private(set) var rows: [StartRow] = []
    private var nextRow = 0

    /// Where the episode was left, in seconds, when it resumes there.
    public var resume: Double?
    /// The viewer chose to play from the beginning: the resume line says so instead.
    public var fromBeginning = false

    public init(id: AnilistId, episode: String, resume: Double?, at now: ContinuousClock.Instant) {
        self.id = id
        self.episode = episode
        self.resume = resume
        began = now
        line = Msg.startLookingUpTitle.text
        lineSince = now
        rungSince = now
        open(Msg.startLookingUpTitle.text, at: now)
    }

    // MARK: Taking the engine's steps

    /// A step of the start. `line` is the step line it brings, when it changes it.
    public mutating func take(_ step: StartStep, line text: Msg?, at now: ContinuousClock.Instant) {
        var next = text?.text
        switch step {
        case .lookingUpTitle:
            phase = .lookup
        case .gettingSourcesReady:
            phase = .lookup
        case .askingSource(let name, let place, let total, let pinned):
            closeLookup(at: now)
            phase = .asking
            asked = Asked(name: name, place: Int(place), total: Int(total), pinned: pinned)
            rungSince = now
            open(text?.text ?? L.startAskingSource(name: name), at: now)
        case .sourceMissed(let name, let miss):
            missed = Missed(name: name, miss: miss)
            finish(.missed, title: Self.missLine(name: name, miss: miss, episode: episode), at: now)
        case .sourcesResting(let names):
            resting = names
            add(.resting, title: L.playerStartResting.prefixed(by: names.joined(separator: ", ")), at: now)
        case .findingPeers(let name):
            closeLookup(at: now)
            phase = .peers(name)
            rungSince = now
        case .found(let count, let source, _, let translation):
            closeLookup(at: now)
            streams = Int(count)
            from = source
            self.translation = translation
            phase = .opening
            attempt = 1
            rungSince = now
            finish(.done, title: source.map { L.playerStartFrom(name: $0) } ?? Msg.startOpeningStream.text, at: now)
        case .fetchingSubtitles:
            break
        case .openingStream(let attempt, let total):
            phase = .opening
            if Int(attempt) != self.attempt { rungSince = now }
            self.attempt = Int(attempt)
            streams = Int(total)
        case .tryingAnotherStream(let nextStream, let total):
            noVideo = true
            phase = .opening
            attempt = Int(nextStream)
            streams = Int(total)
            rungSince = now
            percent = nil
        case .swarm(let peers, let speed):
            self.peers = Int(peers)
            bytesPerSecond = speed
            if phase == .opening { phase = .buffering }
        case .speed(let speed):
            bytesPerSecond = speed
            if phase == .opening { phase = .buffering }
        case .buffering(let percent):
            self.percent = Int(percent)
            phase = .buffering
            next = Self.bufferingLine(percent: Int(percent))
        }
        if let next { say(next, at: now) }
    }

    /// The viewer switched the translation: the streams found are for the other one.
    public mutating func choseTranslation(_ translation: Translation) { self.translation = translation }

    /// mpv loaded the file and has not shown a frame: waiting for data, with no number to give.
    public mutating func loaded(at now: ContinuousClock.Instant) {
        guard phase == .opening, percent == nil else { return }
        phase = .buffering
        say(Msg.startBuffering.text, at: now)
    }

    /// Show `text` as the step line: at once when the last text has held long enough, else
    /// after the hold, in place of whatever else was waiting.
    private mutating func say(_ text: String, at now: ContinuousClock.Instant) {
        if text == line {
            queued = nil
            return
        }
        if now - lineSince >= Self.hold {
            line = text
            lineSince = now
            queued = nil
        } else {
            queued = text
        }
    }

    /// When the hold on the shown text ends and a queued one takes its place, if one waits.
    public var releaseAt: ContinuousClock.Instant? { queued == nil ? nil : lineSince + Self.hold }

    /// Take the queued text if its turn has come.
    @discardableResult
    public mutating func release(at now: ContinuousClock.Instant) -> Bool {
        guard let queued, now - lineSince >= Self.hold else { return false }
        line = queued
        lineSince = now
        self.queued = nil
        return true
    }

    // MARK: Details rows

    private mutating func open(_ title: String, at now: ContinuousClock.Instant) {
        // A row still under way when the next opens was passed over.
        if let i = rows.lastIndex(where: { $0.kind == .now }) {
            rows[i].kind = .done
            rows[i].took = now - rows[i].began
        }
        rows.append(StartRow(id: nextRow, kind: .now, title: title, began: now))
        nextRow += 1
    }

    private mutating func add(_ kind: StartRow.Kind, title: String, at now: ContinuousClock.Instant) {
        rows.append(StartRow(id: nextRow, kind: kind, title: title, began: now, took: nil))
        nextRow += 1
    }

    private mutating func finish(_ kind: StartRow.Kind, title: String, at now: ContinuousClock.Instant) {
        guard let i = rows.lastIndex(where: { $0.kind == .now }) else {
            add(kind, title: title, at: now)
            return
        }
        rows[i].kind = kind
        rows[i].title = title
        rows[i].took = now - rows[i].began
    }

    /// The title lookup is over: its row says so.
    private mutating func closeLookup(at now: ContinuousClock.Instant) {
        guard rows.first?.kind == .now, phase == .lookup else { return }
        rows[0].kind = .done
        rows[0].title = L.appleStartLookedUp
        rows[0].took = now - rows[0].began
    }

    /// Details rows to show: the steps so far, the one under way last.
    public var detailRows: [StartRow] { rows }

    // MARK: What the screen reads

    /// The step line: the engine's, and past 20 s on a source asked, said plainly.
    public func stepLine(at now: ContinuousClock.Instant) -> String {
        if phase == .asking, let asked, now - began >= Self.slowAfter {
            return L.playerStartStillWaiting(name: asked.name)
        }
        return shown(at: now)
    }

    /// The step text as it stands at `now`: a text that waited out its hold is the one shown,
    /// before the model has had its turn to take it.
    func shown(at now: ContinuousClock.Instant) -> String {
        if let queued, now - lineSince >= Self.hold { return queued }
        return line
    }

    /// The detail line: facts joined by a middle dot, from 1 s, and the seconds from 3 s.
    public func detail(at now: ContinuousClock.Instant) -> String {
        let age = now - began
        guard age >= Self.detailAfter else { return "" }
        var facts: [String] = []
        if fromBeginning {
            facts.append(L.appleStartBeginning)
        } else if let resume {
            facts.append(L.playerStartResuming(clock: PlayerClock.label(resume)))
        }
        switch phase {
        case .lookup:
            break
        case .asking:
            guard let asked else { break }
            if asked.pinned { facts.append(L.playerStartPinned) }
            if let missed { facts.append(Self.missLine(name: missed.name, miss: missed.miss, episode: episode)) }
            if asked.total > 1 { facts.append(L.playerStartPlace(place: asked.place, total: asked.total)) }
        case .peers(let name):
            if let name { facts.append(L.playerStartFrom(name: name)) }
        case .opening, .buffering:
            if noVideo { facts.append(L.playerStartNoVideo) }
            if let peers {
                facts.append(L.playerStartPeers(count: peers))
                if let bytesPerSecond, bytesPerSecond > 0 { facts.append(L.playerStartSpeed(speed: Self.rate(bytesPerSecond))) }
            } else if let from, !noVideo {
                facts.append(L.playerStartFrom(name: from))
            }
            if noVideo, streams > 1 { facts.append(L.playerStartStreamPlace(next: attempt, total: streams)) }
        }
        if age >= Self.toolsAfter { facts.append(L.appleStartSeconds(seconds: Int(age.components.seconds))) }
        return facts.joined(separator: " · ")
    }

    /// The determinate fill, 0 to 1, when mpv gave a buffering percentage.
    public var progress: Double? { percent.map { Double($0) / 100 } }

    /// Whether the start can be skipped from where it is, and what the button says.
    func skipLabel(at now: ContinuousClock.Instant) -> String? {
        guard now - rungSince >= Self.skipAfter, let asked else { return nil }
        switch phase {
        case .lookup: return nil
        case .asking, .peers: return asked.place < asked.total ? L.playerStartSkipSource(name: asked.name) : nil
        case .opening, .buffering:
            if attempt < streams { return L.playerStartSkipStream }
            return asked.place < asked.total ? L.playerStartSkipSource(name: asked.name) : nil
        }
    }

    /// The row's buttons at `now`, in the order they arrived.
    public func actions(at now: ContinuousClock.Instant) -> [StartAction] {
        let age = now - began
        var actions: [StartAction] = [.cancel]
        if resume != nil, !fromBeginning, age >= Self.detailAfter { actions.append(.playFromBeginning) }
        if age >= Self.toolsAfter { actions += [.chooseSource, .details] }
        if let label = skipLabel(at: now) { actions.append(.skip(label)) }
        return actions
    }

    /// Whether the wait has run long enough to say "Still starting" once.
    public func isSlow(at now: ContinuousClock.Instant) -> Bool { now - began >= Self.skipAfter }

    /// The Details row for the stream's speed, when a direct stream gave one.
    public var speedFact: String? {
        guard peers == nil, let bytesPerSecond, bytesPerSecond > 0 else { return nil }
        return L.playerStartSpeed(speed: Self.rate(bytesPerSecond))
    }

    // MARK: Words

    static func bufferingLine(percent: Int) -> String {
        L.playerStartBufferingPercent(percent: (Double(percent) / 100).formatted(.percent.precision(.fractionLength(0))))
    }

    static func rate(_ bytes: UInt64) -> String {
        Int64(clamping: bytes).formatted(.byteCount(style: .file))
    }

    /// "alpha doesn't have episode 8", and the other ways a source comes up empty.
    static func missLine(name: String, miss: SourceMiss, episode: String) -> String {
        switch miss {
        case .noMatch: L.playerStartMissNoMatch(name: name)
        case .noEpisode: L.playerStartMissNoEpisode(name: name, episode: episode)
        case .failed: L.playerStartMissFailed(name: name)
        case .timedOut: L.playerStartMissTimedOut(name: name)
        case .skipped: L.playerStartMissSkipped(name: name)
        }
    }
}

private extension String {
    /// "name · text", or the text alone when there is no name.
    func prefixed(by name: String) -> String { name.isEmpty ? self : "\(name) · \(self)" }
}

/// How a failed start reads: the reason in plain words, and which action leads.
public struct StartFailureView: Equatable, Sendable {
    public enum Lead: Equatable, Sendable { case chooseSource, tryAgain, fixMatch, addStore, close }
    public var reason: String
    public var lead: Lead

    /// The first clause of the engine's message: the cause without what each source said.
    public static func reason(of text: String) -> String {
        var clause = text
        if let colon = clause.range(of: ": ") { clause = String(clause[..<colon.lowerBound]) + "." }
        if let stop = clause.range(of: ". ") { clause = String(clause[..<stop.lowerBound]) + "." }
        return clause
    }

    public init(why: NoSourceWhy, text: Msg) {
        reason = Self.reason(of: text.text)
        switch why {
        case .notAired: lead = .close
        case .noSources: lead = .addStore
        case .notFound:
            switch text {
            case .noticeReason(.titleNoMatch): lead = .fixMatch
            case .noticeReason(.sourcesTimedOut): lead = .tryAgain
            default: lead = .chooseSource
            }
        }
    }

    /// The old failure that carries no cause.
    public init() {
        reason = L.playerFailedDetail
        lead = .chooseSource
    }
}
