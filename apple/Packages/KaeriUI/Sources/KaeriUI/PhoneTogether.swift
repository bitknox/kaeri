// Watch together on iPhone: a people button on Home pushes the place; the lobby is pushed
// over it with the tab bar hidden, I'm ready pinned low; the party you are in rides over the tab
// bar while you are elsewhere (the watch together board, iPhone).

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI
import UIKit

/// The people button in Home's toolbar, before the account button.
struct TogetherButton: ToolbarContent {
    @Environment(AppStore.self) private var store

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Button(L.navWatchTogether, systemImage: store.party.inParty ? "person.2.fill" : "person.2") {
                store.party.placeShown = true
            }
        }
    }
}

/// The place on iPhone: the line, Start a party across the width, the party you are in, Join a
/// party, and recent rooms.
struct PhoneTogetherPlace: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        @Bindable var party = store.party
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(L.togetherLine).font(.board(15)).foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
                if party.inParty {
                    PhoneInParty().padding(.top, 20)
                }
                Group {
                    if party.inParty {
                        Button { party.beginStart() } label: {
                            Label(L.togetherStart, systemImage: "person.2").frame(maxWidth: .infinity)
                        }
                        .buttonStyle(SecondaryCapsuleStyle(height: 50))
                    } else {
                        Button { party.beginStart() } label: {
                            Label(L.togetherStart, systemImage: "person.2").frame(maxWidth: .infinity)
                        }
                        .buttonStyle(PrimaryCapsuleStyle(height: 50))
                    }
                }
                .padding(.top, 20)
                Text(L.togetherStartLine).font(.board(13)).foregroundStyle(.haze).padding(.top, 8)
                Text(L.togetherJoin).font(.board(17, weight: .semibold)).foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader)
                    .padding(.top, 32).padding(.bottom, 10)
                TogetherJoinField()
                recent.padding(.top, 32)
            }
            .padding(EdgeInsets(top: 8, leading: 16, bottom: 32, trailing: 16))
        }
        .background(.ground)
        .navigationTitle(L.togetherTitle)
        .navigationBarTitleDisplayMode(.large)
        .navigationDestination(isPresented: $party.lobbyShown) { PhoneTogetherLobby() }
        .task { party.loadTogether() }
    }

    private var recent: some View {
        let party = store.party
        return VStack(alignment: .leading, spacing: 0) {
            Text(L.togetherRecent).font(.board(17, weight: .semibold)).foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            Text(party.recent.isEmpty ? L.togetherRecentEmpty : L.togetherRecentNote)
                .font(.board(13)).foregroundStyle(.haze).padding(.top, 2).padding(.bottom, 8)
            ForEach(party.recent, id: \.room) { room in
                Button { party.rejoin(room) } label: {
                    HStack(spacing: 12) {
                        TogetherPoster(id: room.id, url: room.coverUrl)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(store.titles[room.id]?.showTitle ?? room.title)
                                .font(.showTitle(size: 15)).foregroundStyle(.silk).lineLimit(1)
                            Text(TogetherWords.recentLine(room))
                                .font(.board(13)).foregroundStyle(.haze).lineLimit(2)
                        }
                        Spacer(minLength: 8)
                        Image(systemName: "chevron.right").font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(.haze)
                    }
                    .frame(minHeight: 70)
                    .overlay(alignment: .top) { Rectangle().fill(.faintRule).frame(height: 1) }
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .contextMenu {
                    Button(L.togetherForgetRoom, systemImage: "trash", role: .destructive) { party.forget(room) }
                }
            }
        }
        .animation(.standard, value: party.recent.map(\.room))
    }
}

/// The party you are in, on the iPhone place.
struct PhoneInParty: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let party = store.party
        if let view = party.party {
            let members = party.members ?? []
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) {
                    TogetherStill(id: view.roomEpisode?.id, width: 96, height: 54, radius: 6)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(view.roomEpisode.map { r in r.id.flatMap { store.titles[$0]?.showTitle } ?? r.title } ?? L.partyRoomNothing)
                            .font(.showTitle(size: 17)).foregroundStyle(.silk).lineLimit(1)
                        Text(view.roomEpisode.map {
                            L.togetherInPartyLine(episode: $0.episode, count: members.count,
                                                  ready: members.filter { $0.ready == true }.count)
                        } ?? L.partyRoomNothingLine)
                            .font(.board(13)).foregroundStyle(.haze).monospacedDigit()
                    }
                }
                HStack(spacing: 10) {
                    if view.playing {
                        Button(L.togetherOpenPlayer, systemImage: "play.fill") { party.openPlayer() }
                            .buttonStyle(PrimaryCapsuleStyle(height: 40))
                    } else {
                        Button(L.togetherOpenLobby) { party.openLobby(TogetherRoom(server: view.server, room: view.room)) }
                            .buttonStyle(PrimaryCapsuleStyle(height: 40))
                    }
                    Button(L.partyLeave) { party.leave() }
                        .buttonStyle(SecondaryCapsuleStyle(height: 40))
                }
            }
        }
    }
}

/// The lobby on iPhone: what plays, who is in the room and the code, with I'm ready pinned low
/// and Leave and Change episode under More.
struct PhoneTogetherLobby: View {
    @Environment(AppStore.self) private var store
    /// The keyboard is up, for the chat's field.
    @State private var typing = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private static let chatPlace = "chat"

    /// The compact header's title: the show.
    private var compactTitle: String {
        let party = store.party
        guard let room = party.lobbyEpisode else { return L.togetherTitle }
        return room.id.flatMap { store.titles[$0]?.showTitle } ?? room.title
    }

    /// The compact header's line: how many are in the room and ready.
    private var compactLine: String {
        let members = store.party.members ?? []
        return L.partyBarLine(count: members.count, ready: members.filter { $0.ready == true }.count)
    }

    var body: some View {
        let party = store.party
        ScrollViewReader { reader in
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    // While the chat field has the keyboard the large header steps aside for the
                    // compact one in the navigation bar (watch together chat board, iPhone).
                    if !typing {
                        GeometryReader { proxy in
                            TogetherStill(id: party.lobbyEpisode?.id, width: proxy.size.width,
                                          height: proxy.size.width * 9 / 16)
                        }
                        .aspectRatio(16 / 9, contentMode: .fit)
                        .transition(.opacity)
                        TogetherRoomBlock(titleSize: 22).padding(.top, 16)
                            .transition(.opacity)
                    }
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
                    TogetherMembers(rowHeight: 52).padding(.top, 28)
                    if party.inLobbyRoom, let view = party.party {
                        // Talk before play follows the members (watch together chat board, iPhone).
                        TogetherChat(size: 15).id(Self.chatPlace).padding(.top, 24)
                        TogetherInvite(view: view, codeSize: 22).padding(.top, 28)
                    }
                }
                .padding(EdgeInsets(top: 8, leading: 16, bottom: 24, trailing: 16))
            }
            #if FIXTURES
            .task {
                // The lobby's chat shot: the conversation is under the members, so it scrolls there.
                guard TogetherScene.named() == .lobbyChat else { return }
                try? await Task.sleep(for: .milliseconds(700))
                reader.scrollTo(Self.chatPlace, anchor: .bottom)
            }
            #endif
            .onChange(of: typing) { _, up in
                // The field and the privacy line under it stay in view above the keyboard.
                if up { withAnimation(.standard) { reader.scrollTo(Self.chatPlace, anchor: .bottom) } }
            }
        }
        .background(.ground)
        // While typing, the field rides above the keyboard and I'm ready steps aside for it.
        .safeAreaInset(edge: .bottom) {
            if !typing { pinned.transition(.opacity) }
        }
        .animation(reduceMotion ? .fadeIn : .standard, value: typing)
        .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillShowNotification)) { _ in
            typing = true
        }
        .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in
            typing = false
        }
        .navigationTitle(typing ? compactTitle : "")
        .navigationSubtitle(typing ? compactLine : "")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.hidden, for: .tabBar)
        .toolbar {
            if party.inLobbyRoom, let view = party.party {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        if view.roomEpisode != nil, !view.playing {
                            Button(L.partyChangeEpisode, systemImage: "list.bullet") { party.beginStart() }
                        }
                        Button(L.partyLeave, systemImage: "rectangle.portrait.and.arrow.right", role: .destructive) {
                            party.leave()
                        }
                    } label: {
                        Label(L.actionMore, systemImage: "ellipsis")
                    }
                }
            }
        }
        .onDisappear { if !party.lobbyShown { party.closeLobby() } }
    }

    /// I'm ready, Join or what the room asks for, across the width, and the start line.
    private var pinned: some View {
        let party = store.party
        return VStack(spacing: 8) {
            TogetherActions(height: 50, fills: true, showsSecondary: false)
                .frame(maxWidth: .infinity)
            if party.inLobbyRoom, let view = party.party, view.roomEpisode?.id != nil, !view.playing, party.lastOut == nil {
                TogetherStartLine(view: view)
            }
        }
        .padding(EdgeInsets(top: 10, leading: 16, bottom: 8, trailing: 16))
        .background(.ground)
    }
}

/// The party you are in, over the tab bar while you are elsewhere: the still, the title and
/// episode, the room's count, and a tap back to the lobby or the player.
struct PartyBar: View {
    @Environment(AppStore.self) private var store
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement
    /// The newest line of chat takes the count's place for 4 s.
    @State private var said: ChatColumnLine?

    var body: some View {
        let party = store.party
        if let view = party.party {
            let members = party.members ?? []
            HStack(spacing: 12) {
                if placement != .inline {
                    TogetherStill(id: view.roomEpisode?.id, width: 48, height: 27, radius: 4)
                }
                VStack(alignment: .leading, spacing: 0) {
                    Text(view.roomEpisode.map { r in
                        L.partyBarTitle(title: r.id.flatMap { store.titles[$0]?.showTitle } ?? r.title, episode: r.episode)
                    } ?? L.navWatchTogether)
                        .font(.board(15, weight: .medium)).foregroundStyle(.silk).lineLimit(1)
                    if placement != .inline {
                        Group {
                            if let said {
                                (Text(said.you ? L.partyChatYou : said.from).fontWeight(.semibold) + Text(" ") + Text(said.text))
                                    .foregroundStyle(.silk)
                            } else {
                                Text(view.playing && !party.playerUp() ? L.partyBarIdle
                                     : L.partyBarLine(count: members.count, ready: members.filter { $0.ready == true }.count))
                                    .foregroundStyle(.haze)
                            }
                        }
                        .font(.ui(13)).lineLimit(1).monospacedDigit()
                        .contentTransition(.opacity)
                        .animation(.fadeIn, value: said?.id)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                if view.playing {
                    Image(systemName: "play.fill")
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(.silk)
                        .frame(width: 44, height: 44)
                        .accessibilityHidden(true)
                }
            }
            .padding(.leading, 16)
            .padding(.trailing, 4)
            .contentShape(.rect)
            .task(id: party.chat.latest?.id) {
                guard party.chat.shown, let latest = party.chat.latest else { return }
                said = latest
                try? await Task.sleep(for: .seconds(4))
                if !Task.isCancelled { said = nil }
            }
            .onTapGesture(perform: open)
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.isButton)
            .accessibilityAction { open() }
        }
    }

    private func open() {
        let party = store.party
        guard let view = party.party else { return }
        if view.playing {
            party.openPlayer()
        } else {
            party.openLobby(TogetherRoom(server: view.server, room: view.room))
        }
    }
}
#endif
