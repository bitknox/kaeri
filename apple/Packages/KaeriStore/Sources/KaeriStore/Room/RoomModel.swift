// The room this device is in: the video's people, their states, the room's line and its chat
// (the room board, v5). The engine works out every state; this keeps what it said and words it
// for the screens, which never hold English of their own.

import Foundation
import KaeriKit
import Observation

/// How a face's ring is drawn: solid with the room, an arc while the room waits for them, none
/// (and the face dimmed) when they are not with the room.
public enum RoomRing: Equatable, Sendable {
    /// Water: watching, behind, or ready before play.
    case with
    /// Haze: the room is paused.
    case paused
    /// The room waits for them: the arc fills toward the 20 s cap.
    case waiting(Double)
    case none
}

/// What a face shows for one member.
public struct RoomFaceLook: Equatable, Sendable {
    public var ring: RoomRing
    /// The initials' opacity: 1 with the room, 0.5 not with it, 0.35 gone.
    public var dim: Double
    /// They paused the room.
    public var pauseBadge: Bool
    /// An error on their side: the vermilion dot.
    public var error: Bool

    public init(ring: RoomRing, dim: Double, pauseBadge: Bool, error: Bool) {
        self.ring = ring
        self.dim = dim
        self.pauseBadge = pauseBadge
        self.error = error
    }
}

/// The room's line: what is happening, in words, and the one thing to press when there is one.
public struct RoomStatus: Equatable, Sendable {
    public enum Action: Equatable, Sendable {
        /// Carry on without them, from 20 s.
        case carryOn(String)
        /// Before play, from 20 s.
        case start(String)
        /// An invite opened: Join the room (or Join anyway) and Not now.
        case join(over: Bool)
        /// Someone moved the room and ticked you.
        case goAlong
    }

    public var title: String
    public var detail: String
    public var action: Action?
    /// The room holds for someone: the line is about a wait.
    public var waits: Bool
}

/// What leaving asks about.
public enum RoomLeaveAsk: Equatable, Sendable {
    /// Leave, from the column or More.
    case leave
    /// ⌘W or the window's close: Leave, Keep the room or Cancel.
    case close
}

/// Which of the column's popovers is open.
public enum RoomPopover: Equatable, Sendable {
    case invite
    case details
    case editRelays
}

@Observable @MainActor
public final class RoomModel {
    /// The room, or nil outside one.
    public internal(set) var view: RoomView?
    /// Its members, in join order.
    public internal(set) var members: [RoomMember] = []
    /// The conversation, oldest first.
    public internal(set) var lines: [RoomLine] = []
    /// The muted names.
    public internal(set) var muted: Set<String> = []
    /// An invite opened and not joined yet.
    public internal(set) var invitation: RoomInvitation?
    /// Someone moved the room and ticked this member.
    public internal(set) var offer: RoomMoveOffer?
    /// The rooms kept on this device, newest first.
    public internal(set) var recent: [RoomRecent] = []
    /// A pasted link that led nowhere: none of its relays answered.
    public internal(set) var joinFailed: String?
    /// How many sources this device looked in for the room's episode.
    public internal(set) var sourceCount = 3
    /// The room is in the episode's opening: Skip opening shows for everyone (decision 7).
    public internal(set) var skipShown = false
    /// What this device's sources list for the room's episode, while choosing one.
    public internal(set) var choices: [RoomSourceChoice] = []
    public var chosen: String?
    /// Relays added by hand to this room's list.
    public internal(set) var extraRelays: [String] = []

    // MARK: What the screens have open

    /// The column beside the video on Mac (and iPad in a regular width).
    public var shown = true
    public var popover: RoomPopover?
    public var leaving: RoomLeaveAsk?
    public var moving = false
    /// Who Move to a new room takes, by member id.
    public var moveTicks: Set<String> = []
    /// The phone's room folded into the bar over the tabs.
    public var folded = false
    public var draft = ""
    /// The Join field on the place.
    public var joinText = ""
    /// The last change anyone made, for the full screen strip's 8 s.
    public private(set) var changedAt: Date = .distantPast

    @ObservationIgnored public var send: @MainActor (RoomAction) -> Void = { _ in }
    /// The player shows the room: ⌘W asks there.
    @ObservationIgnored public var playerShown = false
    /// Closes the player, for Leave and Keep the room from ⌘W.
    @ObservationIgnored public var closesPlayer: @MainActor () -> Void = {}
    /// Open sources and Search titles, from no source: the app's own places.
    @ObservationIgnored public var showSources: @MainActor () -> Void = {}
    @ObservationIgnored public var showSearch: @MainActor (String) -> Void = { _ in }

    public init() {}

    public var inRoom: Bool { view != nil }
    /// The room or its invitation is on screen.
    public var active: Bool { view != nil || invitation != nil }
    public var you: RoomMember? { members.first(where: \.you) }

    /// The members, you first, the rest in join order.
    public var people: [RoomMember] {
        members.filter(\.you) + members.filter { !$0.you }
    }

    // MARK: Words

    /// The room's position now, in seconds.
    public func position(at nowMs: Int64) -> Double {
        guard let view else { return 0 }
        guard view.playing, view.positionAtMs > 0 else { return view.position }
        return view.position + Double(nowMs - view.positionAtMs) / 1000
    }

    /// How long someone has been in their state, whole seconds.
    public static func seconds(since: Int64, now: Int64) -> Int {
        max(0, Int((now - since) / 1000))
    }

    /// Whom the room waits for.
    public var waitingFor: [String] { view?.waiting.map(\.name) ?? [] }

    private func waitSeconds(_ now: Int64) -> Int {
        guard let first = view?.waiting.map(\.sinceMs).min() else { return 0 }
        return Self.seconds(since: first, now: now)
    }

    /// The room's line.
    public func status(now: Int64) -> RoomStatus {
        if let invitation {
            return invitation.over
                ? RoomStatus(title: RoomCopy.invitedBy(invitation.by), detail: RoomCopy.overCap, action: .join(over: true), waits: false)
                : RoomStatus(
                    title: RoomCopy.invitedBy(invitation.by), detail: "\(invitation.title) · \(episodeName(invitation.episode.episode))",
                    action: .join(over: false), waits: false)
        }
        guard let view else { return RoomStatus(title: "", detail: "", action: nil, waits: false) }
        if let offer {
            return RoomStatus(
                title: RoomCopy.movedTheRoom(offer.by),
                detail: RoomCopy.goAlongAsk(offer.names, Self.clock(position(at: now))), action: .goAlong, waits: false)
        }
        if let moved = view.moved {
            return RoomStatus(title: RoomCopy.movedToNew(moved.by), detail: RoomCopy.wentAlong(moved.went), action: nil, waits: false)
        }
        if case .reconnecting = view.connection {
            return RoomStatus(title: RoomCopy.reconnectingToRoom, detail: RoomCopy.noRelay, action: nil, waits: false)
        }
        let down = view.relays.filter { !$0.healthy }.count
        if down > 0 {
            return RoomStatus(title: RoomCopy.watchingTogether, detail: RoomCopy.relaysDown(down, view.relays.count), action: nil, waits: false)
        }
        if let me = you, me.state == .behind, let behind = me.behindS {
            return RoomStatus(title: RoomCopy.watchingTogether, detail: RoomCopy.youreBehind(Int(behind.rounded())), action: nil, waits: false)
        }
        let waiting = waitingFor
        let secs = waitSeconds(now)
        if let me = you, waiting.contains(me.name) {
            let others = members.filter { !$0.you && $0.state != .left }.map(\.name)
            return RoomStatus(
                title: RoomCopy.pausedForYou, detail: RoomCopy.areWaiting(RoomCopy.names(others), others.count > 1, secs),
                action: nil, waits: true)
        }
        let names = RoomCopy.names(waiting)
        let many = waiting.count > 1
        let who = many ? RoomCopy.them : names
        if !view.started {
            if let n = view.startingIn {
                return RoomStatus(title: RoomCopy.startingIn(n), detail: RoomCopy.everyoneReady, action: nil, waits: false)
            }
            if waiting.isEmpty {
                return RoomStatus(title: RoomCopy.notStarted, detail: RoomCopy.playStartsIt, action: nil, waits: false)
            }
            if view.canCarryOn {
                return RoomStatus(title: RoomCopy.stillLoading(names, many), detail: RoomCopy.waited20, action: .start(who), waits: true)
            }
            return RoomStatus(
                title: RoomCopy.startingWhen(names, many), detail: RoomCopy.seconds(RoomCopy.loading, secs), action: nil, waits: true)
        }
        if let up = view.upNext, up.cancelledBy == nil, let ends = up.endsAtMs {
            let left = max(0, Int((Double(ends - now) / 1000).rounded(.up)))
            return RoomStatus(title: RoomCopy.upNextIn(left), detail: RoomCopy.playsForEveryone(up.episode), action: nil, waits: false)
        }
        if !waiting.isEmpty {
            if view.canCarryOn {
                return RoomStatus(title: RoomCopy.stillLoading(names, many), detail: RoomCopy.waited20, action: .carryOn(who), waits: true)
            }
            let buffering = view.waiting.count == 1 && view.waiting[0].reason == .buffering
            let word = buffering ? RoomCopy.buffering : RoomCopy.loading
            return RoomStatus(title: RoomCopy.pausedFor(names), detail: RoomCopy.seconds(word, secs), action: nil, waits: true)
        }
        if let carried = view.carriedOn {
            return RoomStatus(
                title: RoomCopy.watchingTogether,
                detail: RoomCopy.carriedOn(carried.by, RoomCopy.names(carried.past), carried.past.count > 1), action: nil, waits: false)
        }
        if !view.playing, let by = view.pausedBy {
            let name = members.first(where: { $0.name == by })?.you == true ? RoomCopy.you : by
            return RoomStatus(title: RoomCopy.paused(name), detail: RoomCopy.at(Self.clock(view.position)), action: nil, waits: false)
        }
        return RoomStatus(
            title: RoomCopy.watchingTogether, detail: RoomCopy.episodeAt(view.episode.episode, Self.clock(position(at: now))),
            action: nil, waits: false)
    }

    /// Whether the room waits for this member.
    public func waits(for member: RoomMember) -> Bool {
        view?.waiting.contains { $0.name == member.name } ?? false
    }

    /// The word under a member's name.
    public func word(_ member: RoomMember, now: Int64) -> String {
        let secs = Self.seconds(since: member.sinceMs, now: now)
        let word: String
        switch member.state {
        case .watching:
            word = view?.started == false ? RoomCopy.ready : RoomCopy.watching
        case .paused: word = member.pausedRoom ? RoomCopy.pausedIt : RoomCopy.paused
        case .loading: word = RoomCopy.seconds(RoomCopy.loading, secs)
        case .buffering: word = RoomCopy.seconds(RoomCopy.buffering, secs)
        case .behind: word = RoomCopy.seconds(RoomCopy.behind, Int((member.behindS ?? 0).rounded()))
        case .otherEpisode: word = RoomCopy.onEpisode(member.episode ?? "")
        case .choosing: word = RoomCopy.choosing
        case .noSource: word = RoomCopy.noSourceFor(view?.episode.episode ?? "")
        case .playerClosed: word = RoomCopy.playerClosed
        case .reconnecting: word = RoomCopy.reconnecting
        case .left: word = RoomCopy.left
        }
        return muted.contains(member.name) ? "\(word) · \(RoomCopy.muted)" : word
    }

    /// A member's face.
    public func look(_ member: RoomMember, now: Int64) -> RoomFaceLook {
        let secs = Double(Self.seconds(since: member.sinceMs, now: now))
        switch member.state {
        case .watching, .behind:
            return RoomFaceLook(ring: .with, dim: 1, pauseBadge: false, error: false)
        case .paused:
            return RoomFaceLook(ring: .paused, dim: 1, pauseBadge: member.pausedRoom, error: false)
        case .loading, .buffering:
            return RoomFaceLook(ring: .waiting(min(1, max(0.04, secs / 20))), dim: 1, pauseBadge: false, error: false)
        case .noSource:
            return RoomFaceLook(ring: .none, dim: 0.5, pauseBadge: false, error: true)
        case .left:
            return RoomFaceLook(ring: .none, dim: 0.35, pauseBadge: false, error: false)
        case .otherEpisode, .choosing, .playerClosed, .reconnecting:
            return RoomFaceLook(ring: .none, dim: 0.5, pauseBadge: false, error: false)
        }
    }

    /// Whether the member is with the room: watching (or ready before play).
    public static func isWatching(_ member: RoomMember) -> Bool {
        member.state == .watching
    }

    /// The full screen strip stays while anyone isn't watching (decision 8).
    public var anyoneNotWatching: Bool {
        members.contains { $0.state != .watching && $0.state != .paused }
    }

    /// "Episode 9"
    public func episodeName(_ episode: String) -> String { "Episode \(episode)" }

    /// 12:04
    public static func clock(_ seconds: Double) -> String {
        let s = max(0, Int(seconds.rounded()))
        return "\(s / 60):" + String(format: "%02d", s % 60)
    }

    // MARK: Actions

    public func carryOn() { act(.carryOn) }
    public func dontWait() { act(.dontWait) }
    public func skipOpening() { act(.skipOpening) }
    public func cancelUpNext() { act(.cancelUpNext) }
    public func playNextNow() { act(.playNextNow) }
    public func play() { act(.play) }
    public func newInvite() { act(.newInvite) }
    public func accept() { act(.accept) }
    public func notNow() {
        invitation = nil
        act(.leave)
    }

    public func answer(go: Bool) {
        guard let offer else { return }
        act(.answerMove(offer: offer.id, go: go))
        self.offer = nil
    }

    public func say() {
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        draft = ""
        act(.say(text))
    }

    public func react(_ reaction: Reaction) { act(.react(reaction)) }

    /// Watch together: a room starts from the episode playing, never empty.
    public func start(id: AnilistId, episode: String) {
        shown = true
        act(.start(id: id, episode: episode))
    }

    public func mute(_ name: String, _ on: Bool) {
        if on { muted.insert(name) } else { muted.remove(name) }
        act(.mute(name, on))
    }

    /// Open Move to a new room: everyone present ticked, except muted people.
    public func startMove() {
        popover = nil
        moveTicks = Set(members.filter { !$0.you && $0.state != .left && !muted.contains($0.name) }.map(\.id))
        moving = true
    }

    public func move() {
        guard !moveTicks.isEmpty else { return }
        moving = false
        act(.moveRoom(take: Array(moveTicks).sorted()))
    }

    public func leave() {
        let closing = leaving == .close
        leaving = nil
        act(.leave)
        if closing { closesPlayer() }
    }

    /// Keep the room: the player closes and this device stays in.
    public func keepRoom() {
        leaving = nil
        act(.closePlayer)
        closesPlayer()
    }

    /// The red button or ⌘W on the player's window while in a room: Leave, Keep the room or
    /// Cancel, asked over the player. Nothing is asked without the player.
    public func asksBeforeClosing() -> Bool {
        guard inRoom, playerShown else { return false }
        leaving = .close
        return true
    }

    public func join() {
        let link = joinText.trimmingCharacters(in: .whitespaces)
        guard !link.isEmpty else { return }
        act(.join(link: link))
    }

    public func openSources() { showSources() }
    public func searchTitles() { showSearch(view?.title ?? "") }

    public func setRelays(_ relays: [String]) { act(.setRelays(relays)) }

    /// Play in the room: the source picked plays at the room's place.
    public func playChosen() {
        guard let chosen, let choice = choices.first(where: { $0.id == chosen }) else { return }
        act(.playSource(choice.id))
    }

    /// Not now, from choosing a source: back to before.
    public func stopChoosing() { act(.choosingSource(open: false)) }

    public func rejoin(_ room: RoomRecent) { act(.rejoin(roomId: room.roomId)) }

    /// Open the player at the room's place, from Watch together.
    public func openPlayer() {
        shown = true
        act(.openPlayer)
    }

    private func act(_ action: RoomAction) {
        changedAt = .now
        send(action)
    }

    /// Something in the room changed: the strip shows for 8 s.
    public func changed() { changedAt = .now }

    /// A screenshot's still moment: the room's clocks read it instead of the time, so a shot's
    /// seconds are the scene's. Only a Fixtures run sets it.
    public static var stillAt: Int64?

    #if FIXTURES
    /// A screenshot run's room, as a scene stages it.
    public func stage(
        view: RoomView?, members: [RoomMember], lines: [RoomLine], invitation: RoomInvitation? = nil,
        offer: RoomMoveOffer? = nil, muted: Set<String> = []
    ) {
        self.view = view
        self.members = members
        self.lines = lines
        self.invitation = invitation
        self.offer = offer
        self.muted = muted
        changedAt = .now
    }

    public func stage(skip: Bool) { skipShown = skip }

    public func stage(choices: [RoomSourceChoice], extraRelays: [String] = []) {
        self.choices = choices
        chosen = choices.first?.id
        self.extraRelays = extraRelays
    }

    public func stage(recent: [RoomRecent], joinFailed: String?) {
        self.recent = recent
        self.joinFailed = joinFailed
        if let joinFailed { joinText = joinFailed }
    }

    public func clear() {
        view = nil
        members = []
        lines = []
        invitation = nil
        offer = nil
        popover = nil
        leaving = nil
        moving = false
    }
    #endif
}

/// A source that lists the room's episode.
public struct RoomSourceChoice: Equatable, Sendable, Identifiable {
    public var id: String
    public var name: String
    /// "1080p · 1.2 GB"
    public var detail: String

    public init(id: String, name: String, detail: String) {
        self.id = id
        self.name = name
        self.detail = detail
    }
}
