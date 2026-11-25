// The room on iPhone (and iPad in a compact width): held upright, the video sits on top and the
// room fills the rest; turned sideways the video fills the screen and the strip holds the room
// (the room board, v5).

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// Under the video, held upright: the title row, the room's line, the faces, the chat and its
/// field.
struct PhoneRoomUpright: View {
    let room: RoomModel
    /// Subtitles and audio, and Episodes: the player's own sheets.
    let tracks: () -> Void
    let episodes: () -> Void
    @State private var everyone = false
    @FocusState private var fieldFocus: Bool

    var body: some View {
        RoomClock { now in
            let status = room.status(now: now)
            VStack(alignment: .leading, spacing: 0) {
                titleRow
                VStack(alignment: .leading, spacing: 0) {
                    Text(status.title)
                        .font(.system(size: 22, weight: .bold))
                        .fixedSize(horizontal: false, vertical: true)
                    Text(status.detail)
                        .font(.system(size: 15))
                        .foregroundStyle(.haze)
                        .monospacedDigit()
                        .fixedSize(horizontal: false, vertical: true)
                    RoomLineActions(room: room, status: status)
                        .padding(.top, 10)
                }
                .contentTransition(.opacity)
                .animation(.fadeIn, value: status)
                .padding(.top, 14)
                faces(now)
                    .padding(.top, 14)
                Rectangle().fill(Palette.faintRule).frame(height: 1).padding(.top, 14)
                ScrollView {
                    RoomConversation(room: room, size: 15, lineHeight: 20, gap: 8)
                        .padding(.vertical, 10)
                }
                .defaultScrollAnchor(.bottom)
                .scrollIndicators(.never)
                .frame(maxHeight: .infinity)
                field
            }
            .padding(.horizontal, 16)
            .padding(.top, 12)
        }
        .foregroundStyle(.silk)
        .background(Palette.ground)
        .sheet(isPresented: $everyone) { PhoneRoomEveryone(room: room) }
        .modifier(PhoneRoomSheets(room: room))
    }

    private var titleRow: some View {
        HStack(spacing: 8) {
            VStack(alignment: .leading, spacing: 0) {
                Text(room.invitation?.title ?? room.view?.title ?? "")
                    .font(.showTitle(size: 15))
                    .lineLimit(1)
                Text(RoomCopy.inTheRoom(room.view?.episode.episode ?? room.invitation?.episode.episode ?? "", room.people.count))
                    .font(.system(size: 13))
                    .foregroundStyle(.haze)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if room.inRoom {
                glassCircle("person.badge.plus", RoomCopy.invite) { room.popover = .invite }
                Menu {
                    Button(RoomCopy.leaveTheRoomDots, systemImage: "rectangle.portrait.and.arrow.right") { room.leaving = .leave }
                    Divider()
                    Button(RoomCopy.details, systemImage: "info.circle") { room.popover = .details }
                    Button(RoomCopy.copyInviteLink, systemImage: "doc.on.doc") { RoomPasteboard.copy(room.view?.link ?? "") }
                    Button(RoomCopy.moveToNewDots, systemImage: "person.2") { room.startMove() }
                    Button(L.playerSubtitlesAndAudio, systemImage: "captions.bubble") { tracks() }
                    Button(L.playerEpisodes, systemImage: "list.bullet") { episodes() }
                } label: {
                    Image(systemName: "ellipsis")
                        .font(.system(size: 18, weight: .semibold))
                        .frame(width: 44, height: 44)
                        .glassEffect(.regular.interactive(), in: .circle)
                }
                .accessibilityLabel(RoomCopy.more)
            }
        }
    }

    private func glassCircle(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 18, weight: .medium))
                .frame(width: 44, height: 44)
                .glassEffect(.regular.interactive(), in: .circle)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }

    private func faces(_ now: Int64) -> some View {
        let (shown, rest) = RoomFaces.capped(room.people)
        return HStack(alignment: .top, spacing: 4) {
            ForEach(shown) { member in
                VStack(spacing: 0) {
                    RoomFace(name: member.name, look: room.look(member, now: now), disc: 48)
                    Text(member.you ? RoomCopy.you : member.name)
                        .font(.system(size: 13, weight: .semibold))
                        .lineLimit(1)
                        .padding(.top, 4)
                    Text(room.word(member, now: now))
                        .font(.system(size: 12))
                        .foregroundStyle(.haze)
                        .monospacedDigit()
                        .multilineTextAlignment(.center)
                        .lineLimit(2)
                }
                .frame(width: 86)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("\(member.you ? RoomCopy.you : member.name), \(room.word(member, now: now))")
            }
            if !rest.isEmpty {
                Button { everyone = true } label: {
                    VStack(spacing: 0) {
                        RoomMoreFace(label: "+\(rest.count)", disc: 48)
                        Text(RoomCopy.more).font(.system(size: 13, weight: .semibold)).padding(.top, 4)
                        Text(RoomFaces.moreWord(rest)).font(.system(size: 12)).foregroundStyle(.haze).multilineTextAlignment(.center)
                    }
                    .frame(width: 86)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
            }
        }
        .animation(.standard, value: room.people.map(\.id))
    }

    private var field: some View {
        HStack(spacing: 8) {
            Menu {
                ForEach(Reaction.order, id: \.self) { reaction in
                    Button(reaction.buttonName) { room.react(reaction) }
                }
            } label: {
                ReactionGlyphView(.laugh, size: 22, weight: 1.6)
                    .frame(width: 44, height: 44)
                    .glassEffect(.regular.interactive(), in: .circle)
            }
            .accessibilityLabel(RoomCopy.react)
            TextField("", text: Binding(get: { room.draft }, set: { room.draft = $0 }), prompt: Text(RoomCopy.saySomething).foregroundStyle(.haze))
                .font(.system(size: 16))
                .focused($fieldFocus)
                .submitLabel(.send)
                .onSubmit { room.say() }
                .padding(.horizontal, 16)
                .frame(height: 44)
                .glassEffect(.regular, in: .capsule)
            Button { room.say() } label: {
                Image(systemName: "arrow.up")
                    .font(.system(size: 17, weight: .bold))
                    .frame(width: 44, height: 44)
                    .background(room.draft.isEmpty ? Palette.silk.opacity(0.10) : Palette.amber, in: .circle)
                    .foregroundStyle(room.draft.isEmpty ? Palette.haze : Palette.ground)
            }
            .buttonStyle(.plain)
            .disabled(room.draft.trimmingCharacters(in: .whitespaces).isEmpty)
            .accessibilityLabel(RoomCopy.send)
        }
        .disabled(!room.inRoom)
        .opacity(room.inRoom ? 1 : 0.5)
        .padding(.top, 8)
        .padding(.bottom, fieldFocus ? 8 : 40)
    }
}

/// Everyone in the room, from the faces' More: the Mac's rows.
struct PhoneRoomEveryone: View {
    let room: RoomModel

    var body: some View {
        RoomClock { now in
            ScrollView {
                VStack(spacing: 2) {
                    ForEach(room.people) { RoomRow(room: room, member: $0, now: now) }
                }
                .padding(20)
            }
        }
        .foregroundStyle(.silk)
        .presentationDetents([.medium, .large])
        .presentationBackground(Palette.ground)
    }
}

/// Invite, Details, Edit relays and Move as sheets, and Leave as an alert.
struct PhoneRoomSheets: ViewModifier {
    let room: RoomModel

    func body(content: Content) -> some View {
        content
            .sheet(isPresented: Binding(get: { room.popover != nil }, set: { if !$0 { room.popover = nil } })) {
                Group {
                    switch room.popover {
                    case .invite: PhoneRoomInvite(room: room)
                    case .details: VStack(alignment: .leading, spacing: 0) { RoomDetailsPanel(room: room) }.font(.system(size: 15))
                    case .editRelays: VStack(alignment: .leading, spacing: 0) { RoomRelayEditor(room: room) }.font(.system(size: 15))
                    case nil: EmptyView()
                    }
                }
                .padding(.horizontal, 20)
                .padding(.top, 24)
                .frame(maxHeight: .infinity, alignment: .top)
                .foregroundStyle(.silk)
                .presentationDetents([.medium, .large])
                .presentationBackground(.thinMaterial)
                .preferredColorScheme(.dark)
            }
            .sheet(isPresented: Binding(get: { room.moving }, set: { if !$0 { room.moving = false } })) {
                PhoneRoomMove(room: room)
                    .presentationDetents([.medium, .large])
                    .presentationBackground(.thinMaterial)
                    .preferredColorScheme(.dark)
            }
            .alert(
                RoomCopy.leaveTheRoom,
                isPresented: Binding(get: { room.leaving != nil }, set: { if !$0 { room.leaving = nil } })
            ) {
                Button(RoomCopy.cancel, role: .cancel) { room.leaving = nil }
                Button(RoomCopy.leave, role: .destructive) { room.leave() }
            } message: {
                Text(RoomCopy.othersKeepWatching)
            }
    }
}

struct PhoneRoomInvite: View {
    let room: RoomModel

    var body: some View {
        let link = room.view?.link ?? ""
        VStack(alignment: .leading, spacing: 0) {
            Text(RoomCopy.invite).font(.system(size: 20, weight: .bold))
            Text(RoomCopy.inviteBodyPhone).font(.system(size: 15)).foregroundStyle(.haze).padding(.top, 4)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 16) {
                RoomQRCode(link: link, size: 112)
                Text(RoomCopy.scanNote).font(.system(size: 13)).foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.top, 14)
            Text(roomLinkShown(link))
                .font(.system(size: 13, design: .monospaced))
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
                .padding(.horizontal, 14)
                .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                .background(Palette.silk.opacity(0.08), in: .rect(cornerRadius: 12))
                .padding(.top, 14)
            HStack(spacing: 10) {
                Button { RoomPasteboard.copy(link) } label: { Label(RoomCopy.copyLink, systemImage: "doc.on.doc") }
                    .buttonStyle(RoomButtonStyle(kind: .primary, height: 50, size: 17, wide: true))
                if let url = URL(string: link) {
                    ShareLink(item: url) { Label(RoomCopy.share, systemImage: "square.and.arrow.up") }
                        .buttonStyle(RoomButtonStyle(kind: .secondary, height: 50, size: 17, wide: true))
                }
            }
            .padding(.top, 12)
            Button { room.newInvite() } label: {
                HStack {
                    Text(RoomCopy.newInvite).font(.system(size: 15))
                    Spacer()
                    Text(RoomCopy.stopsOldLink).font(.system(size: 13)).foregroundStyle(.haze)
                }
                .frame(height: 44)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .overlay(alignment: .top) { Rectangle().fill(Palette.glassEdge).frame(height: 1) }
            .padding(.top, 8)
        }
    }
}

struct PhoneRoomMove: View {
    let room: RoomModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(RoomCopy.moveToNew).font(.system(size: 20, weight: .bold))
            Text(RoomCopy.moveBody).font(.system(size: 15)).foregroundStyle(.haze).padding(.top, 4)
                .fixedSize(horizontal: false, vertical: true)
            RoomMoveList(room: room, phone: true)
                .padding(.horizontal, 14)
                .padding(.vertical, 2)
                .background(Palette.silk.opacity(0.06), in: .rect(cornerRadius: 20))
                .padding(.top, 14)
            Text(RoomCopy.moveFoot(RoomModel.clock(room.position(at: .nowMs))))
                .font(.system(size: 13)).foregroundStyle(.haze).padding(.top, 10)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 10) {
                Button(RoomCopy.cancel) { room.moving = false }
                    .buttonStyle(RoomButtonStyle(kind: .secondary, height: 50, size: 17, wide: true))
                Button(RoomCopy.moveWith(room.moveTicks.count)) { room.move() }
                    .buttonStyle(RoomButtonStyle(kind: .primary, height: 50, size: 17, wide: true))
                    .disabled(room.moveTicks.isEmpty)
            }
            .padding(.top, 14)
        }
        .foregroundStyle(.silk)
        .padding(.horizontal, 20)
        .padding(.top, 24)
        .frame(maxHeight: .infinity, alignment: .top)
    }
}

/// Over the video held upright: the room before play, its invitation, choosing a source, no
/// source.
struct PhoneRoomVideoPanel: View {
    let room: RoomModel
    let panel: RoomVideoPanel

    var body: some View {
        ZStack(alignment: .topLeading) {
            RoomBanner(id: room.episodeId)
            Color(rgb: 0x0B0D18).opacity(panel == .noSource ? 0.8 : 0.5)
            switch panel {
            case .beforePlay(let invited):
                if invited {
                    VStack(alignment: .leading, spacing: 0) {
                        Text(room.invitation?.title ?? "").font(.showTitle(size: 20)).lineLimit(1)
                        Text(episodeLine).font(.system(size: 13))
                    }
                    .overVideoText()
                    .padding(.leading, 16)
                    .padding(.bottom, 14)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading)
                } else {
                    let waiting = room.view?.waiting.isEmpty == false || room.view?.startingIn != nil
                    VStack(spacing: 8) {
                        Button { room.play() } label: {
                            Image(systemName: "play.fill")
                                .font(.system(size: 28))
                                .frame(width: 64, height: 64)
                                .glassEffect(.regular.interactive(), in: .circle)
                        }
                        .buttonStyle(.plain)
                        .disabled(waiting)
                        .accessibilityLabel(RoomCopy.playForEveryone)
                        Text(waiting ? RoomCopy.waitingFirstFrames : RoomCopy.playForEveryone)
                            .font(.system(size: 13))
                            .overVideoText()
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            case .choosing:
                VStack(alignment: .leading, spacing: 0) {
                    Text(RoomCopy.chooseSourceFor(room.view?.episode.episode ?? "")).font(.system(size: 15, weight: .semibold))
                    RoomSourceList(room: room, rowHeight: 44, nameSize: 15)
                        .padding(.top, 8)
                    Button(RoomCopy.playInTheRoom) { room.playChosen() }
                        .buttonStyle(RoomButtonStyle(kind: .primary, height: 44, size: 15, padding: 18))
                        .padding(.top, 8)
                }
                .padding(.leading, 56)
                .padding(.trailing, 16)
                .padding(.top, 12)
            case .noSource:
                VStack(alignment: .leading, spacing: 0) {
                    Text(RoomCopy.noneHas(room.view?.episode.episode ?? "")).font(.system(size: 17, weight: .semibold))
                    Text(RoomCopy.youStay).font(.system(size: 13)).foregroundStyle(.haze).padding(.top, 4)
                    HStack(spacing: 8) {
                        Button(RoomCopy.openSources) { room.openSources() }
                            .buttonStyle(RoomButtonStyle(kind: .primary, height: 44, size: 15, padding: 18))
                        Button(RoomCopy.searchTitles) { room.searchTitles() }
                            .buttonStyle(RoomButtonStyle(kind: .glass, height: 44, size: 15, padding: 16))
                    }
                    .padding(.top, 14)
                }
                .padding(.horizontal, 16)
                .padding(.top, 52)
            }
        }
        .foregroundStyle(.silk)
        .clipped()
    }

    private var episodeLine: String {
        guard let episode = room.invitation?.episode ?? room.view?.episode else { return "" }
        let name = room.episodeName(episode.episode)
        return episode.name.map { "\(name) · \($0)" } ?? name
    }
}
#endif
