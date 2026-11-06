// Watch together, a place of its own (the watch together board, Direction A): Start a party and
// Join a party side by side, the party you are in, recent rooms and servers, then the lobby
// before play, pushed in the section. Mac and iPad lay it out the same; iPhone has its own
// (PhoneTogether). PartyModel keeps what the engine said. Member rows say a name and a state,
// never a source: each viewer plays from their own.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI
#if os(iOS)
import UIKit
#endif

// MARK: - Words

/// The lobby's words, the same on each platform.
@MainActor
enum TogetherWords {
    /// "Episode 9 · The Long Leg · 24 min", as far as this device knows the episode.
    static func episodeLine(_ episode: String, of id: AnilistId?, store: AppStore) -> String {
        var parts = [L.episodeNumberString(number: episode)]
        if let id, let row = store.titles.episodes[id]?.first(where: { $0.number == episode }), let title = row.title {
            parts.append(title)
        }
        if let id, let minutes = store.titles[id]?.duration, minutes > 0 {
            parts.append(L.durationMinutes(minutes: Int(minutes)))
        }
        return parts.joined(separator: " · ")
    }

    /// A member's state: Ready, Not ready, Joining, or Nothing open yet.
    static func state(_ member: PartyMember, roomHasEpisode: Bool) -> String {
        if member.ready == true { return L.partyMemberReady }
        if roomHasEpisode, member.file == nil, member.ready != nil { return L.partyMemberNoFile }
        if member.ready == nil { return L.partyMemberJoining }
        return L.partyMemberNotReady
    }

    /// The initials on a member's circle.
    static func initial(_ name: String) -> String {
        name.first.map { String($0).uppercased() } ?? "?"
    }

    /// The start line: the count, everyone ready, who the room waits for, or alone.
    static func startLine(_ view: PartyView, members: [PartyMember]?) -> String {
        if let seconds = view.startingIn { return L.partyStarting(seconds: Int(seconds)) }
        guard let members, members.count >= 2 else { return L.partyAlone }
        if PartyWords.allReady(members) { return L.partyAllReady }
        if members.allSatisfy({ $0.ready != true }) { return L.partyStartsLine }
        return PartyWords.readyLine(members)
    }

    /// "with Mika and Ren", from names already listed.
    static func names(_ names: [String]) -> String {
        names.formatted(.list(type: .and).locale(AirTime.locale))
    }

    /// "Tuesday", "18 October": when a recent room was last in.
    static func when(_ seconds: Int64, now: Date = .now) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let days = Calendar.current.dateComponents([.day], from: date, to: now).day ?? 0
        if Calendar.current.isDateInToday(date) { return date.formatted(date: .omitted, time: .shortened) }
        if days < 7 { return date.formatted(.dateTime.weekday(.wide).locale(AirTime.locale)) }
        return date.formatted(.dateTime.day().month(.wide).locale(AirTime.locale))
    }

    /// A recent room's line: "Episode 8 · with Mika and Ren · Tuesday".
    static func recentLine(_ room: PartyRecentRoom) -> String {
        if room.names.isEmpty {
            return [L.episodeNumberString(number: room.episode), when(room.lastAt)].joined(separator: " · ")
        }
        return L.togetherRecentRow(episode: room.episode, names: names(room.names), when: when(room.lastAt))
    }

    /// Who looking in found: "Mika is in the room", or "Nobody here now".
    static func presence(_ glimpse: PartyGlimpse) -> String {
        let others = glimpse.members.filter { !$0.you }.map(\.name)
        return others.isEmpty ? L.togetherNobodyHere : L.togetherSomeoneHere(names: names(others), count: others.count)
    }
}

// MARK: - Shared parts

/// A member: initials on a raised circle, the name, and the state as a word, with a check when
/// ready (never colour alone).
struct TogetherMemberRow: View {
    let member: PartyMember
    let roomHasEpisode: Bool
    var height: CGFloat = 44

    var body: some View {
        let state = TogetherWords.state(member, roomHasEpisode: roomHasEpisode)
        HStack(spacing: 12) {
            Text(TogetherWords.initial(member.name))
                .font(.board(12, weight: .semibold))
                .foregroundStyle(.silk)
                .frame(width: 28, height: 28)
                .background(Palette.raisedSelected, in: .circle)
            Text(member.you ? L.partyMemberYou(name: member.name) : member.name)
                .foregroundStyle(.silk)
                .font(.board(13))
                .lineLimit(1)
            Spacer(minLength: 8)
            HStack(spacing: 6) {
                if member.ready == true {
                    Image(systemName: "checkmark").font(.system(size: 11, weight: .semibold))
                }
                Text(state).contentTransition(.opacity)
            }
            .font(.board(13))
            .foregroundStyle(member.ready == true ? .silk : .haze)
            .animation(.quick, value: state)
        }
        .frame(minHeight: height)
        .overlay(alignment: .top) { Rectangle().fill(.faintRule).frame(height: 1) }
        .accessibilityElement(children: .combine)
    }
}

/// The room's still: the title's wide art, or its cover's colour.
struct TogetherStill: View {
    @Environment(AppStore.self) private var store
    @Environment(\.displayScale) private var scale
    let id: AnilistId?
    let width: CGFloat
    let height: CGFloat
    var radius: CGFloat = 10

    var body: some View {
        let entry = id.flatMap { store.titles[$0] }
        ArtworkView(url: entry?.bannerUrl ?? entry?.coverUrl, color: entry?.plateColor,
                    maxPixelSize: Int(width * scale))
            .frame(width: width, height: height)
            .clipShape(.rect(cornerRadius: radius))
            .accessibilityHidden(true)
            .task(id: id) {
                // The room's title may be one this device has not opened: its art and episodes.
                guard let id else { return }
                if store.titles[id] == nil { store.loadDetail(id) }
                if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
            }
    }
}

/// A recent room's poster, 36 × 54.
struct TogetherPoster: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId
    let url: String?
    var width: CGFloat = 36

    var body: some View {
        if let entry = store.titles[id] {
            CoverArt(entry: entry, width: width)
        } else {
            ArtworkView(url: url, maxPixelSize: Int(width * 3))
                .frame(width: width, height: width * 1.5)
                .clipShape(.rect(cornerRadius: 4))
                .task(id: id) { store.loadDetail(id) }
        }
    }
}

/// A line that shows only after 600 ms of waiting, where the answer will land.
struct LateLine: View {
    let text: String
    @State private var shown = false

    var body: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(text)
        }
        .font(.board(13))
        .foregroundStyle(.haze)
        .opacity(shown ? 1 : 0)
        .task {
            try? await Task.sleep(for: .milliseconds(600))
            withAnimation(.fadeIn) { shown = true }
        }
    }
}

/// The room's title block: who invited, "The room is watching", the title, the episode, and what
/// the room's state adds (moved on, matched from a file name, found by its name).
struct TogetherRoomBlock: View {
    @Environment(AppStore.self) private var store
    var titleSize: CGFloat = 24

    var body: some View {
        let party = store.party
        let room = party.lobbyEpisode
        let lobby = party.lobby
        VStack(alignment: .leading, spacing: 4) {
            if let kicker = kicker(lobby: lobby, room: room) {
                Text(kicker)
                    .font(.board(11, weight: .medium))
                    .tracking(0.88)
                    .textCase(.uppercase)
                    .foregroundStyle(.haze)
            }
            if let room {
                if room.found == .unmatched, let by = room.plainBy {
                    Text(L.partyRoomPlainFile(name: by, file: room.file))
                        .font(.board(15, weight: .semibold))
                        .foregroundStyle(.silk)
                } else {
                    Text(room.id.flatMap { store.titles[$0]?.showTitle } ?? room.title)
                        .font(.showTitle(size: titleSize))
                        .foregroundStyle(.silk)
                        .lineLimit(2)
                    Text(TogetherWords.episodeLine(room.episode, of: room.id, store: store))
                        .font(.board(15))
                        .foregroundStyle(.haze)
                        .monospacedDigit()
                }
                if let asked = lobby?.episode, asked != room.episode, lobby?.by != nil {
                    Text(L.partyRoomMovedOn(episode: room.episode)).font(.board(13)).foregroundStyle(.silk)
                }
                switch room.found {
                case .name:
                    Text(L.partyRoomFoundByName).font(.board(13)).foregroundStyle(.haze)
                case .fileName:
                    HStack(spacing: 10) {
                        if let by = room.plainBy {
                            Text(L.partyRoomPlainFile(name: by, file: room.file)).foregroundStyle(.haze)
                        }
                        Text(L.partyRoomMatched).foregroundStyle(.haze)
                        Button(L.partyRoomNotThis) { party.chooseTitle(nil) }
                            .buttonStyle(.borderless)
                            .foregroundStyle(.silk)
                    }
                    .font(.board(13))
                default:
                    EmptyView()
                }
            } else if let title = lobby?.title {
                // The link's words, until the room says its own.
                Text(title).font(.showTitle(size: titleSize)).foregroundStyle(.silk)
                if let episode = lobby?.episode {
                    Text(L.episodeNumberString(number: episode)).font(.board(15)).foregroundStyle(.haze)
                }
            }
        }
        .animation(.fadeIn, value: room)
        .accessibilityElement(children: .combine)
    }

    /// "Mika invited you · The room is watching"; only who invited while the room watches
    /// nothing, and nothing at all in your own room with nothing open.
    private func kicker(lobby: TogetherRoom?, room: PartyRoomEpisode?) -> String? {
        let watching = room != nil || lobby?.title != nil
        if let by = lobby?.by, !store.party.inLobbyRoom {
            return watching ? L.partyInvitedBy(name: by) + " · " + L.partyRoomWatching : L.partyInvitedBy(name: by)
        }
        return watching ? L.partyRoomWatching : nil
    }
}

/// The sources line: what this device's own sources found for the room's episode.
struct TogetherSourcesLine: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var showsActions = true

    var body: some View {
        let party = store.party
        Group {
            switch party.inLobbyRoom ? party.party?.sources : nil {
            case .found?:
                line(dot: .water, L.partyLobbySourcesFound)
            case .missing(let why, let reason)?:
                VStack(alignment: .leading, spacing: 8) {
                    switch why {
                    case .notAired:
                        line(dot: .water, airs(reason: reason))
                    case .noSources:
                        line(glyph: true, L.partyLobbySourcesMissing)
                    case .notFound, .noConnection:
                        line(glyph: true, L.partyLobbySourcesMissing)
                    }
                    if showsActions, why != .notAired {
                        HStack(spacing: 8) {
                            if why == .notFound {
                                Button(L.partyChooseSource) { party.chooseSourceFromLobby() }
                                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                            }
                            Button(L.partyOpenSources) { navigator.openProviders() }
                                .buttonStyle(SecondaryCapsuleStyle())
                        }
                    }
                }
            case .checking?:
                LateLine(text: L.partyLookingIn)
            case nil:
                if party.lobbyLook == .looking { LateLine(text: L.partyLookingIn) }
            }
        }
        .animation(.fadeIn, value: party.party?.sources)
    }

    private func airs(reason: Msg) -> String {
        reason.text
    }

    private func line(dot: Color? = nil, glyph: Bool = false, _ text: String) -> some View {
        HStack(spacing: 8) {
            if let dot { Circle().fill(dot).frame(width: 6, height: 6) }
            if glyph { Image(systemName: "exclamationmark.circle").foregroundStyle(.vermilion) }
            Text(text).foregroundStyle(.silk)
        }
        .font(.board(13))
    }
}

/// What the room can do next, under its title block: Join, I'm ready or Ready, and the room's
/// own states (nothing open, the last episode out).
struct TogetherActions: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var height: CGFloat = 34
    var fills = false
    var showsSecondary = true

    var body: some View {
        let party = store.party
        // Side by side while they fit; the secondary actions under the first when not.
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 10) {
                first
                second
            }
            VStack(alignment: .leading, spacing: 10) {
                first
                HStack(spacing: 10) { second }
            }
        }
        .animation(.quick, value: party.party?.ready)
    }

    @ViewBuilder
    private var first: some View {
        let party = store.party
        if !party.inLobbyRoom {
            Button { party.enterLobby() } label: { wide(Text(L.partyJoin)) }
                .buttonStyle(PrimaryCapsuleStyle(height: height))
                .disabled(party.entering || party.lobbyLook == .looking)
        } else if let view = party.party {
            // Picked as anything is played: browsing the app, where Play is the room's.
            if view.roomEpisode == nil, !view.looking {
                Button { navigator.browseForRoom(store: store) } label: { wide(Text(L.partyRoomPick)) }
                    .buttonStyle(PrimaryCapsuleStyle(height: height))
            } else if party.lastOut != nil {
                Button { navigator.browseForRoom(store: store) } label: { wide(Text(L.partyWatchSomethingElse)) }
                    .buttonStyle(PrimaryCapsuleStyle(height: height))
            } else if view.playing {
                Button { party.openPlayer() } label: { wide(Label(L.togetherOpenPlayer, systemImage: "play.fill")) }
                    .buttonStyle(PrimaryCapsuleStyle(height: height))
            } else if view.ready {
                Button { party.setReady(false) } label: {
                    wide(Label(L.partyReadyOn, systemImage: "checkmark"))
                }
                .buttonStyle(SecondaryCapsuleStyle(height: height))
                .sensoryFeedback(.impact(weight: .light), trigger: view.ready)
            } else if case .missing? = view.sources {
                EmptyView()
            } else if view.roomEpisode?.id == nil {
                // A plain file no title matched: which title it is comes first.
                EmptyView()
            } else {
                Button { party.setReady(true) } label: { wide(Text(L.partyReadyAction)) }
                    .buttonStyle(PrimaryCapsuleStyle(height: height))
            }
        }
    }

    @ViewBuilder
    private var second: some View {
        let party = store.party
        if party.inLobbyRoom, let view = party.party {
            if showsSecondary {
                Button(L.partyLeave) { party.leave() }
                    .buttonStyle(SecondaryCapsuleStyle(height: height))
                if view.roomEpisode != nil, !view.playing, party.lastOut == nil {
                    Button(L.partyChangeEpisode) { navigator.browseForRoom(store: store) }
                        .buttonStyle(SecondaryCapsuleStyle(height: height))
                }
            }
        }
    }

    /// The label across the width, for the iPhone's pinned button.
    private func wide(_ label: some View) -> some View {
        label.frame(maxWidth: fills ? .infinity : nil)
    }
}

/// "That was the last episode out", once the room finished it.
struct TogetherLastOutLine: View {
    let lastOut: TogetherLastOut

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(L.partyLastOut).foregroundStyle(.silk)
            if let airs = lastOut.airs { Text(airs).foregroundStyle(.haze) }
        }
        .font(.board(13))
    }
}

/// A guess for a plain file's title.
struct TogetherGuesses: View {
    @Environment(AppStore.self) private var store
    let guesses: [PartyGuess]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(L.partyRoomGuesses).font(.board(13, weight: .semibold)).foregroundStyle(.silk)
            ForEach(guesses, id: \.id) { guess in
                Button { store.party.chooseTitle(guess.id) } label: {
                    HStack(spacing: 12) {
                        TogetherPoster(id: guess.id, url: guess.coverUrl, width: 30)
                        Text(guess.title).font(.board(13)).foregroundStyle(.silk)
                        Spacer()
                    }
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
            }
        }
    }
}

/// The invite: the code at 24 pt mono, Copy invite link, Share…, and the room as text for
/// Syncplay's own app.
struct TogetherInvite: View {
    @Environment(AppStore.self) private var store
    let view: PartyView
    var codeSize: CGFloat = 24

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(L.partyInvite).font(.board(15, weight: .semibold)).foregroundStyle(.silk)
            Text(view.code ?? view.room)
                .font(.system(size: codeSize, weight: .medium, design: .monospaced))
                .foregroundStyle(.silk)
                .textSelection(.enabled)
                .accessibilityLabel(L.partyCode + ", " + (view.code ?? view.room))
            HStack(spacing: 8) {
                Button(L.partyCopyInvite, systemImage: "doc.on.doc") { copy() }
                    .buttonStyle(SecondaryCapsuleStyle())
                    .disabled(link == nil)
                if let link, let url = URL(string: link) {
                    ShareLink(item: url, message: Text(shareText)) {
                        Label(L.partyShare, systemImage: "square.and.arrow.up")
                    }
                    .buttonStyle(SecondaryCapsuleStyle())
                }
            }
            Text(L.partyOtherApp + ": " + L.partyOtherAppDetail(server: view.server, room: view.room))
                .font(.board(12))
                .foregroundStyle(.haze)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var link: String? { view.link ?? view.invite }

    private var shareText: String {
        guard let room = view.roomEpisode else { return "" }
        return L.partyShareText(title: room.title, episode: room.episode)
    }

    private func copy() {
        guard let link else { return }
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(link, forType: .string)
        #else
        UIPasteboard.general.string = link
        #endif
        store.say(L.partyInviteCopied)
    }
}

/// The start line, with Start anyway for whoever may.
struct TogetherStartLine: View {
    @Environment(AppStore.self) private var store
    let view: PartyView

    var body: some View {
        let party = store.party
        let line = TogetherWords.startLine(view, members: party.members)
        HStack(spacing: 12) {
            Text(line)
                .font(.board(13))
                .foregroundStyle(.haze)
                .monospacedDigit()
                .contentTransition(.numericText(countsDown: true))
                .animation(.linear(duration: 0.25), value: line)
            Spacer(minLength: 8)
            if view.canStartAnyway, view.startingIn == nil {
                Button(L.partyStartAnyway) { party.startAnyway() }
                    .buttonStyle(SecondaryCapsuleStyle())
                    .transition(.opacity)
            }
        }
        .animation(.fadeIn, value: view.canStartAnyway)
    }
}

/// The lobby's right column on Mac and iPad, and its list on iPhone: who is in the room, or
/// what looking in found, or why it could not.
struct TogetherMembers: View {
    @Environment(AppStore.self) private var store
    var rowHeight: CGFloat = 44

    var body: some View {
        let party = store.party
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(heading).font(.board(15, weight: .semibold)).foregroundStyle(.silk)
                    .monospacedDigit().contentTransition(.numericText())
                if let note = countNote {
                    Text(note).font(.board(13)).foregroundStyle(.haze).monospacedDigit()
                        .contentTransition(.numericText())
                }
            }
            .padding(.bottom, 8)
            if case .failed(let reason)? = party.lobbyLook {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: "exclamationmark.circle").foregroundStyle(.vermilion)
                    Text(reason).foregroundStyle(.silk).fixedSize(horizontal: false, vertical: true)
                }
                .font(.board(13))
                .padding(.vertical, 10)
                .overlay(alignment: .top) { Rectangle().fill(.faintRule).frame(height: 1) }
                if let lobby = party.lobby {
                    Button(L.actionTryAgain) { party.openLobby(lobby) }
                        .buttonStyle(SecondaryCapsuleStyle())
                        .padding(.top, 4)
                }
            } else if let members = party.lobbyMembers {
                ForEach(members, id: \.name) { member in
                    TogetherMemberRow(member: member, roomHasEpisode: party.lobbyEpisode != nil, height: rowHeight)
                        .transition(.opacity)
                }
                if !party.inLobbyRoom {
                    if members.isEmpty, let lobby = party.lobby, lobby.fromCode {
                        Text(L.partyCodeEmptyRoom(code: lobby.room)).font(.board(13)).foregroundStyle(.haze)
                            .padding(.top, 10)
                    }
                    Text(L.partyLookingInNote).font(.board(12)).foregroundStyle(.haze).padding(.top, 10)
                }
            } else {
                LateLine(text: L.partyLookingIn).frame(minHeight: rowHeight)
            }
            if party.inLobbyRoom, let view = party.party, case .reconnecting(let attempt, let of) = view.connection {
                Text(L.partyConnectionLost(attempt: Int(attempt), of: Int(of)))
                    .font(.board(13)).foregroundStyle(.haze).padding(.top, 10)
            }
        }
        .animation(.standard, value: party.lobbyMembers?.map(\.name))
    }

    /// "In the room · 1 of 3 ready" once in it; "In the room" while looking in.
    private var heading: String {
        let party = store.party
        guard party.inLobbyRoom, let members = party.lobbyMembers else { return L.partyInTheRoom }
        return L.partyLobbyCount(ready: members.filter { $0.ready == true }.count, count: members.count)
    }

    /// "2 here", while looking in.
    private var countNote: String? {
        let party = store.party
        guard !party.inLobbyRoom, let members = party.lobbyMembers, !members.isEmpty else { return nil }
        return L.partyHereCount(count: members.count)
    }
}

// MARK: - Mac and iPad

/// The Watch together section on Mac and on a wide iPad: the place, and the lobby over it. On
/// iPad the lobby is pushed on the tab's stack. The Mac's detail column holds no stack of its
/// own: one there takes the window's toolbar from the split view, search and all, and the two
/// can pass it back and forth until AppKit stops the app. So the lobby lies over the place as a
/// title page lies over its section, with Back in the toolbar (`TogetherLobbyBack`).
struct TogetherScreen: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        @Bindable var party = store.party
        Group {
            #if os(macOS)
            ZStack {
                TogetherPlace()
                    .accessibilityHidden(party.lobbyShown)
                if party.lobbyShown {
                    TogetherLobby()
                        .transition(.opacity)
                        .zIndex(1)
                }
            }
            .animation(.standard, value: party.lobbyShown)
            #else
            TogetherPlace()
                .navigationDestination(isPresented: $party.lobbyShown) { TogetherLobby() }
            #endif
        }
        .task { party.loadTogether() }
    }
}

/// The place: the line, the party you are in, Start and Join, recent rooms, servers.
struct TogetherPlace: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let party = store.party
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(L.togetherLine)
                    .font(.board(13))
                    .foregroundStyle(.haze)
                if party.inParty {
                    TogetherInParty().padding(.top, 28)
                }
                HStack(alignment: .top, spacing: 40) {
                    VStack(alignment: .leading, spacing: 12) {
                        Group {
                            if party.inParty {
                                Button(L.togetherStart, systemImage: "person.2") { party.beginStart() }
                                    .buttonStyle(SecondaryCapsuleStyle(height: 34))
                            } else {
                                Button(L.togetherStart, systemImage: "person.2") { party.beginStart() }
                                    .buttonStyle(PrimaryCapsuleStyle())
                            }
                        }
                        Text(L.togetherStartLine).font(.board(13)).foregroundStyle(.haze)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    TogetherJoinField()
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(.top, 28)
                recent.padding(.top, 44)
                servers.padding(.top, 36)
            }
            .frame(maxWidth: 992, alignment: .leading)
            .padding(EdgeInsets(top: 20, leading: 28, bottom: 32, trailing: 32))
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(.ground)
        .navigationTitle(L.togetherTitle)
    }

    @ViewBuilder
    private var recent: some View {
        let party = store.party
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(L.togetherRecent).font(.board(17, weight: .semibold)).foregroundStyle(.silk)
                if !party.recent.isEmpty {
                    Text(L.togetherRecentNote).font(.board(13)).foregroundStyle(.haze)
                }
            }
            .accessibilityAddTraits(.isHeader)
            .padding(.bottom, 10)
            if party.recent.isEmpty {
                Text(L.togetherRecentEmpty).font(.board(13)).foregroundStyle(.haze)
            } else {
                ForEach(party.recent, id: \.room) { room in
                    TogetherRecentRow(room: room, selected: party.lookedAt == TogetherRoom.key(room.server, room.room)) {
                        party.lookedAt = TogetherRoom.key(room.server, room.room)
                        party.look(server: room.server, room: room.room)
                    }
                }
            }
        }
        .animation(.standard, value: party.recent.map(\.room))
    }

    private var servers: some View {
        let form = store.party.form
        return VStack(alignment: .leading, spacing: 0) {
            Text(L.togetherServers).font(.board(17, weight: .semibold)).foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
                .padding(.bottom, 10)
            ForEach(form?.recentServers ?? [], id: \.self) { server in
                if server != form?.suggestedServer { serverRow(server, line: nil) }
            }
            if let form {
                serverRow(form.suggestedServer, line: L.togetherServerSuggested(operator: form.suggestedRunBy))
            }
        }
    }

    private func serverRow(_ server: String, line: String?) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(server).font(.board(13)).foregroundStyle(.silk)
            if let line { Text(line).font(.board(12)).foregroundStyle(.haze) }
        }
        .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
        .overlay(alignment: .top) { Rectangle().fill(.faintRule).frame(height: 1) }
    }
}

/// Join a party: the field, Paste and Join, and the line under them, which says what is wrong
/// with what was typed.
struct TogetherJoinField: View {
    @Environment(AppStore.self) private var store
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        let party = store.party
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                HStack(spacing: 8) {
                    Image(systemName: "link").font(.system(size: 12)).foregroundStyle(.haze)
                    TextField(L.togetherJoinField, text: $text)
                        .textFieldStyle(.plain)
                        .font(.board(13))
                        .focused($focused)
                        .onSubmit(join)
                        #if os(iOS)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        #endif
                }
                .padding(.horizontal, 10)
                .frame(height: 34)
                .background(.raised, in: .rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8)
                    .strokeBorder(party.joinProblem == nil ? Palette.faintRule : Palette.vermilion, lineWidth: 1))
                paste
                Button(L.togetherJoinAction) { join() }
                    .buttonStyle(SecondaryCapsuleStyle(height: 34))
                    .disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            Text(party.joinProblem ?? L.togetherJoinLine)
                .font(.board(13))
                .foregroundStyle(party.joinProblem == nil ? .haze : .silk)
                .fixedSize(horizontal: false, vertical: true)
                .animation(.fadeIn, value: party.joinProblem)
        }
        .onChange(of: text) { if party.joinProblem != nil { party.clearJoinProblem() } }
        .sensoryFeedback(.error, trigger: party.joinProblem) { _, now in now != nil }
    }

    @ViewBuilder
    private var paste: some View {
        #if os(macOS)
        Button(L.togetherPaste, systemImage: "doc.on.clipboard") {
            if let pasted = NSPasteboard.general.string(forType: .string) {
                text = pasted
                join()
            }
        }
        .buttonStyle(SecondaryCapsuleStyle(height: 34))
        #else
        PasteButton(payloadType: String.self) { strings in
            guard let pasted = strings.first else { return }
            Task { @MainActor in
                text = pasted
                join()
            }
        }
        .labelStyle(.titleAndIcon)
        .buttonBorderShape(.capsule)
        .tint(Palette.raised)
        #endif
    }

    private func join() {
        if store.party.join(text: text) { text = "" } else { focused = true }
    }
}

/// A recent room: poster, title, line, and what looking in found, with Rejoin.
struct TogetherRecentRow: View {
    @Environment(AppStore.self) private var store
    let room: PartyRecentRoom
    let selected: Bool
    let select: () -> Void

    var body: some View {
        let party = store.party
        let look = party.looks[TogetherRoom.key(room.server, room.room)]
        HStack(spacing: 14) {
            TogetherPoster(id: room.id, url: room.coverUrl)
            VStack(alignment: .leading, spacing: 2) {
                Text(store.titles[room.id]?.showTitle ?? room.title)
                    .font(.showTitle(size: 15))
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                Text(TogetherWords.recentLine(room) + " · " + room.server)
                    .font(.board(13))
                    .foregroundStyle(.haze)
                    .lineLimit(1)
            }
            Spacer(minLength: 12)
            if selected {
                Group {
                    switch look {
                    case .looking?: LateLine(text: L.partyLookingIn)
                    case .found(let glimpse)?:
                        HStack(spacing: 6) {
                            if glimpse.members.contains(where: { !$0.you }) {
                                Circle().fill(.water).frame(width: 6, height: 6)
                            }
                            Text(TogetherWords.presence(glimpse))
                        }
                        .font(.board(13))
                        .foregroundStyle(.silk)
                    case .failed?: Text(L.togetherNobodyHere).font(.board(13)).foregroundStyle(.haze)
                    case nil: EmptyView()
                    }
                }
                .transition(.opacity)
            }
            Button(L.togetherRejoin) { party.rejoin(room) }
                .buttonStyle(SecondaryCapsuleStyle())
        }
        .padding(.horizontal, selected ? 10 : 0)
        .frame(minHeight: 70)
        .background(selected ? Palette.raisedSelected : .clear, in: .rect(cornerRadius: 8))
        .padding(.horizontal, selected ? -10 : 0)
        .overlay(alignment: .top) { if !selected { Rectangle().fill(.faintRule).frame(height: 1) } }
        .contentShape(.rect)
        .onTapGesture(perform: select)
        .animation(.fadeIn, value: look)
        .contextMenu {
            Button(L.togetherRejoin) { party.rejoin(room) }
            Button(L.partyCopyInvite) { copy(party.link(for: room)) }
            Divider()
            Button(L.togetherForgetRoom, role: .destructive) { party.forget(room) }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAction(named: L.togetherRejoin) { party.rejoin(room) }
    }

    private func copy(_ link: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(link, forType: .string)
        #else
        UIPasteboard.general.string = link
        #endif
        store.say(L.partyInviteCopied)
    }
}

/// The party you are in, at the top of the place: the still, the title, the room's count, Open
/// player or Open lobby in amber, and Leave watch party.
struct TogetherInParty: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let party = store.party
        if let view = party.party {
            let room = view.roomEpisode
            let members = party.members ?? []
            HStack(alignment: .top, spacing: 18) {
                TogetherStill(id: room?.id, width: 160, height: 90, radius: 8)
                VStack(alignment: .leading, spacing: 4) {
                    Text(room.map { r in r.id.flatMap { store.titles[$0]?.showTitle } ?? r.title } ?? L.partyRoomNothing)
                        .font(.showTitle(size: 20))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    Text(line(view, members: members))
                        .font(.board(13))
                        .foregroundStyle(.haze)
                        .monospacedDigit()
                        .contentTransition(.numericText())
                    HStack(spacing: 4) {
                        ForEach(members, id: \.name) { member in
                            Text(TogetherWords.initial(member.name))
                                .font(.board(10, weight: .semibold))
                                .frame(width: 20, height: 20)
                                .background(Palette.raisedSelected, in: .circle)
                        }
                    }
                    .foregroundStyle(.silk)
                    .padding(.top, 4)
                    .accessibilityHidden(true)
                    HStack(spacing: 10) {
                        if view.playing {
                            Button(L.togetherOpenPlayer, systemImage: "play.fill") { party.openPlayer() }
                                .buttonStyle(PrimaryCapsuleStyle())
                        } else {
                            Button(L.togetherOpenLobby) { party.openLobby(TogetherRoom(server: view.server, room: view.room)) }
                                .buttonStyle(PrimaryCapsuleStyle())
                        }
                        Button(L.partyLeave) { party.leave() }
                            .buttonStyle(SecondaryCapsuleStyle(height: 34))
                    }
                    .padding(.top, 10)
                }
            }
            .accessibilityElement(children: .contain)
        }
    }

    private func line(_ view: PartyView, members: [PartyMember]) -> String {
        if view.playing, !store.party.playerUp() { return L.partyBarIdle }
        guard let room = view.roomEpisode else { return L.partyRoomNothingLine }
        return L.togetherInPartyLine(episode: room.episode, count: members.count,
                                     ready: members.filter { $0.ready == true }.count)
    }
}

/// The lobby on Mac and iPad: what plays on the left, who is in the room and the invite on the
/// right.
struct TogetherLobby: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    private static let chatPlace = "chat"

    var body: some View {
        let party = store.party
        ScrollViewReader { reader in
            ScrollView {
                let columns = sizeClass == .compact
                    ? AnyLayout(VStackLayout(alignment: .leading, spacing: 32))
                    : AnyLayout(HStackLayout(alignment: .top, spacing: 40))
                columns {
                    VStack(alignment: .leading, spacing: 0) {
                        GeometryReader { proxy in
                            TogetherStill(id: party.lobbyEpisode?.id, width: proxy.size.width,
                                          height: proxy.size.width * 9 / 16)
                        }
                        .aspectRatio(16 / 9, contentMode: .fit)
                        .frame(maxWidth: 560)
                        TogetherRoomBlock().padding(.top, 18)
                        if party.inLobbyRoom, party.party?.roomEpisode == nil, party.party?.looking == false {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(L.partyRoomNothing).foregroundStyle(.silk)
                                Text(L.partyRoomNothingLine).foregroundStyle(.haze)
                            }
                            .font(.board(13))
                            .padding(.top, 4)
                        }
                        if let guesses = party.party?.guesses, !guesses.isEmpty, party.inLobbyRoom {
                            TogetherGuesses(guesses: guesses).padding(.top, 14)
                        }
                        if let lastOut = party.lastOut, party.inLobbyRoom {
                            TogetherLastOutLine(lastOut: lastOut).padding(.top, 12)
                        }
                        TogetherSourcesLine().padding(.top, 12)
                        TogetherActions().padding(.top, 18)
                    }
                    .frame(maxWidth: 560, alignment: .leading)
                    VStack(alignment: .leading, spacing: 0) {
                        TogetherMembers()
                        if party.inLobbyRoom, let view = party.party {
                            TogetherInvite(view: view).padding(.top, 28)
                            if view.roomEpisode?.id != nil, party.lastOut == nil, !view.playing {
                                Rectangle().fill(.faintRule).frame(height: 1).padding(.top, 20)
                                TogetherStartLine(view: view).padding(.top, 12)
                            }
                            TogetherChat().id(Self.chatPlace).padding(.top, 24)
                        }
                    }
                    .frame(maxWidth: 420, alignment: .leading)
                }
                .padding(EdgeInsets(top: 20, leading: 28, bottom: 32, trailing: 32))
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            #if os(iOS)
            // On iPad the chat's field and the privacy line under it stay above the keyboard.
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardDidShowNotification)) { _ in
                withAnimation(.standard) { reader.scrollTo(Self.chatPlace, anchor: .bottom) }
            }
            #endif
        }
        .background(.ground)
        #if os(iOS)
        .navigationTitle(L.togetherTitle)
        #endif
        .onDisappear { if !party.lobbyShown { party.closeLobby() } }
    }
}

#if os(macOS)
/// Back from the lobby on the Mac, in the window's toolbar where a pushed page's would be.
struct TogetherLobbyBack: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        Button(L.actionBack, systemImage: "chevron.left") { store.party.lobbyShown = false }
            .keyboardShortcut("[", modifiers: .command)
    }
}
#endif

// MARK: - The start sheet

/// The server and your name, the first time a party starts. Start a party brings nothing: in the
/// room, Play on any episode picks it for the room. Watch together on a title page brings its
/// episode.
struct TogetherStartSheet: View {
    @Environment(AppStore.self) private var store
    let step: TogetherPick

    var body: some View {
        let party = store.party
        VStack(alignment: .leading, spacing: 0) {
            server
            Spacer(minLength: 0)
            Divider()
            HStack(spacing: 8) {
                Spacer()
                Button(L.actionCancel) { party.cancelStart() }
                    .keyboardShortcut(.cancelAction)
                    .buttonStyle(SecondaryCapsuleStyle())
                Button(L.partyStartParty) { party.startOnServer() }
                    .keyboardShortcut(.defaultAction)
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .disabled(!party.canStartOnServer)
            }
            .padding(16)
        }
        #if os(macOS)
        .frame(width: 560, height: 520)
        #endif
        .background(.raised)
    }

    /// Nothing picked yet: the room's episode comes from a Play once in it.
    private var picksLater: Bool {
        switch step {
        case .server(let pick): pick == nil
        }
    }

    private var server: some View {
        @Bindable var party = store.party
        let form = party.form
        return VStack(alignment: .leading, spacing: 0) {
            Text(L.partyFirstServerTitle).font(.board(15, weight: .semibold)).foregroundStyle(.silk)
            if picksLater {
                Text(L.partyFormPickLater).font(.board(13)).foregroundStyle(.silk)
                    .fixedSize(horizontal: false, vertical: true).padding(.top, 6)
            }
            Text(L.partyPrivacy).font(.board(13)).foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true).padding(.top, 6)
            Text(L.partyServer).font(.board(12, weight: .semibold)).foregroundStyle(.silk).padding(.top, 20)
            TextField(L.partyServerHint, text: $party.pickServer)
                .textFieldStyle(.roundedBorder)
                .font(.system(size: 13, design: .monospaced))
                .padding(.top, 6)
                #if os(iOS)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .keyboardType(.URL)
                #endif
            if let form {
                HStack(alignment: .top, spacing: 12) {
                    Text(L.partyServerSuggest(server: form.suggestedServer, operator: form.suggestedRunBy))
                        .font(.board(12)).foregroundStyle(.haze).fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 8)
                    Button(L.partyUseIt) { party.pickServer = form.suggestedServer }
                        .buttonStyle(SecondaryCapsuleStyle())
                }
                .padding(.top, 8)
            }
            if let problem = PartyWords.serverProblem(party.pickServer), !party.pickServer.isEmpty {
                Text(problem).font(.board(12)).foregroundStyle(.silk).padding(.top, 4)
            }
            Text(L.partyYourName).font(.board(12, weight: .semibold)).foregroundStyle(.silk).padding(.top, 18)
            TextField(L.partyYourName, text: $party.pickName)
                .textFieldStyle(.roundedBorder)
                .font(.board(13))
                .padding(.top, 6)
                .onSubmit { party.startOnServer() }
        }
        .padding(22)
    }
}


// MARK: - Chat in the lobby

/// Talk before play: the conversation, the field, and the privacy line once.
struct TogetherChat: View {
    @Environment(AppStore.self) private var store
    /// The lines' size on the board: 14 where the Mac and iPad share the lobby, 15 on iPhone.
    var size: CGFloat = 14
    @FocusState private var focus: Bool

    var body: some View {
        let chat = store.party.chat
        VStack(alignment: .leading, spacing: 10) {
            Rectangle().fill(.faintRule).frame(height: 1)
            Text(L.partyChatTitle).font(.board(13, weight: .semibold)).foregroundStyle(.haze)
            if chat.hidden {
                Text(L.partyChatHidden).font(.board(13)).foregroundStyle(.silk)
                Text(L.partyChatHiddenLine).font(.board(12)).foregroundStyle(.haze)
                Button(L.partyChatShow) { chat.show() }.buttonStyle(SecondaryCapsuleStyle())
            } else if !chat.allowed {
                Text(L.partyChatOff).font(.board(13)).foregroundStyle(.haze)
            } else {
                let lines = chat.rows.map(ChatLineData.init)
                if !lines.isEmpty {
                    ChatConversation(lines, size: size, spacing: 9, maxHeight: 260, font: .board(size))
                }
                if chat.failed != nil {
                    HStack(spacing: 4) {
                        Text(L.partyChatNotSent)
                        Text("·")
                        Button(L.partyChatRetry) { chat.retry() }.buttonStyle(.plain).underline().foregroundStyle(.silk)
                    }
                    .font(.board(13)).foregroundStyle(.haze)
                }
                ForEach(chat.state?.muted ?? [], id: \.self) { name in
                    HStack(spacing: 4) {
                        Text(L.partyMutedLine(name: name))
                        Text("·")
                        Button(L.partyUnmuteShort) { chat.mute(name, false) }.buttonStyle(.plain).underline().foregroundStyle(.silk)
                    }
                    .font(.board(13)).foregroundStyle(.haze)
                }
                HStack(spacing: 8) {
                    ChatTextField(
                        text: Binding(get: { chat.draft }, set: { chat.draft = $0 }),
                        placeholder: L.partyChatField, style: .roomy, limit: chat.maxLength,
                        remaining: chat.remaining,
                        waitLabel: chat.wait > 0 ? L.partyChatWait(seconds: chat.wait) : nil,
                        waitSpoken: chat.wait > 0 ? L.partyChatWaitSpoken(seconds: chat.wait) : nil,
                        focus: $focus
                    ) { chat.submit() }
                    #if os(iOS)
                    Button { chat.submit() } label: {
                        Image(systemName: "arrow.up")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(chat.canSend ? Palette.silk : Palette.haze)
                            .frame(width: 40, height: 40)
                            .background(chat.canSend ? Palette.raisedSelected : Palette.raised, in: .circle)
                    }
                    .buttonStyle(.plain)
                    .disabled(!chat.canSend)
                    .accessibilityLabel(L.partyChatSend)
                    #endif
                }
                if !chat.privacySeen {
                    Text(L.partyChatPrivacy).font(.board(12)).foregroundStyle(.haze)
                }
            }
        }
        .animation(.standard, value: chat.hidden)
        #if FIXTURES
        .task {
            // The typing shot: the field focused, so the keyboard comes up under it.
            guard TogetherScene.named() == .lobbyChatTyping else { return }
            try? await Task.sleep(for: .milliseconds(600))
            focus = true
        }
        #endif
    }
}

extension ChatLineData {
    init(_ row: ChatRow) {
        switch row.kind {
        case .said(let from, let text, let you):
            self.init(id: row.id, name: you ? L.partyChatYou : from, text: text, you: you)
        case .reacted(let text):
            self.init(id: row.id, name: nil, text: text)
        }
    }
}
