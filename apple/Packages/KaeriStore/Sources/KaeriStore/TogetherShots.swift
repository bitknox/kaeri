// Watch together's states for screenshots (apple/scripts/shots.sh, together:together=<state>):
// the together scene's recent rooms, and a room the run brings as the engine would, event by
// event. Testing only: the release app has none of this.

#if FIXTURES
import KaeriKit
import KaeriStrings
import Foundation

/// A Watch together state a screenshot shows: `-KaeriTogether lobby-filling`.
public enum TogetherScene: String, CaseIterable, Sendable {
    /// The place with no rooms kept yet.
    case placeEmpty = "place-empty"
    /// The place with the scene's recent rooms.
    case placeRecent = "place-recent"
    /// A recent room opened: who is in it now.
    case placeLook = "place-look"
    /// The place while in a party: its block at the top.
    case placeParty = "place-party"
    /// Start a party the first time, with nothing picked: Choose a server.
    case pick
    /// Watch together on a title page the first time: Choose a server.
    case pickServer = "pick-server"
    /// An invite opened: looking in, before joining.
    case lobbyInvited = "lobby-invited"
    /// A party just started, nobody else in yet.
    case lobbyAlone = "lobby-alone"
    /// Mika ready, Ren joining, you not ready yet.
    case lobbyFilling = "lobby-filling"
    /// You and Mika ready, Ren not.
    case lobbyReady = "lobby-ready"
    /// Everyone ready: the count.
    case lobbyStarting = "lobby-starting"
    /// Whoever started the room left: anyone may start.
    case lobbyStarterLeft = "lobby-starter-left"
    /// None of your sources has the episode.
    case lobbyNosource = "lobby-nosource"
    /// The episode is not out yet.
    case lobbyOutlater = "lobby-outlater"
    /// The server does not answer.
    case lobbyUnreachable = "lobby-unreachable"
    /// The server went away: trying again.
    case lobbyLost = "lobby-lost"
    /// The room finished the last episode out.
    case lobbyLastout = "lobby-lastout"
    /// In a room that watches nothing yet.
    case lobbyNothing = "lobby-nothing"
    /// The link named episode 8; the room moved on to 9.
    case lobbyMovedon = "lobby-movedon"
    /// The room's title found by its name.
    case lobbyByname = "lobby-byname"
    /// The room's plain file matched to a title.
    case lobbyMatched = "lobby-matched"
    /// A plain file no title matched: the guesses.
    case lobbyUnsure = "lobby-unsure"
    /// A typed code the wrong shape.
    case joinShape = "join-shape"
    /// A code to a room with nobody in it.
    case codeEmpty = "code-empty"
    /// The lobby with a conversation under the start line.
    case lobbyChat = "lobby-chat"
    /// The same, a line being typed and the privacy line still up.
    case lobbyChatTyping = "lobby-chat-typing"
    /// In a party, elsewhere in the app: the bar over the tabs (iPhone).
    case bar
    /// The same bar while a line arrives: the line takes the count's place for a few seconds.
    case barChat = "bar-chat"

    public static let argument = "-KaeriTogether"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> TogetherScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1) else { return nil }
        return TogetherScene(rawValue: arguments[index + 1])
    }
}

extension PartyModel {
    /// Bring Watch together to `scene`, for a shot. False when the scene's rooms did not come.
    public func stage(_ scene: TogetherScene, store: AppStore) async -> Bool {
        loadTogether()
        guard await waited(10, { !self.recent.isEmpty && self.form != nil }) else { return false }
        let first = recent[0]
        let server = first.server
        let room = "kite-7qm4-xr9d" // i18n-skip
        let code = "KITE-7QM4" // i18n-skip
        let id = first.id
        let title = first.title
        let link = partyShareLink(share: PartyShare(server: server, room: room, by: "Mika", title: title, episode: "9"))

        func episode(_ found: PartyFound = .tag, id: AnilistId? = id, plainBy: String? = nil) -> PartyRoomEpisode {
            PartyRoomEpisode(id: id, episode: "9", title: title, file: "natsukaze-relay-09.mkv", // i18n-skip
                             found: found, plainBy: plainBy)
        }
        func member(_ name: String, ready: Bool?, you: Bool = false) -> PartyMember {
            PartyMember(name: name, ready: ready, file: ready == nil ? nil : "natsukaze-relay-09.mkv", // i18n-skip
                        duration: 1440, fileMatches: ready == nil ? nil : true, you: you)
        }
        func view(ready: Bool = false, roomEpisode: PartyRoomEpisode? = episode(), starter: String? = "Ada",
                  starterPresent: Bool = true, canStartAnyway: Bool = true, startingIn: UInt32? = nil,
                  sources: PartySources? = .found, guesses: [PartyGuess] = [],
                  connection: PartyConnection = .connected, playing: Bool = false) -> PartyView {
            PartyView(server: server, room: room, name: "Ada", connection: connection, encrypted: false,
                      ready: ready, episode: roomEpisode.flatMap { r in r.id.map { PartyEpisode(id: $0, episode: r.episode) } },
                      invite: link, motd: nil, latencyMs: 24, roomEpisode: roomEpisode, nextEpisode: nil,
                      starter: starter, starterPresent: starterPresent, canStartAnyway: canStartAnyway,
                      code: code, link: link, playing: playing, startingIn: startingIn, sources: sources,
                      guesses: guesses, looking: false, movesOn: true)
        }
        func inRoom(_ view: PartyView, _ members: [PartyMember]) {
            lobby = TogetherRoom(server: server, room: room)
            store.apply(.partyJoined(view))
            store.apply(.partyMembers(members))
            lobbyShown = true
        }
        func lookingIn(_ lobby: TogetherRoom, _ glimpse: PartyGlimpse?) {
            self.lobby = lobby
            looks[lobby.key] = glimpse.map { .found($0) } ?? .looking
            lobbyShown = true
        }
        let ada = member("Ada", ready: false, you: true)

        switch scene {
        case .placeEmpty:
            recent = []
        case .placeRecent:
            break
        case .placeLook:
            lookedAt = TogetherRoom.key(first.server, first.room)
            looks[lookedAt!] = .found(PartyGlimpse(server: first.server, room: first.room,
                                                  members: [member("Mika", ready: false)],
                                                  roomEpisode: episode(), nextEpisode: nil))
        case .placeParty, .bar, .barChat:
            // Joined from its lobby, as the place's own Start would: no "you're in" note.
            lobby = TogetherRoom(server: server, room: room)
            store.apply(.partyJoined(view()))
            store.apply(.partyMembers([member("Ada", ready: true, you: true), member("Mika", ready: true), member("Ren", ready: false)]))
            lobbyShown = false
            // The bar shows elsewhere: Home, with the place popped.
            if scene == .bar || scene == .barChat { placeShown = false }
            if scene == .barChat {
                // Late, so the bar is on screen when the line arrives and holds it for 4 s.
                try? await Task.sleep(for: .milliseconds(1200))
                store.party.chat.hidden = false
                store.apply(.partyChat(PartyConversation(lines: [
                    PartyChatLine(id: 1, from: "Mika", you: false, body: .said(text: "snacks ready?"), muted: false),
                ], marks: [], failed: nil)))
            }
        case .pick:
            pickServer = ""
            pickName = ""
            picking = .server(nil)
        case .pickServer:
            pickServer = ""
            pickName = ""
            picking = .server(PartyEpisode(id: id, episode: "9"))
        case .lobbyInvited:
            lookingIn(TogetherRoom(server: server, room: room, by: "Mika", title: title, episode: "9"),
                      PartyGlimpse(server: server, room: room,
                                   members: [member("Mika", ready: true), member("Ren", ready: false)],
                                   roomEpisode: episode(), nextEpisode: nil))
        case .lobbyAlone:
            inRoom(view(canStartAnyway: false), [ada])
        case .lobbyFilling:
            inRoom(view(), [ada, member("Mika", ready: true), member("Ren", ready: nil)])
        case .lobbyReady:
            inRoom(view(ready: true), [member("Ada", ready: true, you: true), member("Mika", ready: true), member("Ren", ready: false)])
        case .lobbyStarting:
            inRoom(view(ready: true, canStartAnyway: false, startingIn: 3),
                   [member("Ada", ready: true, you: true), member("Mika", ready: true), member("Ren", ready: true)])
        case .lobbyStarterLeft:
            inRoom(view(starter: "Mika", starterPresent: false), [ada, member("Ren", ready: true)])
            store.apply(.partyMessage(PartyMessage(kind: .starterLeft(name: "Mika"), text: L.partyStarterLeft(name: "Mika"))))
        case .lobbyNosource:
            inRoom(view(sources: .missing(why: .notFound, reason: .verbatim(text: L.partyLobbySourcesMissing))),
                   [ada, member("Mika", ready: true)])
        case .lobbyOutlater:
            inRoom(view(sources: .missing(why: .notAired, reason: .verbatim(text: L.partyLastOutAirs(episode: "9", day: "Saturday")))), // i18n-skip
                   [ada, member("Mika", ready: false)])
        case .lobbyUnreachable:
            let lobby = TogetherRoom(server: "party.example:8999", room: room, by: "Mika", title: title, episode: "9") // i18n-skip
            self.lobby = lobby
            looks[lobby.key] = .failed(L.partyServerUnreachable(server: lobby.server))
            lobbyShown = true
        case .lobbyLost:
            inRoom(view(connection: .reconnecting(attempt: 2, of: 5)), [ada, member("Mika", ready: true)])
        case .lobbyLastout:
            inRoom(view(canStartAnyway: false), [ada, member("Mika", ready: false)])
            lastOut = TogetherLastOut(episode: "9", airs: L.partyLastOutAirs(episode: "10", day: "Saturday")) // i18n-skip
        case .lobbyNothing:
            inRoom(view(roomEpisode: nil, canStartAnyway: false, sources: nil), [ada, member("Mika", ready: nil)])
        case .lobbyMovedon:
            lookingIn(TogetherRoom(server: server, room: room, by: "Mika", title: title, episode: "8"),
                      PartyGlimpse(server: server, room: room, members: [member("Mika", ready: false)],
                                   roomEpisode: episode(), nextEpisode: nil))
        case .lobbyByname:
            inRoom(view(roomEpisode: episode(.name)), [ada, member("Mika", ready: true)])
        case .lobbyMatched:
            inRoom(view(roomEpisode: episode(.fileName, plainBy: "Ren")), [ada, member("Ren", ready: true)])
        case .lobbyUnsure:
            let guesses = recent.prefix(3).map { PartyGuess(id: $0.id, title: $0.title, coverUrl: $0.coverUrl) }
            inRoom(view(roomEpisode: episode(.unmatched, id: nil, plainBy: "Ren"), canStartAnyway: false,
                        sources: nil, guesses: Array(guesses)), [ada, member("Ren", ready: true)])
        case .lobbyChat, .lobbyChatTyping:
            store.party.chat.hidden = false
            store.party.chat.forgetPrivacySeen()
            inRoom(view(ready: true), [member("Ada", ready: true, you: true), member("Mika", ready: true), member("Ren", ready: false)])
            func line(_ id: UInt64, _ from: String, _ text: String, you: Bool = false) -> PartyChatLine {
                PartyChatLine(id: id, from: from, you: you, body: .said(text: text), muted: false)
            }
            store.apply(.partyChatState(PartyChatState(allowed: true, maxLength: 150, muted: [], waitSeconds: 0, reactWaitSeconds: 0)))
            store.apply(.partyChat(PartyConversation(lines: [
                line(1, "Mika", "snacks ready?"),
                line(2, "Ren", "two minutes, making tea"),
                line(3, "Ada", "starting when Ren is back", you: true),
                line(4, "Sol", "the soundtrack is at music.example/ost"),
            ], marks: [], failed: nil)))
            if scene == .lobbyChatTyping { store.party.chat.draft = "who has the good cushion" }
        case .joinShape:
            joinProblem = L.partyCodeShape(example: "kite-7qm4-xr9d") // i18n-skip
        case .codeEmpty:
            lookingIn(TogetherRoom(server: server, room: "KITE-7QM4", fromCode: true), // i18n-skip
                      PartyGlimpse(server: server, room: "KITE-7QM4", members: [], roomEpisode: nil, nextEpisode: nil)) // i18n-skip
        }
        try? await Task.sleep(for: .milliseconds(900))
        return true
    }

    private func waited(_ seconds: Double, _ there: @escaping @MainActor () -> Bool) async -> Bool {
        let end = Date().addingTimeInterval(seconds)
        while Date() < end {
            if there() { return true }
            try? await Task.sleep(for: .milliseconds(100))
        }
        return there()
    }
}
#endif
