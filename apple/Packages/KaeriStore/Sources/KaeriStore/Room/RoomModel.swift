// The room this device is in: the video's people, their states, the room's line and its chat
// (the room board, v5). The engine works out every state (the room's FFI: RoomView, RoomMember,
// Command.room); this keeps what it said and words it for the screens, which never hold English
// of their own.

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
        /// Carry on without them, from 20 s: whom it leaves behind.
        case carryOn([String])
        /// Before play, from 20 s.
        case start([String])
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
    /// Its members, this device first.
    public internal(set) var members: [RoomMember] = []
    /// The conversation, oldest first, without muted members' lines.
    public internal(set) var lines: [RoomLine] = []
    /// The chat is off in Settings: no lines or field over the video.
    public internal(set) var chatHidden = false
    /// An invite opened and not joined yet.
    public internal(set) var invitation: RoomInvitation?
    /// Someone moved the room and ticked this member.
    public internal(set) var offer: RoomMoveOffer?
    /// The rooms kept on this device, newest first.
    public internal(set) var recent: [RecentRoom] = []
    /// A pasted link that led nowhere, and why.
    public internal(set) var joinFailed: String?
    public internal(set) var joinFailure: RoomJoinFailure?
    /// How many sources this device looked in for the room's episode.
    public var sourceCount: Int { countSources() }
    /// The room is in the episode's opening: Skip opening shows for everyone, and skips to here.
    public var skipTo: Double?
    public var skipShown: Bool { skipTo != nil }
    /// What this device's sources list for the room's episode, while choosing one.
    public internal(set) var choices: [SourceCandidate] = []
    public var chosen: String?

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
    /// Sideways on iPhone: the chat field is open over the video.
    public var writing = false
    /// The Join field on the place.
    public var joinText = ""
    /// The last change anyone made, for the full screen strip's 8 s.
    public private(set) var changedAt: Date = .distantPast

    /// Rooms Rejoin found quiet, by room id.
    private var quiet: Set<String> = []
    private var kept: [KaeriKit.RoomRecent] = []
    /// What a Join or a Rejoin is waiting on: a failure is about it.
    private var joining: String?
    private var rejoining: String?
    /// A room looked into from Watch together: its glimpse isn't an invitation.
    private var lookingIn: String?
    private var peeked: String?

    @ObservationIgnored public var send: @MainActor (Command) -> Void = { _ in }
    @ObservationIgnored public var countSources: @MainActor () -> Int = { 0 }
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

    /// The members, you first, the rest in join order; nobody who left.
    public var people: [RoomMember] {
        members.filter(\.you) + members.filter { !$0.you }
    }

    // MARK: Words

    /// The room's position now, in seconds.
    public func position(at nowMs: Int64) -> Double {
        guard let view else { return 0 }
        let position = view.position ?? 0
        guard view.playing, view.positionAtMs > 0 else { return position }
        return position + Double(nowMs - view.positionAtMs) / 1000
    }

    /// How long someone has been in their state, whole seconds.
    public static func seconds(since: Int64?, now: Int64) -> Int {
        guard let since else { return 0 }
        return max(0, Int((now - since) / 1000))
    }

    /// Whom the room waits for.
    public var waitingFor: [String] { view?.waiting.map(\.name) ?? [] }

    private func waitSeconds(_ now: Int64) -> Int {
        Self.seconds(since: view?.waitShownAtMs ?? view?.waiting.map(\.sinceMs).min(), now: now)
    }

    /// The room's line.
    public func status(now: Int64) -> RoomStatus {
        if let invitation {
            return invitation.over
                ? RoomStatus(
                    title: RoomCopy.invitedBy(invitation.by), detail: RoomCopy.overCap(invitation.names.count),
                    action: .join(over: true), waits: false)
                : RoomStatus(
                    title: RoomCopy.invitedBy(invitation.by), detail: RoomCopy.invitedDetail(invitation.title, invitation.episode),
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
        if view.waiting.contains(where: \.you) {
            let others = members.filter { !$0.you && $0.state != .left }.map(\.name)
            return RoomStatus(title: RoomCopy.pausedForYou, detail: RoomCopy.areWaiting(others, secs), action: nil, waits: true)
        }
        if !view.started {
            if let n = view.startingIn(now: now) {
                return RoomStatus(title: RoomCopy.startingIn(n), detail: RoomCopy.everyoneReady, action: nil, waits: false)
            }
            if waiting.isEmpty {
                return RoomStatus(title: RoomCopy.notStarted, detail: RoomCopy.playStartsIt, action: nil, waits: false)
            }
            if view.canCarryOn {
                return RoomStatus(title: RoomCopy.stillLoading(waiting), detail: RoomCopy.waited(secs), action: .start(waiting), waits: true)
            }
            return RoomStatus(
                title: RoomCopy.startingWhen(waiting), detail: RoomCopy.loading(secs), action: nil, waits: true)
        }
        if let up = view.upNext, up.cancelledBy == nil, let ends = up.endsAtMs {
            let left = max(0, Int((Double(ends - now) / 1000).rounded(.up)))
            return RoomStatus(title: RoomCopy.upNextIn(left), detail: RoomCopy.playsForEveryone(up.episode.episode), action: nil, waits: false)
        }
        if !waiting.isEmpty {
            if view.canCarryOn {
                return RoomStatus(title: RoomCopy.stillLoading(waiting), detail: RoomCopy.waited(secs), action: .carryOn(waiting), waits: true)
            }
            let buffering = view.waiting.count == 1 && view.waiting[0].reason == .buffering
            return RoomStatus(
                title: RoomCopy.pausedFor(waiting), detail: buffering ? RoomCopy.buffering(secs) : RoomCopy.loading(secs),
                action: nil, waits: true)
        }
        if let carried = view.carriedOn, !carried.past.isEmpty {
            return RoomStatus(
                title: RoomCopy.watchingTogether, detail: RoomCopy.carriedOn(carried.byYou ? RoomCopy.you : carried.by, carried.past),
                action: nil, waits: false)
        }
        if !view.playing, view.pausedBy != nil || view.pausedByYou {
            return RoomStatus(
                title: RoomCopy.paused(by: view.pausedByYou ? nil : view.pausedBy), detail: RoomCopy.at(Self.clock(view.position ?? 0)),
                action: nil, waits: false)
        }
        return RoomStatus(
            title: RoomCopy.watchingTogether, detail: RoomCopy.episodeAt(view.episodeNumber, Self.clock(position(at: now))),
            action: nil, waits: false)
    }

    /// Whether the room waits for this member.
    public func waits(for member: RoomMember) -> Bool {
        view?.waiting.contains { $0.id == member.id } ?? false
    }

    /// The word under a member's name.
    public func word(_ member: RoomMember, now: Int64) -> String {
        let secs = Self.seconds(since: member.sinceMs, now: now)
        let word: String
        switch member.state {
        case .watching: word = view?.started == false ? RoomCopy.ready : RoomCopy.watching
        case .ready: word = RoomCopy.ready
        case .paused: word = member.pausedRoom ? RoomCopy.pausedIt : RoomCopy.paused
        case .loading: word = RoomCopy.loading(secs)
        case .buffering: word = RoomCopy.buffering(secs)
        case .behind: word = RoomCopy.behind(Int((member.behindS ?? 0).rounded()))
        case .otherEpisode: word = RoomCopy.onEpisode(member.episode?.episode ?? "")
        case .choosing: word = RoomCopy.choosing
        case .noSource: word = RoomCopy.noSourceFor(view?.episodeNumber ?? "")
        case .playerClosed: word = RoomCopy.playerClosed
        case .reconnecting: word = RoomCopy.reconnecting
        case .left: word = RoomCopy.left
        }
        return member.muted ? RoomCopy.andMuted(word) : word
    }

    /// A member's face.
    public func look(_ member: RoomMember, now: Int64) -> RoomFaceLook {
        let secs = Double(Self.seconds(since: member.sinceMs, now: now))
        switch member.state {
        case .watching, .ready, .behind:
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
        member.state == .watching || member.state == .ready
    }

    /// The full screen strip stays while anyone isn't watching (decision 8).
    public var anyoneNotWatching: Bool {
        members.contains { !Self.isWatching($0) && $0.state != .paused }
    }

    /// "Episode 9"
    public func episodeName(_ episode: String) -> String { RoomCopy.upNextEpisode(episode, nil) }

    /// 12:04
    nonisolated public static func clock(_ seconds: Double) -> String {
        let s = max(0, Int(seconds.rounded()))
        return "\(s / 60):" + String(format: "%02d", s % 60)
    }

    // MARK: Actions

    public func carryOn() { act(.carryOn) }
    public func dontWait() { act(.dontWait) }
    public func skipOpening() {
        guard let skipTo else { return }
        act(.skipOpening(to: skipTo))
    }
    public func cancelUpNext() { act(.cancelUpNext) }
    public func playNextNow() { act(.playNextNow) }
    /// Play for everyone before play (and Carry on during a wait).
    public func play() { act(.play(play: true)) }
    public func newInvite() { act(.newInvite) }

    /// Join the room, from an invite opened.
    public func accept() {
        guard let invitation else { return }
        joining = invitation.link
        act(.lobby(invite: invitation.link, name: nil, pick: nil, anyway: invitation.full))
    }

    /// Not now, from an invite opened: nothing was joined, so nothing is left.
    public func notNow() {
        invitation = nil
        peeked = nil
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
        act(.say(text: text))
    }

    public func react(_ reaction: Reaction) { act(.react(reaction: reaction)) }

    /// Watch together: a room starts from the episode playing, never empty.
    public func start(id: AnilistId, episode: String) {
        shown = true
        act(.lobby(invite: nil, name: nil, pick: RoomEpisode(id: id, episode: episode), anyway: false))
    }

    public func mute(_ member: RoomMember, _ on: Bool) {
        act(.mute(id: member.id, muted: on))
    }

    /// Open Move to a new room: everyone present ticked, except muted people.
    public func startMove() {
        popover = nil
        moveTicks = Set(members.filter { !$0.you && $0.state != .left && !$0.muted }.map(\.id))
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

    /// Join, on Watch together's page: the pasted link.
    public func join() {
        let link = joinText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !link.isEmpty else { return }
        joinFailed = nil
        joinFailure = nil
        joining = link
        act(.lobby(invite: link, name: nil, pick: nil, anyway: false))
    }

    /// An invite link opened from outside the app: the room shows behind its banner first.
    public func open(invite link: String) {
        peeked = link
        send(.room(.peekInvite(invite: link)))
    }

    public func openSources() { showSources() }
    public func searchTitles() { showSearch(view?.title ?? "") }

    /// Edit relays, for this room and everyone in it.
    public func setRelays(_ relays: [String]) { act(.editRelays(relays: relays)) }

    /// Choosing a source for the room's episode: what each source lists, loading in place.
    public func chooseSource() {
        guard let view, let id = view.episodeId else { return }
        choices = []
        chosen = nil
        act(.choosingSource(open: true))
        send(.loadSources(id: id, episode: view.episodeNumber))
    }

    /// Play in the room: the source picked plays at the room's place.
    public func playChosen() {
        guard let view, let id = view.episodeId, let chosen, let choice = choices.first(where: { $0.roomKey == chosen }) else { return }
        send(.playSource(id: id, episode: view.episodeNumber, providerId: choice.providerId, sourceId: choice.id))
    }

    /// Not now, from choosing a source: back to before.
    public func stopChoosing() { act(.choosingSource(open: false)) }

    public func rejoin(_ room: RecentRoom) {
        quiet.remove(room.roomId)
        rejoining = room.roomId
        recent = rows()
        act(.rejoin(roomId: room.roomId))
    }

    /// Watch together's page opened: the rooms kept on this device.
    public func loadRecent() { send(.room(.loadRecent)) }

    /// Open the player at the room's place, from Watch together.
    public func openPlayer() {
        shown = true
        act(.openPlayer)
    }

    private func act(_ command: RoomCommand) {
        changedAt = .now
        send(.room(command))
    }

    /// Something in the room changed: the strip shows for 8 s.
    public func changed() { changedAt = .now }

    /// A screenshot's still moment: the room's clocks read it instead of the time, so a shot's
    /// seconds are the scene's. Only a Fixtures run sets it.
    public static var stillAt: Int64?

    // MARK: The engine

    func apply(_ event: Event) {
        switch event {
        case .roomJoined(let view):
            self.view = view
            invitation = nil
            peeked = nil
            joining = nil
            rejoining = nil
            joinFailed = nil
            joinFailure = nil
            joinText = ""
            changed()
        case .roomState(let view):
            self.view = view
            if case .found? = view.sources { choices = [] }
        case .roomMembers(let members):
            self.members = members
            changed()
        case .roomMessage:
            changed()
        case .roomChat(let conversation):
            lines = conversation.lines.compactMap(RoomLine.init)
        case .roomChatState(let state):
            chatHidden = state.hidden
        case .roomLeft(let why):
            left(why)
        case .roomStarted(let id, let episode, _, _):
            // The room plays: the player opens at its place, and the seat follows the room.
            shown = true
            send(.play(id: id, episode: episode))
        case .roomGlimpse(let glimpse):
            if let room = glimpse.roomId, room == lookingIn {
                lookingIn = nil
                return
            }
            guard view == nil, let by = glimpse.by else { return }
            let episode = glimpse.roomEpisode
            invitation = RoomInvitation(
                by: by, title: episode?.title ?? "", id: episode?.id, episode: episode?.episode ?? "",
                names: glimpse.members.map(\.name), full: glimpse.full, link: peeked)
            members = glimpse.members
            lines = []
        case .roomLookFailed(let roomId, _):
            quiet.insert(roomId)
            if rejoining == roomId { rejoining = nil }
            recent = rows()
        case .roomRecent(let rooms):
            #if FIXTURES
            if stagedRecent { return }
            #endif
            kept = rooms
            recent = rows()
        case .roomMoveOffer(let offer):
            self.offer = offer
            changed()
        case .roomMoveOfferGone(let id):
            if offer?.id == id { offer = nil }
        case .sources(let id, let episode, let candidates, _):
            if view?.episodeId == id, view?.episodeNumber == episode, you?.state == .choosing {
                choices = candidates
                chosen = chosen ?? candidates.first?.roomKey
            }
        default:
            break
        }
    }

    private func left(_ why: RoomLeftWhy) {
        let failed: RoomJoinFailure? = switch why {
        case .left, .moved: nil
        case .unreachable: .unreachable
        case .full: .full
        case .oldInvite: .oldInvite
        case .failed(let reason): .failed(reason.text)
        }
        if let rejoining, failed != nil {
            quiet.insert(rejoining)
        } else if let joining, let failed {
            joinFailed = joining
            joinFailure = failed
            joinText = joining
        }
        joining = nil
        rejoining = nil
        view = nil
        members = []
        lines = []
        offer = nil
        popover = nil
        leaving = nil
        moving = false
        writing = false
        recent = rows()
    }

    private func rows() -> [RecentRoom] {
        #if FIXTURES
        if stagedRecent { return recent }
        #endif
        return kept.map { room in
            RecentRoom(
                roomId: room.roomId, title: room.title, id: room.id, episode: room.episode, others: room.names,
                lastAt: RecentRoom.when(room.lastAt), quiet: quiet.contains(room.roomId))
        }
    }

    #if FIXTURES
    private var stagedRecent = false

    /// A screenshot run's join of `link`, which the fake room sends itself.
    public func stage(joining link: String) {
        joinText = link
        joining = link
    }

    /// A screenshot run's recent rooms and Join failure, for Watch together's page.
    public func stage(recent: [RecentRoom], joinFailed: String?) {
        stagedRecent = true
        self.recent = recent
        self.joinFailed = joinFailed
        joinFailure = joinFailed == nil ? nil : .unreachable
        if let joinFailed { joinText = joinFailed }
    }

    /// The player's skip mark, for a screenshot of Skip opening.
    public func stage(skipTo: Double?) { self.skipTo = skipTo }
    #endif
}
