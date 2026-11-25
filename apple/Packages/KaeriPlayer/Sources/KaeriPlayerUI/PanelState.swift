// What the player's Episodes and Sources panel draws from besides the engine's menus: the
// episode's releases across every source, the switch to one of them as the engine says it goes,
// and the picture held over the video while the switch loads (player panel board).

import CoreGraphics
import KaeriKit
import KaeriStrings

/// What a switch of the playing episode goes to, by the ids the panel's rows carry.
public struct SwitchTarget: Equatable, Sendable {
    /// The source, as `SourceCandidate.providerId` names it.
    public var providerId: String
    /// The name the source goes by.
    public var name: String
    /// The release, by its `SourceCandidate.id`; nil for a stream of the source playing.
    public var release: String?
    /// The stream, by its `MenuSource.id`; nil for a release.
    public var stream: String?

    public init(providerId: String, name: String, release: String? = nil, stream: String? = nil) {
        self.providerId = providerId
        self.name = name
        self.release = release
        self.stream = stream
    }

    init(_ to: SwitchTo) {
        self.init(providerId: to.providerId, name: to.name, release: to.release, stream: to.stream)
    }

    /// The same row: a release by its source and id, a stream by its id.
    func same(_ other: SwitchTarget) -> Bool {
        if let stream { return stream == other.stream }
        return other.stream == nil && release == other.release && providerId == other.providerId
    }
}

/// A switch picked in the panel, from the tap until it lands or fails. The row it was picked on
/// shows it: nothing for 600 ms, then a spinner, then "Starting · 3 s"; a failure stays on the row
/// until another pick, with Try again.
public struct PanelSwitch: Equatable, Sendable {
    public enum Phase: Equatable, Sendable {
        /// Sent, and the engine has said nothing yet.
        case asked
        /// The release is being found; the episode plays on.
        case finding
        /// Found and loading at the playhead; the picture is held.
        case loading
        /// It did not start, and the episode plays on from what played before: the engine's
        /// line, for the row.
        case failed(String)
    }

    public var target: SwitchTarget
    public var phase: Phase
    /// When it was picked: what the row's wait counts from.
    public var since: ContinuousClock.Instant
    /// The candidate a release switch was picked from, for Try again.
    public var candidate: SourceCandidate?

    /// Whether this row is the one the switch was picked on.
    public func on(_ target: SwitchTarget) -> Bool { self.target.same(target) }

    public var failed: String? {
        if case .failed(let text) = phase { return text }
        return nil
    }

    /// How long the row has waited, in whole seconds.
    public func waited(at now: ContinuousClock.Instant) -> Int {
        max(0, Int((now - since).components.seconds))
    }

    /// The spinner shows once the pick has gone 600 ms without landing, so a quick switch never
    /// flickers one.
    public static let silent: Duration = .milliseconds(600)
    /// "Starting · 3 s" takes the detail line's place from here.
    public static let counting: Duration = .seconds(3)
}

/// A switch to the other version, subtitled or dubbed, from the pick until it lands, or its
/// failure until the next pick. The switch's fill moves at once; its picked segment shows a
/// spinner from 600 ms, and the line under it counts the seconds from 3 s, as a row's switch does.
public struct VersionSwitch: Equatable, Sendable {
    public enum Phase: Equatable, Sendable {
        /// Sent, and the engine has said nothing yet.
        case asked
        /// The other version is being found; the episode plays on.
        case finding
        /// Found and loading at the playhead; the picture is held.
        case loading
        /// It did not start, and the episode plays on in the version it was in: the engine's
        /// line, for under the switch.
        case failed(String)
    }

    /// The version picked.
    public var to: KaeriKit.Translation
    public var phase: Phase
    /// When it was picked: what the wait counts from.
    public var since: ContinuousClock.Instant

    public var failed: String? {
        if case .failed(let text) = phase { return text }
        return nil
    }

    /// Whether the picked segment shows a spinner: on its way, and past the silent 600 ms.
    public func spins(at now: ContinuousClock.Instant) -> Bool {
        failed == nil && now - since >= PanelSwitch.silent
    }

    /// The seconds the line under the switch counts, from 3 s; nil before then or once it failed.
    public func counting(at now: ContinuousClock.Instant) -> Int? {
        guard failed == nil, now - since >= PanelSwitch.counting else { return nil }
        return Int((now - since).components.seconds)
    }

    /// "Finding episode 4 dubbed · 5 s", from 3 s.
    public func note(episode: String, at now: ContinuousClock.Instant) -> String? {
        guard let seconds = counting(at: now) else { return nil }
        return to == .dub
            ? L.playerFindingVersionSeconds(episode: episode, seconds: seconds)
            : L.playerFindingVersionSubbedSeconds(episode: episode, seconds: seconds)
    }
}

/// The episode's releases across every source, for the panel's Sources tab: the engine's answer
/// to `LoadSources`, in the order the sources gave them, and each source's row while it asks.
public struct PanelReleases: Equatable, Sendable {
    public let id: AnilistId
    public let episode: String
    /// Nil until the engine answers.
    public var candidates: [SourceCandidate]?
    /// When the tab asked: a row still asked says nothing for the first 600 ms after it.
    public var asked: ContinuousClock.Instant
    /// Every source's row as the last `SourcesProgress` said it, in the order set in Sources;
    /// nil before the first.
    public var rows: [SourceListing]?
    /// When each source's row first said Asking, by its id: what its seconds count from.
    public var askingSince: [String: ContinuousClock.Instant] = [:]
    /// Why the engine asked no source for this episode, from `SourcesFailed`: none is set up,
    /// the title's details could not be read, or the episode has not aired. Ends the loading.
    public var failed: String?

    public init(
        id: AnilistId, episode: String, candidates: [SourceCandidate]? = nil, asked: ContinuousClock.Instant = .now
    ) {
        self.id = id
        self.episode = episode
        self.candidates = candidates
        self.asked = asked
    }

    /// One source's releases, in the order it gave them.
    public func of(_ providerId: String) -> [SourceCandidate] {
        (candidates ?? []).filter { $0.providerId == providerId }
    }

    /// Release `id` of `providerId`, from the engine's answer or, while the tab still loads, from
    /// the row that listed it.
    public func release(_ providerId: String, _ id: String) -> SourceCandidate? {
        let isIt = { (c: SourceCandidate) in c.providerId == providerId && c.id == id }
        if let found = candidates?.first(where: isIt) { return found }
        for row in rows ?? [] {
            if case .listed(let listed) = row.state, let found = listed.first(where: isIt) { return found }
        }
        return nil
    }

    /// A `SourcesProgress` for this episode: its rows replace the last ones whole, and a row that
    /// says Asking for the first time starts counting.
    mutating func progressed(_ rows: [SourceListing], at now: ContinuousClock.Instant) {
        for row in rows where row.state == .asking && askingSince[row.providerId] == nil {
            askingSince[row.providerId] = now
        }
        self.rows = rows
    }

    /// One source's line while the tab loads, from its row: `listed` is the line a source that
    /// listed releases shows, its coverage of the title.
    func loading(_ row: SourceListing, listed: String?) -> RowLoading {
        switch row.state {
        case .asking: RowLoading(asked: asked, since: askingSince[row.providerId] ?? asked)
        case .listed: RowLoading(asked: asked, said: listed ?? "")
        case .missed(let miss): RowLoading(asked: asked, said: Self.words(miss, episode: episode))
        case .off: RowLoading(asked: asked, said: L.sourceNoteOff)
        }
    }

    /// The starting screen's row words for a source that listed nothing.
    static func words(_ miss: ListingMiss, episode: String) -> String {
        switch miss {
        case .noMatch: L.playerStartRowNoMatch
        case .ambiguous: L.sourceNoteAmbiguous
        case .noEpisode: L.playerStartRowNoEpisode(episode: episode)
        case .failed: L.playerStartRowFailed
        case .timedOut: L.playerStartRowTimedOut
        case .heldBack(let why): StartingWalk.heldNote(why)
        }
    }
}

/// A source's line on the Sources tab while it loads: nothing for 600 ms after the tab asked, then
/// a spinner and Asking, then the seconds from 3 s, until the answer takes its place.
public struct RowLoading: Equatable, Sendable {
    public enum Phase: Equatable, Sendable {
        /// Asked, inside the silent 600 ms: an empty line held one line high.
        case silent
        /// "Asking", with a spinner, from 600 ms.
        case asking
        /// "Asking · 4 s", with a spinner, from 3 s.
        case counting(Int)
        /// The answer, or what the source is without one (switched off, not asked).
        case said(String)

        /// The line, nil while silent.
        public var text: String? {
            switch self {
            case .silent: nil
            case .asking: L.playerStartRowAsking
            case .counting(let seconds): L.playerPanelAsking(seconds: seconds)
            case .said(let text): text
            }
        }

        /// Which line it is: the seconds count on in place, every other change crossfades.
        var key: String {
            switch self {
            case .silent: "silent"
            case .asking: "asking"
            case .counting: "counting"
            case .said(let text): "said " + text
            }
        }

        /// The spinner in the check slot.
        public var spins: Bool {
            switch self {
            case .asking, .counting: true
            case .silent, .said: false
            }
        }
    }

    /// When the tab asked.
    public var asked: ContinuousClock.Instant
    /// When the row first said Asking; nil once it has its answer.
    public var since: ContinuousClock.Instant?
    /// The answer's line; nil while the source is asked.
    public var said: String?

    /// Still asked: the line changes with time.
    public var asking: Bool { said == nil }

    public func phase(at now: ContinuousClock.Instant) -> Phase {
        if let said { return .said(said) }
        guard now - asked >= PanelSwitch.silent else { return .silent }
        let waited = now - (since ?? asked)
        guard waited >= PanelSwitch.counting else { return .asking }
        return .counting(Int(waited.components.seconds))
    }
}

/// The picture on screen just before a switch loads, held over the video until the new stream's
/// first frame, then crossfaded away: libmpv clears its picture on `loadfile`.
public struct HeldPicture: Equatable, @unchecked Sendable {
    public let image: CGImage
    let serial: Int

    public static func == (lhs: HeldPicture, rhs: HeldPicture) -> Bool { lhs.serial == rhs.serial }
}

/// Words the panel's release rows share with the old Sources list.
enum ReleaseWords {
    /// "1080p · 1.4 GiB · lantern-archive · Dubbed · Automatic picks this", from the release's own
    /// fields only.
    static func detail(_ candidate: SourceCandidate) -> String {
        var parts: [String] = []
        if let quality = candidate.quality { parts.append("\(quality)p") }
        if let size = candidate.size { parts.append(size) }
        if let label = candidate.label, !label.isEmpty { parts.append(label) }
        if candidate.dubbed { parts.append(L.playerReleaseDub) } else if candidate.dualAudio { parts.append(L.playerReleaseDualAudio) }
        if candidate.autoPick { parts.append(L.playerReleaseAutomatic) }
        // Each part wraps whole: "Automatic picks this" never leaves "picks this" on a line alone.
        return parts.map { $0.replacingOccurrences(of: " ", with: "\u{00A0}") }.joined(separator: " · ")
    }
}

/// A screenshot's or recording's panel, as a viewer would have left it: the tab, the episode
/// revealed and the source opened. Each change is a new `serial`, so the same state can be asked
/// for again.
public struct PanelShot: Equatable, Sendable {
    public var tab: PanelTab
    public var revealed: String?
    public var expanded: String?
    /// The row scrolled to the top as the tab opens, by its id, with nothing opened.
    public var scroll: String?
    public var serial: Int

    public init(tab: PanelTab, revealed: String? = nil, expanded: String? = nil, scroll: String? = nil, serial: Int = 0) {
        self.tab = tab
        self.revealed = revealed
        self.expanded = expanded
        self.scroll = scroll
        self.serial = serial
    }
}
