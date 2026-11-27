// The room's column beside the video on Mac (and iPad in a regular width): 336 pt, a floating
// glass sidebar inset 8. The room's line on top, the people under it, then the chat. Invite and
// Details open from the header; Leave always asks (the room board, v5).

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct RoomColumn: View {
    let room: RoomModel
    /// The window's height: under 680 pt the list shows three rows and scrolls.
    var height: CGFloat = 720
    @FocusState private var fieldFocus: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    static let width: CGFloat = 336

    var body: some View {
        RoomClock { now in
            VStack(alignment: .leading, spacing: 0) {
                header
                line(now)
                rows(now)
                Rectangle().fill(Palette.faintRule).frame(height: 1)
                // Scrolls once it is longer than the room left for it, the newest line at the
                // bottom; the header and the field stay where they are.
                ScrollView {
                    RoomConversation(room: room, size: 13, lineHeight: 18, gap: 7)
                        .padding(.top, 12)
                        .padding(.bottom, 10)
                }
                .defaultScrollAnchor(.bottom)
                .scrollIndicators(.never)
                .frame(maxHeight: .infinity, alignment: .bottom)
                // Hide chat in rooms: no field; the room's own lines still say what it does.
                if !room.chatOff { field }
            }
            .padding(.horizontal, 16)
            .frame(maxHeight: .infinity)
            .background(Color(rgb: 0x1C2034), in: .rect(cornerRadius: 16))
            .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(Palette.glassEdge, lineWidth: 1))
            .overlay {
                // Invite and Details have nothing to lose: a click outside closes them. Edit
                // relays holds edits not yet saved, so it stays until Done or Esc.
                if room.popoverClosesOutside {
                    Color.clear.contentShape(.rect).onTapGesture { room.popover = nil }
                }
            }
            .overlay(alignment: .topTrailing) { popover }
            .clipShape(.rect(cornerRadius: 16))
            .padding(8)
        }
        .frame(width: Self.width)
        .background(Color(rgb: 0x0B0D18))
        .foregroundStyle(.silk)
        .animation(.fadeIn, value: room.popover)
    }

    // MARK: Header

    private var header: some View {
        HStack(spacing: 8) {
            Text(RoomCopy.columnTitle)
                .font(.system(size: 13, weight: .semibold))
                .frame(maxWidth: .infinity, alignment: .leading)
            if room.inRoom {
                headerIcon("person.badge.plus", RoomCopy.invite, on: room.popover == .invite) { toggle(.invite) }
                headerIcon("info.circle", RoomCopy.details, on: room.popover == .details || room.popover == .editRelays) {
                    toggle(.details)
                }
                Button(RoomCopy.leave) { room.leaving = .leave }
                    .buttonStyle(RoomButtonStyle(kind: .secondary, height: 28, size: 12, padding: 12))
            }
        }
        .frame(height: 52)
    }

    private func toggle(_ which: RoomPopover) {
        room.popover = room.popover == which || (which == .details && room.popover == .editRelays) ? nil : which
    }

    private func headerIcon(_ symbol: String, _ label: String, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 13, weight: .medium))
                .frame(width: 28, height: 28)
                .background(Palette.silk.opacity(on ? 0.22 : 0.10), in: .circle)
                .contentShape(.circle)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .help(label)
    }

    // MARK: The room's line

    private func line(_ now: Int64) -> some View {
        let status = room.status(now: now)
        return VStack(alignment: .leading, spacing: 2) {
            Text(status.title)
                .font(.system(size: 20, weight: .semibold))
                .lineSpacing(2)
                .fixedSize(horizontal: false, vertical: true)
            Text(status.detail)
                .font(.system(size: 13))
                .foregroundStyle(.haze)
                .monospacedDigit()
                .fixedSize(horizontal: false, vertical: true)
            RoomLineActions(room: room, status: status, mac: true)
                .padding(.top, 8)
        }
        .contentTransition(.opacity)
        .animation(.fadeIn, value: status)
        .padding(.top, 2)
        .padding(.bottom, 12)
        .accessibilityElement(children: .contain)
    }

    // MARK: People

    private func rows(_ now: Int64) -> some View {
        let people = room.people
        let limit = height < 680 ? 3 : 5
        let many = people.count > limit
        return ScrollView {
            VStack(spacing: 2) {
                ForEach(people) { member in
                    RoomRow(room: room, member: member, now: now)
                        .transition(reduceMotion ? .opacity : .opacity.combined(with: .offset(y: -8)))
                }
            }
            .padding(.bottom, 10)
            .animation(.standard, value: people.map(\.id))
            .accessibilityElement(children: .contain)
            .accessibilityLabel(RoomCopy.people)
        }
        .scrollDisabled(!many)
        .scrollIndicators(.never)
        .frame(height: many ? CGFloat(limit) * 48 + 10 : CGFloat(people.count) * 48 + 10)
        .mask {
            if many {
                LinearGradient(stops: [.init(color: .black, location: 0.8), .init(color: .clear, location: 1)], startPoint: .top, endPoint: .bottom)
            } else {
                Color.black
            }
        }
    }

    // MARK: The field

    private var field: some View {
        HStack(spacing: 8) {
            TextField(
                "", text: Binding(get: { room.draft }, set: { room.draft = $0 }),
                prompt: Text(RoomCopy.saySomething).foregroundStyle(.haze)
            )
            .textFieldStyle(.plain)
            .font(.system(size: 13))
            .focused($fieldFocus)
            .onChange(of: room.chatAsked) { fieldFocus = true }
            .onSubmit { room.say() }
            .padding(.horizontal, 14)
            .frame(height: 32)
            .background(Palette.silk.opacity(0.08), in: .capsule)
            .overlay(Capsule().strokeBorder(Palette.glassEdge, lineWidth: 1))
            .disabled(!room.inRoom)
            Button { room.say() } label: {
                Image(systemName: "arrow.up")
                    .font(.system(size: 13, weight: .bold))
                    .frame(width: 32, height: 32)
                    .background(room.draft.isEmpty ? Palette.silk.opacity(0.10) : Palette.amber, in: .circle)
                    .foregroundStyle(room.draft.isEmpty ? Palette.haze : Palette.ground)
                    .contentShape(.circle)
            }
            .buttonStyle(.plain)
            .disabled(room.draft.trimmingCharacters(in: .whitespaces).isEmpty)
            .accessibilityLabel(RoomCopy.send)
        }
        .padding(.bottom, 14)
    }

    // MARK: Popovers

    @ViewBuilder
    private var popover: some View {
        switch room.popover {
        case .invite: RoomPopBox { RoomInvitePanel(room: room) }
        case .details: RoomPopBox { RoomDetailsPanel(room: room) }
        case .editRelays: RoomPopBox { RoomRelayEditor(room: room) }
        case nil: EmptyView()
        }
    }
}

/// One person: face, name, their word.
struct RoomRow: View {
    let room: RoomModel
    let member: RoomMember
    let now: Int64

    var body: some View {
        HStack(spacing: 12) {
            RoomFace(name: member.name, look: room.look(member, now: now), disc: 30)
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 4) {
                    Text(member.name).fontWeight(.semibold)
                    if member.you { Text(RoomCopy.youMark).foregroundStyle(.haze) }
                }
                .font(.system(size: 13))
                .lineLimit(1)
                Text(room.word(member, now: now))
                    .font(.system(size: 12))
                    .foregroundStyle(member.state == .watching ? Palette.haze : Palette.silk)
                    .monospacedDigit()
                    .lineLimit(1)
                    .contentTransition(.opacity)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if member.state == .behind, !member.you, room.view != nil {
                Text(RoomModel.clock(room.position(at: now) - Double(member.behindS ?? 0)))
                    .font(.system(size: 12))
                    .foregroundStyle(.haze)
                    .monospacedDigit()
            }
        }
        .frame(height: 46)
        .opacity(member.state == .left ? 0.8 : 1)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(member.name)\(member.you ? " \(RoomCopy.youMark)" : ""), \(room.word(member, now: now))")
        .contextMenu {
            if !member.you {
                Button(member.muted ? L.roomUnmute(name: member.name) : L.roomMute(name: member.name)) { room.mute(member, !member.muted) }
            }
        }
    }
}

/// The room line's buttons: Carry on, Start, Join, Go along.
struct RoomLineActions: View {
    let room: RoomModel
    let status: RoomStatus
    var mac = false

    var body: some View {
        let height: CGFloat = mac ? 32 : 44
        let size: CGFloat = mac ? 13 : 15
        let padding: CGFloat = mac ? 14 : 20
        Group {
            switch status.action {
            case .carryOn(let who):
                Button(RoomCopy.carryOnWithout(who)) { room.carryOn() }
                    .buttonStyle(RoomButtonStyle(kind: .primary, height: height, size: size, padding: padding))
            case .start(let who):
                Button(RoomCopy.startWithout(who)) { room.carryOn() }
                    .buttonStyle(RoomButtonStyle(kind: .primary, height: height, size: size, padding: padding))
            case .join(let over):
                HStack(spacing: 8) {
                    Button(over ? RoomCopy.joinAnyway : RoomCopy.joinTheRoom) { room.accept() }
                        .buttonStyle(RoomButtonStyle(kind: .primary, height: height, size: size, padding: padding))
                    Button(RoomCopy.notNow) { room.notNow() }
                        .buttonStyle(RoomButtonStyle(kind: .secondary, height: height, size: size, padding: padding))
                }
            case .goAlong:
                HStack(spacing: 8) {
                    Button(RoomCopy.goAlong) { room.answer(go: true) }
                        .buttonStyle(RoomButtonStyle(kind: .primary, height: height, size: size, padding: padding))
                    Button(RoomCopy.stayHere) { room.answer(go: false) }
                        .buttonStyle(RoomButtonStyle(kind: .secondary, height: height, size: size, padding: padding))
                }
            case nil:
                // On a phone the starting screen has no room for it under the video: the room
                // waits for you, so the line offers to let it go on.
                if !mac, room.view?.canDontWait == true {
                    Button(RoomCopy.dontWait) { room.dontWait() }
                        .buttonStyle(RoomButtonStyle(kind: .secondary, height: height, size: size, padding: padding))
                }
            }
        }
        .transition(.opacity)
    }
}

/// The room's conversation: what the room says in haze, chat lines with the speaker's name.
struct RoomConversation: View {
    let room: RoomModel
    var size: CGFloat = 13
    var lineHeight: CGFloat = 18
    var gap: CGFloat = 7

    var body: some View {
        VStack(alignment: .leading, spacing: gap) {
            ForEach(room.shownLines) { line in
                lineView(line)
                    .transition(.opacity)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .animation(.fadeIn, value: room.shownLines.map(\.id))
    }

    @ViewBuilder
    private func lineView(_ line: RoomLine) -> some View {
        switch line.kind {
        case .system(let text):
            Text(text)
                .font(.system(size: size - 1))
                .foregroundStyle(.haze)
                .monospacedDigit()
                .lineSpacing(lineHeight - size - 3)
        case .chat(let from, let text, let you):
            Text("\(Text(from).fontWeight(.semibold).foregroundColor(you ? Palette.haze : Palette.silk))  \(text)")
                .font(.system(size: size))
                .lineSpacing(lineHeight - size - 3)
                .fixedSize(horizontal: false, vertical: true)
        case .reaction(let from, let reaction, let at):
            HStack(spacing: 6) {
                ReactionGlyphView(ReactionGlyphKind(reaction), size: size + 3, weight: 1.6)
                Text(RoomCopy.reacted(from, reaction, at))
            }
            .font(.system(size: size - 1))
            .foregroundStyle(.haze)
        }
    }
}
