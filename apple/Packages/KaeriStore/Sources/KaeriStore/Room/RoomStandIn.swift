// Stand-in types for the room's engine surface, shaped like the room spec's section 6 (RoomView,
// RoomMember, MemberState, PausedFor, RoomUpNext, RelayHealth, MoveCandidate, RoomMoveOffer,
// RoomConnection). They go when the engine's FFI lands and KaeriKit carries the real ones; the
// screens read them only through `RoomModel`.

import Foundation
import KaeriKit

/// One person's state, as the room's rows show it (spec section 5).
public enum RoomMemberState: Equatable, Sendable {
    case watching
    case paused
    case loading
    case buffering
    case behind
    case otherEpisode
    case choosing
    case noSource
    case playerClosed
    case reconnecting
    case left
}

/// Why the room waits for someone.
public enum RoomWaitReason: Equatable, Sendable {
    case loading
    case seeking
    case buffering
}

/// Someone the room waits for, and since when (epoch ms on this device's clock).
public struct RoomPausedFor: Equatable, Sendable {
    public var name: String
    public var reason: RoomWaitReason
    public var sinceMs: Int64

    public init(name: String, reason: RoomWaitReason, sinceMs: Int64) {
        self.name = name
        self.reason = reason
        self.sinceMs = sinceMs
    }
}

/// The room's Up next card.
public struct RoomUpNext: Equatable, Sendable {
    /// The episode it plays, as a number ("10"), and its name when known.
    public var episode: String
    public var name: String?
    /// When it plays, or nil while the card waits for a longer file.
    public var endsAtMs: Int64?
    public var waitingFor: [String]
    public var cancelledBy: String?

    public init(episode: String, name: String? = nil, endsAtMs: Int64?, waitingFor: [String] = [], cancelledBy: String? = nil) {
        self.episode = episode
        self.name = name
        self.endsAtMs = endsAtMs
        self.waitingFor = waitingFor
        self.cancelledBy = cancelledBy
    }
}

/// One of the room's relays.
public struct RoomRelayHealth: Equatable, Sendable, Identifiable {
    public var url: String
    public var healthy: Bool
    public var rttMs: Int?
    public var id: String { url }

    public init(url: String, healthy: Bool, rttMs: Int?) {
        self.url = url
        self.healthy = healthy
        self.rttMs = rttMs
    }
}

/// Someone Move to a new room can take along.
public struct RoomMoveCandidate: Equatable, Sendable, Identifiable {
    public var id: String
    public var name: String
    public var muted: Bool
    public var ticked: Bool

    public init(id: String, name: String, muted: Bool, ticked: Bool) {
        self.id = id
        self.name = name
        self.muted = muted
        self.ticked = ticked
    }
}

/// Someone moved the room and ticked this member: Go along or Stay here.
public struct RoomMoveOffer: Equatable, Sendable, Identifiable {
    public var id: String
    public var by: String
    /// Who else was ticked.
    public var names: [String]
    public var title: String
    public var episode: String
    public var expiresAtMs: Int64

    public init(id: String, by: String, names: [String], title: String, episode: String, expiresAtMs: Int64) {
        self.id = id
        self.by = by
        self.names = names
        self.title = title
        self.episode = episode
        self.expiresAtMs = expiresAtMs
    }
}

/// This device's way to the room.
public enum RoomConnection: Equatable, Sendable {
    case connecting
    case connected
    case reconnecting(relaysUp: Int)
}

/// The room's episode.
public struct RoomEpisode: Equatable, Sendable {
    public var id: AnilistId
    public var episode: String
    /// "The Long Leg"
    public var name: String?
    /// Its length in seconds, when known.
    public var duration: Double?

    public init(id: AnilistId, episode: String, name: String? = nil, duration: Double? = nil) {
        self.id = id
        self.episode = episode
        self.name = name
        self.duration = duration
    }
}

/// One person in the room.
public struct RoomMember: Equatable, Sendable, Identifiable {
    public var id: String
    public var name: String
    public var you: Bool
    public var state: RoomMemberState
    /// When the state began (epoch ms), for the seconds a row counts.
    public var sinceMs: Int64
    public var behindS: Float?
    /// The episode they are on, when it isn't the room's.
    public var episode: String?
    /// They paused the room (the badge).
    public var pausedRoom: Bool
    public var background: Bool
    /// Before play: their first frame is ready.
    public var ready: Bool

    public init(
        id: String, name: String, you: Bool = false, state: RoomMemberState, sinceMs: Int64 = 0, behindS: Float? = nil,
        episode: String? = nil, pausedRoom: Bool = false, background: Bool = false, ready: Bool = false
    ) {
        self.id = id
        self.name = name
        self.you = you
        self.state = state
        self.sinceMs = sinceMs
        self.behindS = behindS
        self.episode = episode
        self.pausedRoom = pausedRoom
        self.background = background
        self.ready = ready
    }
}

/// After a move, for whoever stayed: who moved and who went along.
public struct RoomMoved: Equatable, Sendable {
    public var by: String
    public var went: [String]

    public init(by: String, went: [String]) {
        self.by = by
        self.went = went
    }
}

/// The room this device is in (`RoomView` in the spec).
public struct RoomView: Equatable, Sendable {
    public var roomId: String
    public var title: String
    public var episode: RoomEpisode
    /// The room's position in seconds, and when it was read (epoch ms).
    public var position: Double
    public var positionAtMs: Int64
    /// The effective state: playing, or held (paused or waiting).
    public var playing: Bool
    /// Before anyone pressed Play for everyone: the room before play.
    public var started: Bool
    /// "Starting in 3" before play, once everyone is ready.
    public var startingIn: Int?
    public var pausedBy: String?
    public var waiting: [RoomPausedFor]
    /// From 20 s into a wait.
    public var canCarryOn: Bool
    /// This device is waited for.
    public var canDontWait: Bool
    /// Who carried on past whom in the current wait, for the line until they join.
    public var carriedOn: (by: String, past: [String])?
    public var upNext: RoomUpNext?
    public var relays: [RoomRelayHealth]
    public var connection: RoomConnection
    public var link: String
    public var moveCandidates: [RoomMoveCandidate]
    public var moved: RoomMoved?

    public init(
        roomId: String, title: String, episode: RoomEpisode, position: Double, positionAtMs: Int64 = 0, playing: Bool,
        started: Bool = true, startingIn: Int? = nil, pausedBy: String? = nil, waiting: [RoomPausedFor] = [],
        canCarryOn: Bool = false, canDontWait: Bool = false, carriedOn: (by: String, past: [String])? = nil,
        upNext: RoomUpNext? = nil, relays: [RoomRelayHealth] = [], connection: RoomConnection = .connected,
        link: String, moveCandidates: [RoomMoveCandidate] = [], moved: RoomMoved? = nil
    ) {
        self.roomId = roomId
        self.title = title
        self.episode = episode
        self.position = position
        self.positionAtMs = positionAtMs
        self.playing = playing
        self.started = started
        self.startingIn = startingIn
        self.pausedBy = pausedBy
        self.waiting = waiting
        self.canCarryOn = canCarryOn
        self.canDontWait = canDontWait
        self.carriedOn = carriedOn
        self.upNext = upNext
        self.relays = relays
        self.connection = connection
        self.link = link
        self.moveCandidates = moveCandidates
        self.moved = moved
    }

    public static func == (a: RoomView, b: RoomView) -> Bool {
        a.roomId == b.roomId && a.title == b.title && a.episode == b.episode && a.position == b.position
            && a.positionAtMs == b.positionAtMs && a.playing == b.playing && a.started == b.started
            && a.startingIn == b.startingIn && a.pausedBy == b.pausedBy && a.waiting == b.waiting
            && a.canCarryOn == b.canCarryOn && a.canDontWait == b.canDontWait
            && a.carriedOn?.by == b.carriedOn?.by && a.carriedOn?.past == b.carriedOn?.past
            && a.upNext == b.upNext && a.relays == b.relays && a.connection == b.connection && a.link == b.link
            && a.moveCandidates == b.moveCandidates && a.moved == b.moved
    }
}

/// An invite opened and not joined yet: the room shows behind its banner, and nobody sees you.
public struct RoomInvitation: Equatable, Sendable {
    public var by: String
    public var title: String
    public var episode: RoomEpisode
    public var names: [String]

    public init(by: String, title: String, episode: RoomEpisode, names: [String]) {
        self.by = by
        self.title = title
        self.episode = episode
        self.names = names
    }

    /// Past the soft cap of 10.
    public var over: Bool { names.count >= 10 }
}

/// A line in the room's conversation.
public struct RoomLine: Identifiable, Equatable, Sendable {
    public enum Kind: Equatable, Sendable {
        /// "Ren paused at 3:10": the room says it.
        case system(String)
        case chat(from: String, text: String, you: Bool)
        case reaction(from: String, reaction: Reaction, at: String)
    }

    public var id: UInt64
    public var kind: Kind

    public init(id: UInt64, kind: Kind) {
        self.id = id
        self.kind = kind
    }
}

/// A recent room kept on this device.
public struct RoomRecent: Equatable, Sendable, Identifiable {
    public var roomId: String
    public var title: String
    public var id: AnilistId
    public var episode: String
    public var others: [String]
    public var lastAt: String
    /// Rejoin found nobody answering.
    public var quiet: Bool

    public var key: String { roomId }

    public init(roomId: String, title: String, id: AnilistId, episode: String, others: [String], lastAt: String, quiet: Bool = false) {
        self.roomId = roomId
        self.title = title
        self.id = id
        self.episode = episode
        self.others = others
        self.lastAt = lastAt
        self.quiet = quiet
    }
}

/// What the screens ask of the room: the spec's `RoomCommand`s, as the app sends them.
public enum RoomAction: Equatable, Sendable {
    case carryOn
    case dontWait
    case skipOpening
    case cancelUpNext
    case playNextNow
    case choosingSource(open: Bool)
    case closePlayer
    case newInvite
    case moveRoom(take: [String])
    case answerMove(offer: String, go: Bool)
    case setRelays([String])
    case say(String)
    case react(Reaction)
    case mute(String, Bool)
    case leave
    case join(link: String)
    case rejoin(roomId: String)
    case accept
    case play
    case playSource(String)
    /// Watch together from the player or a title's page: a room starts from this episode.
    case start(id: AnilistId, episode: String)
    /// The player closed (Keep the room) opens again at the room's place.
    case openPlayer
}
