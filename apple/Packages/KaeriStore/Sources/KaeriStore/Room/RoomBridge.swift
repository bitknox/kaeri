// The engine's room (KaeriKit's RoomView, RoomMember and the rest, from the room's FFI) as the
// room's screens read it, and the few things only the app keeps: an invite opened and not
// joined, the conversation's lines, the recent rooms with what Rejoin found.

import Foundation
import KaeriKit

extension RoomView {
    /// The show the room is watching.
    public var title: String { roomEpisode?.title ?? name }
    /// Someone pressed Play for everyone.
    public var started: Bool { !beforePlay }
    /// The room's episode, as a number ("9").
    public var episodeNumber: String { roomEpisode?.episode ?? episode?.episode ?? "" }
    public var episodeId: AnilistId? { roomEpisode?.id ?? episode?.id }
    /// "Starting in 3" before play, once everyone is ready.
    public func startingIn(now: Int64) -> Int? {
        startsAtMs.map { max(1, Int((Double($0 - now) / 1000).rounded(.up))) }
    }
}

extension SourceCandidate {
    /// One release among every source's, for the room's source list.
    public var roomKey: String { "\(providerId)/\(id)" }
}

extension RoomMember: @retroactive Identifiable {}
extension MoveCandidate: @retroactive Identifiable {}
extension RoomMoveOffer: @retroactive Identifiable {}
extension RelayHealth: @retroactive Identifiable {
    public var id: String { url }
}

/// An invite opened and not joined yet: the room shows behind its banner, and nobody sees you.
public struct RoomInvitation: Equatable, Sendable {
    public var by: String
    public var title: String
    public var id: AnilistId?
    public var episode: String
    public var names: [String]
    /// The room is at its cap: Join anyway.
    public var full: Bool
    /// The link that was opened, for Join the room.
    public var link: String?

    public init(by: String, title: String, id: AnilistId?, episode: String, names: [String], full: Bool, link: String?) {
        self.by = by
        self.title = title
        self.id = id
        self.episode = episode
        self.names = names
        self.full = full
        self.link = link
    }

    public var over: Bool { full }
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

    /// The engine's line, or nil for a muted member's.
    init?(_ line: RoomChatLine) {
        guard !line.muted else { return nil }
        id = line.id
        switch line.body {
        case .said(let text): kind = .chat(from: line.from, text: text, you: line.you)
        case .reacted(let reaction, let at, _): kind = .reaction(from: line.from, reaction: reaction, at: RoomModel.clock(at))
        case .room(let message): kind = .system(message.text)
        }
    }
}

/// A recent room kept on this device, as Watch together lists it.
public struct RecentRoom: Equatable, Sendable, Identifiable {
    public var roomId: String
    public var title: String
    public var id: AnilistId
    public var episode: String
    public var others: [String]
    /// "19:25", "Tuesday", "18 November".
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

    /// When a room was last open: the time today, the weekday this week, else the date.
    static func when(_ ms: Int64, now: Date = .now) -> String {
        let date = Date(timeIntervalSince1970: Double(ms) / 1000)
        let calendar = Calendar.current
        if calendar.isDate(date, inSameDayAs: now) { return date.formatted(date: .omitted, time: .shortened) }
        if let days = calendar.dateComponents([.day], from: date, to: now).day, days < 7 {
            return date.formatted(.dateTime.weekday(.wide))
        }
        return date.formatted(.dateTime.day().month(.wide))
    }
}

/// Why joining a room from a link or Rejoin didn't happen.
public enum RoomJoinFailure: Equatable, Sendable {
    /// None of the link's relays, nor the spares, answered.
    case unreachable
    case full
    case oldInvite
    case failed(String)
}
