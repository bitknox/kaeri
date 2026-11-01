// Watch together, the place of its own: recent rooms, looking into one, the lobby before play, and
// the sheet that starts a party. The engine keeps the rooms and the room's playlist; this keeps
// what it said for the screens. Every viewer plays from their own sources.

import KaeriKit
import KaeriStrings
import Foundation

/// A room the lobby is for: where it is, and what the link that opened it said, until the room
/// says its own.
public struct TogetherRoom: Equatable, Sendable {
    public var server: String
    public var room: String
    /// Who invited, from a link: "Mika invited you".
    public var by: String?
    /// The title and episode the link named, shown only until the room answers.
    public var title: String?
    public var episode: String?
    /// It was a code, typed or read out.
    public var fromCode = false

    public init(server: String, room: String, by: String? = nil, title: String? = nil, episode: String? = nil, fromCode: Bool = false) {
        self.server = server
        self.room = room
        self.by = by
        self.title = title
        self.episode = episode
        self.fromCode = fromCode
    }

    init(_ target: PartyTarget) {
        self.init(server: target.server, room: target.room, by: target.by, title: target.title,
                  episode: target.episode, fromCode: target.fromCode)
    }

    public var key: String { Self.key(server, room) }

    public static func key(_ server: String, _ room: String) -> String {
        server.lowercased() + "/" + room
    }
}

/// What looking into a room found.
public enum TogetherLook: Equatable, Sendable {
    case looking
    case found(PartyGlimpse)
    case failed(String)
}

/// The start sheet's step.
public enum TogetherPick: Equatable, Sendable, Identifiable {
    /// What are you watching?
    case title
    /// The first time only: the server and the name, for this pick.
    case server(PartyEpisode)

    public var id: String {
        switch self {
        case .title: "title" // i18n-skip
        case .server(let pick): "server \(pick.id) \(pick.episode)"
        }
    }
}

/// The room finished the last episode out.
public struct TogetherLastOut: Equatable, Sendable {
    public var episode: String
    /// "Episode 10 is out on Saturday", when the schedule says.
    public var airs: String?
}

extension PartyModel {
    // MARK: The place

    /// The place opened: the rooms kept, and the form's servers for Start.
    public func loadTogether() {
        send(.party(.loadRecent))
        if form == nil { send(.loadPartyForm) }
    }

    /// Look into a recent room: who is there now. Only for the room opened (decision 13).
    public func look(server: String, room: String) {
        let key = TogetherRoom.key(server, room)
        if case .found? = looks[key] { return }
        looks[key] = .looking
        send(.party(.lookIn(server: server, room: room, name: form?.name)))
    }

    public func forget(_ room: PartyRecentRoom) {
        recent.removeAll { $0.server == room.server && $0.room == room.room }
        send(.party(.forgetRoom(server: room.server, room: room.room)))
    }

    /// The https invite for a recent room, for Copy invite link.
    public func link(for room: PartyRecentRoom) -> String {
        partyShareLink(share: PartyShare(server: room.server, room: room.room, by: form?.name,
                                         title: room.title, episode: room.episode))
    }

    /// Join with what was typed or pasted: a link or a code opens its lobby. False, with
    /// `joinProblem` set, when it is neither.
    @discardableResult
    public func join(text: String) -> Bool {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return false }
        guard let target = parsePartyTarget(text: trimmed) else {
            let looksLikeLink = trimmed.contains("://") || trimmed.contains("#")
            joinProblem = looksLikeLink ? L.partyLinkIncomplete : L.partyCodeShape(example: "kite-7qm4-xr9d") // i18n-skip
            return false
        }
        joinProblem = nil
        openLobby(TogetherRoom(target))
        return true
    }

    public func clearJoinProblem() {
        joinProblem = nil
    }

    /// An invite opened from outside (`kaeri://party#…`, or an older link): its lobby, looking
    /// in. The room this device is in already opens its lobby as it is.
    public func openLink(_ link: String) {
        guard let target = parsePartyTarget(text: link) else {
            say(L.partyLinkIncomplete, L.partyLinkIncomplete, .alert)
            return
        }
        openLobby(TogetherRoom(target))
    }

    /// The lobby for `room`: looking in until this device joins it.
    public func openLobby(_ room: TogetherRoom) {
        if let party, TogetherRoom.key(party.server, party.room) == room.key {
            lobbyShown = true
            return
        }
        lobby = room
        looks[room.key] = .looking
        lobbyShown = true
        send(.party(.lookIn(server: room.server, room: room.room, name: form?.name)))
        if form == nil { send(.loadPartyForm) }
    }

    /// Rejoin a recent room: its lobby.
    public func rejoin(_ room: PartyRecentRoom) {
        openLobby(TogetherRoom(server: room.server, room: room.room, title: room.title, episode: room.episode))
    }

    /// The lobby on screen is this device's room.
    public var inLobbyRoom: Bool {
        guard let party, let lobby else { return false }
        return TogetherRoom.key(party.server, party.room) == lobby.key
    }

    /// Join watch party, in the lobby: into the room, which says what it watches.
    public func enterLobby() {
        guard let lobby, !entering else { return }
        if party != nil, !inLobbyRoom { send(.partyLeave) }
        entering = true
        send(.party(.lobby(server: lobby.server, room: lobby.room, name: form?.name, pick: nil)))
    }

    /// The lobby went away (Back): looking in stops showing. The room this device is in stays.
    public func closeLobby() {
        lobbyShown = false
        if !inLobbyRoom { lobby = party.map { TogetherRoom(server: $0.server, room: $0.room) } }
    }

    // MARK: Start

    /// Start a party: What are you watching?
    public func beginStart() {
        if form == nil { send(.loadPartyForm) }
        picking = .title
    }

    /// Watch together on a title page, or a pick in the sheet: the server the first time, then
    /// the lobby.
    public func start(_ pick: PartyEpisode) {
        if form == nil { send(.loadPartyForm) }
        if let server = form?.recentServers.first, !server.isEmpty, let form, !form.name.isEmpty {
            picking = nil
            startLobby(server: server, name: form.name, pick: pick)
        } else {
            pickServer = form?.recentServers.first ?? ""
            pickName = form?.name ?? ""
            picking = .server(pick)
        }
    }

    /// Start party, on the server step.
    public func startOnServer() {
        guard case .server(let pick)? = picking, canStartOnServer else { return }
        picking = nil
        startLobby(server: pickServer.trimmingCharacters(in: .whitespaces),
                   name: pickName.trimmingCharacters(in: .whitespaces), pick: pick)
    }

    public var canStartOnServer: Bool {
        let server = pickServer.trimmingCharacters(in: .whitespaces)
        return !server.isEmpty && PartyWords.serverProblem(server) == nil
            && !pickName.trimmingCharacters(in: .whitespaces).isEmpty
    }

    /// Back, on the server step.
    public func backToTitles() {
        picking = .title
    }

    public func cancelStart() {
        picking = nil
    }

    private func startLobby(server: String, name: String, pick: PartyEpisode) {
        if party != nil { send(.partyLeave) }
        let room = form?.room ?? roomFallback()
        lobby = TogetherRoom(server: server, room: room)
        looks[lobby!.key] = nil
        lobbyShown = true
        entering = true
        send(.party(.lobby(server: server, room: room, name: name, pick: pick)))
    }

    private func roomFallback() -> String {
        let letters = Array("abcdefghjkmnpqrstuvwxyz23456789")
        let key = String((0..<8).map { _ in letters.randomElement()! })
        return "room-" + key.prefix(4) + "-" + key.suffix(4) // i18n-skip
    }

    // MARK: In the lobby

    public func startAnyway() {
        send(.party(.startAnyway))
    }

    public func openPlayer() {
        send(.party(.openPlayer))
    }

    /// Change episode, Pick what to watch and Watch something else: the room's playlist, for
    /// everyone.
    public func changeEpisode(_ pick: PartyEpisode) {
        picking = nil
        lastOut = nil
        send(.party(.changeEpisode(id: pick.id, episode: pick.episode)))
    }

    /// A guess for a plain file, or Not this title (`nil`).
    public func chooseTitle(_ id: AnilistId?) {
        send(.party(.chooseTitle(id: id)))
    }

    /// Choose source, from the lobby's sources line: the party's source sheet.
    public func chooseSourceFromLobby() {
        guard let view = party, let room = view.roomEpisode, let id = room.id,
              case .missing(let why, let reason)? = view.sources else { return }
        noSource = PartyNoSourceAsk(id: id, episode: room.episode, why: why, reason: reason)
        chooseSource()
    }

    /// Change episode and Watch something else open the start sheet on its titles, and its pick
    /// changes the room's episode instead of starting a party.
    public var changingEpisode: Bool { inLobbyRoom && picking != nil }

    /// What the lobby says plays: the room's own, or what looking in found, or the link's words.
    public var lobbyEpisode: PartyRoomEpisode? {
        if inLobbyRoom { return party?.roomEpisode }
        guard let lobby, case .found(let glimpse)? = looks[lobby.key] else { return nil }
        return glimpse.roomEpisode
    }

    /// Who the lobby lists: the room's members once in it, or those looking in found.
    public var lobbyMembers: [PartyMember]? {
        if inLobbyRoom { return members }
        guard let lobby, case .found(let glimpse)? = looks[lobby.key] else { return nil }
        return glimpse.members
    }

    /// The lobby's look into the room, before joining.
    public var lobbyLook: TogetherLook? {
        guard let lobby, !inLobbyRoom else { return nil }
        return looks[lobby.key]
    }

    #if FIXTURES
    /// A screenshot run's state, as the engine would bring it.
    public func rehearse(lobby: TogetherRoom?, shown: Bool) {
        self.lobby = lobby
        lobbyShown = shown
    }

    public func rehearse(picking: TogetherPick?) {
        self.picking = picking
    }

    public func rehearse(joinProblem: String?) {
        self.joinProblem = joinProblem
    }
    #endif
}
