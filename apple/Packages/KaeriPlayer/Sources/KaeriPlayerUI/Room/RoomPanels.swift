// The room's popovers, dialogs and the panels over the video: Invite, Details and Edit relays;
// Leave and ⌘W; Move to a new room; the room before play, choosing a source and no source; Up
// next and Skip opening (the room board, v5).

import CoreImage.CIFilterBuiltins
import KaeriDesign
import KaeriKit
import KaeriStore
import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

// MARK: Popovers

/// A popover's box, hung under the column's header.
struct RoomPopBox<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) { content() }
            .font(.system(size: 13))
            .padding(.horizontal, 16)
            .padding(.vertical, 14)
            .frame(width: 300, alignment: .leading)
            .background(Palette.raisedHigh, in: .rect(cornerRadius: 14))
            .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(Palette.rule, lineWidth: 1))
            .shadow(color: .black.opacity(0.45), radius: 15, y: 10)
            .padding(.top, 48)
            .padding(.trailing, 8)
            .transition(.opacity)
    }
}

extension Palette {
    /// --high on the board: popovers and menus.
    static let raisedHigh = Color(rgb: 0x2A2F45)
    /// The column's own glass.
    static let roomSide = Color(rgb: 0x1C2034)
}

enum RoomPasteboard {
    @MainActor static func copy(_ text: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #else
        UIPasteboard.general.string = text
        #endif
    }
}

/// The link as it reads in a field: without the scheme.
func roomLinkShown(_ link: String) -> String {
    link.replacingOccurrences(of: "https://", with: "")
}

struct RoomInvitePanel: View {
    let room: RoomModel

    var body: some View {
        let link = room.view?.link ?? ""
        Text(RoomCopy.invite).fontWeight(.semibold).padding(.bottom, 4)
        Text(RoomCopy.inviteBody).font(.system(size: 12)).foregroundStyle(.haze).fixedSize(horizontal: false, vertical: true)
        HStack(spacing: 8) {
            Text(roomLinkShown(link))
                .font(.system(size: 11, design: .monospaced))
                .lineLimit(1)
                .truncationMode(.tail)
                .textSelection(.enabled)
                .padding(.horizontal, 10)
                .frame(maxWidth: .infinity, minHeight: 28, alignment: .leading)
                .background(Palette.silk.opacity(0.08), in: .rect(cornerRadius: 7))
                .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(Palette.glassEdge, lineWidth: 1))
            Button { RoomPasteboard.copy(link) } label: { Label(RoomCopy.copyLink, systemImage: "doc.on.doc") }
                .buttonStyle(RoomButtonStyle(kind: .primary, height: 28, size: 12, padding: 12))
        }
        .padding(.top, 10)
        HStack(spacing: 8) {
            if let url = URL(string: link) {
                ShareLink(item: url) { Text(RoomCopy.share) }
                    .buttonStyle(RoomButtonStyle(kind: .secondary, height: 28, size: 12, padding: 12))
            }
            Button(RoomCopy.newInvite) { room.newInvite() }
                .buttonStyle(RoomButtonStyle(kind: .secondary, height: 28, size: 12, padding: 12))
        }
        .padding(.top, 6)
        Text(RoomCopy.newInviteNote).font(.system(size: 12)).foregroundStyle(.haze).fixedSize(horizontal: false, vertical: true).padding(.top, 8)
        Rectangle().fill(Palette.faintRule).frame(height: 1).padding(.vertical, 10)
        Button(RoomCopy.moveToNewDots) { room.startMove() }
            .buttonStyle(RoomButtonStyle(kind: .secondary, height: 28, size: 12, padding: 12))
        Text(RoomCopy.moveNote).font(.system(size: 12)).foregroundStyle(.haze).fixedSize(horizontal: false, vertical: true).padding(.top, 6)
    }
}

struct RoomDetailsPanel: View {
    let room: RoomModel

    var body: some View {
        let relays = room.view?.relays ?? []
        let up = relays.filter(\.healthy).count
        Text(RoomCopy.details).fontWeight(.semibold).padding(.bottom, 8)
        detailRow(RoomCopy.encryption, RoomCopy.endToEnd)
        detailRow(RoomCopy.relays, RoomCopy.answering(up, relays.count))
        VStack(spacing: 0) {
            ForEach(Array(relays.enumerated()), id: \.element.id) { index, relay in
                HStack(spacing: 8) {
                    Circle().fill(relay.healthy ? Palette.water : Palette.haze).frame(width: 7, height: 7)
                    Text(relay.url).font(.system(size: 11, design: .monospaced))
                        // A long address wraps whole: there's no other way to read it.
                        .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Text(relay.healthy ? relay.rttMs.map { RoomCopy.ms(Int($0)) } ?? "" : RoomCopy.notAnswering)
                        .font(.system(size: 12)).foregroundStyle(.haze).monospacedDigit()
                }
                .frame(height: 28)
                .overlay(alignment: .top) { if index > 0 { Rectangle().fill(Palette.faintRule).frame(height: 1) } }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 2)
        .background(Palette.silk.opacity(0.05), in: .rect(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Palette.glassEdge, lineWidth: 1))
        .padding(.top, 6)
        Text(RoomCopy.relaysNote).font(.system(size: 12)).foregroundStyle(.haze).fixedSize(horizontal: false, vertical: true).padding(.top, 8)
        Button(RoomCopy.editRelaysDots) { room.popover = .editRelays }
            .buttonStyle(RoomButtonStyle(kind: .secondary, height: 28, size: 12, padding: 12))
            .padding(.top, 10)
    }

    private func detailRow(_ label: String, _ value: String) -> some View {
        HStack(spacing: 12) {
            Text(label).foregroundStyle(.haze)
            Spacer(minLength: 0)
            Text(value).lineLimit(1)
        }
        .frame(height: 26)
    }
}

struct RoomRelayEditor: View {
    let room: RoomModel
    @State private var relays: [String] = []
    @State private var adding = "wss://"

    var body: some View {
        Text(RoomCopy.editRelays).fontWeight(.semibold).padding(.bottom, 4)
        Text(RoomCopy.editRelaysNote).font(.system(size: 12)).foregroundStyle(.haze).fixedSize(horizontal: false, vertical: true)
        VStack(spacing: 0) {
            ForEach(Array(relays.enumerated()), id: \.element) { index, relay in
                HStack(spacing: 8) {
                    Circle().fill(Palette.water).frame(width: 7, height: 7)
                    Text(relay).font(.system(size: 11, design: .monospaced))
                        // A long address wraps whole: there's no other way to read it.
                        .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Button { relays.removeAll { $0 == relay } } label: {
                        Image(systemName: "xmark").font(.system(size: 9, weight: .bold)).foregroundStyle(.haze)
                            .frame(width: 24, height: 24).contentShape(.circle)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(RoomCopy.remove(relay))
                    .disabled(relays.count <= 1)
                }
                .frame(height: 30)
                .overlay(alignment: .top) { if index > 0 { Rectangle().fill(Palette.faintRule).frame(height: 1) } }
            }
        }
        .padding(.leading, 10)
        .padding(.trailing, 4)
        .padding(.vertical, 2)
        .background(Palette.silk.opacity(0.05), in: .rect(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Palette.glassEdge, lineWidth: 1))
        .padding(.top, 8)
        HStack(spacing: 8) {
            TextField("", text: $adding)
                .textFieldStyle(.plain)
                .font(.system(size: 11, design: .monospaced))
                .padding(.horizontal, 10)
                .frame(height: 28)
                .background(Palette.silk.opacity(0.08), in: .rect(cornerRadius: 7))
                .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(Palette.glassEdge, lineWidth: 1))
                .onSubmit(add)
            Button(RoomCopy.add, action: add)
                .buttonStyle(RoomButtonStyle(kind: .secondary, height: 28, size: 12, padding: 12))
                .disabled(!adding.hasPrefix("wss://") || adding.count < 10)
        }
        .padding(.top, 8)
        Text(RoomCopy.settingsRelaysNote).font(.system(size: 12)).foregroundStyle(.haze).fixedSize(horizontal: false, vertical: true).padding(.top, 8)
        HStack {
            Spacer()
            Button(RoomCopy.done) {
                room.setRelays(relays.map { $0.hasPrefix("wss://") ? $0 : "wss://\($0)" })
                room.popover = nil
            }
            .buttonStyle(RoomButtonStyle(kind: .primary, height: 28, size: 12, padding: 14))
        }
        .padding(.top, 8)
        .onAppear { relays = (room.view?.relays ?? []).map(\.url).map { $0.replacingOccurrences(of: "wss://", with: "") } }
    }

    private func add() {
        let relay = adding.replacingOccurrences(of: "wss://", with: "").trimmingCharacters(in: .whitespaces)
        guard !relay.isEmpty, !relays.contains(relay) else { return }
        relays.append(relay)
        adding = "wss://"
    }
}

// MARK: Dialogs

/// Leave the room?, and ⌘W's three ways out. Glass over a dimmed window.
struct RoomLeaveDialog: View {
    let room: RoomModel
    let ask: RoomLeaveAsk

    var body: some View {
        ZStack {
            Color.black.opacity(0.25).onTapGesture { room.leaving = nil }
            VStack(spacing: 0) {
                Text(RoomCopy.leaveTheRoom).font(.system(size: 15, weight: .bold))
                Text(ask == .close ? RoomCopy.othersKeepWatchingKeep : RoomCopy.othersKeepWatching)
                    .font(.system(size: 13)).foregroundStyle(.haze).multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 6)
                if ask == .close {
                    VStack(spacing: 8) {
                        Button(RoomCopy.leave) { room.leave() }
                            .buttonStyle(RoomButtonStyle(kind: .primary, height: 32, wide: true))
                            .keyboardShortcut(.defaultAction)
                        Button(RoomCopy.keepTheRoom) { room.keepRoom() }
                            .buttonStyle(RoomButtonStyle(kind: .secondary, height: 32, wide: true))
                        Button(RoomCopy.cancel) { room.leaving = nil }
                            .buttonStyle(RoomButtonStyle(kind: .secondary, height: 32, wide: true))
                            .keyboardShortcut(.cancelAction)
                    }
                    .padding(.top, 16)
                } else {
                    HStack(spacing: 8) {
                        Button(RoomCopy.cancel) { room.leaving = nil }
                            .buttonStyle(RoomButtonStyle(kind: .secondary, height: 32, wide: true))
                            .keyboardShortcut(.cancelAction)
                        Button(RoomCopy.leave) { room.leave() }
                            .buttonStyle(RoomButtonStyle(kind: .primary, height: 32, wide: true))
                            .keyboardShortcut(.defaultAction)
                    }
                    .padding(.top, 16)
                }
            }
            .padding(.top, 22)
            .padding(.horizontal, 18)
            .padding(.bottom, 16)
            .frame(width: 300)
            .roomDialogGlass(radius: 26)
        }
        .transition(.opacity)
    }
}

extension View {
    func roomDialogGlass(radius: CGFloat) -> some View {
        background(.ultraThinMaterial, in: .rect(cornerRadius: radius))
            .background(Color(rgb: 0x2A2F45).opacity(0.86), in: .rect(cornerRadius: radius))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(Palette.silk.opacity(0.16), lineWidth: 1))
            .shadow(color: .black.opacity(0.55), radius: 30, y: 24)
    }
}

/// A tick for Move's list.
struct RoomTick: View {
    let on: Bool
    var locked = false
    var size: CGFloat = 20

    var body: some View {
        ZStack {
            if on {
                Circle().fill(Palette.amber).opacity(locked ? 0.45 : 1)
                Image(systemName: "checkmark").font(.system(size: size * 0.5, weight: .bold)).foregroundStyle(Color(rgb: 0x1A1510))
            } else {
                Circle().strokeBorder(Palette.silk.opacity(0.4), lineWidth: 1.5)
            }
        }
        .frame(width: size, height: size)
    }
}

/// Move to a new room: who comes along. Everyone ticked except people you muted; you always go.
struct RoomMoveList: View {
    let room: RoomModel
    var phone = false

    var body: some View {
        let disc: CGFloat = phone ? 32 : 24
        let others = room.people.filter { !$0.you && $0.state != .left }
        VStack(spacing: 0) {
            if let you = room.you {
                row(name: RoomCopy.you, face: you, note: RoomCopy.alwaysGoes, on: true, locked: true, disc: disc, first: true) {}
            }
            ForEach(others) { member in
                let on = room.moveTicks.contains(member.id)
                let note = member.muted && !on ? RoomCopy.mutedNotTicked : nil
                row(name: member.name, face: member, note: note, on: on, locked: false, disc: disc, first: false) {
                    if on { room.moveTicks.remove(member.id) } else { room.moveTicks.insert(member.id) }
                }
            }
        }
    }

    private func row(
        name: String, face: RoomMember, note: String?, on: Bool, locked: Bool, disc: CGFloat, first: Bool,
        toggle: @escaping () -> Void
    ) -> some View {
        Button(action: toggle) {
            HStack(spacing: phone ? 12 : 10) {
                RoomFace(name: face.name, look: RoomFaceLook(ring: .with, dim: 1, pauseBadge: false, error: false), disc: disc)
                VStack(alignment: .leading, spacing: 0) {
                    Text(name).font(.system(size: phone ? 17 : 13))
                    if let note { Text(note).font(.system(size: phone ? 13 : 11)).foregroundStyle(.haze) }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                RoomTick(on: on, locked: locked, size: phone ? 26 : 20)
            }
            .frame(minHeight: phone ? 52 : 42)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .disabled(locked)
        .overlay(alignment: .top) { if !first { Rectangle().fill(Palette.faintRule).frame(height: 1) } }
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

struct RoomMoveDialog: View {
    let room: RoomModel

    var body: some View {
        ZStack {
            Color.black.opacity(0.25).onTapGesture { room.moving = false }
            VStack(spacing: 0) {
                Text(RoomCopy.moveToNew).font(.system(size: 15, weight: .bold))
                Text(RoomCopy.moveBody).font(.system(size: 13)).foregroundStyle(.haze).multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true).padding(.top, 6)
                RoomMoveList(room: room)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 2)
                    .background(Palette.silk.opacity(0.05), in: .rect(cornerRadius: 12))
                    .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Palette.glassEdge, lineWidth: 1))
                    .padding(.top, 14)
                Text(RoomCopy.moveFoot(RoomModel.clock(room.position(at: .nowMs))))
                    .font(.system(size: 12)).foregroundStyle(.haze).multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true).padding(.top, 10)
                HStack(spacing: 8) {
                    Button(RoomCopy.cancel) { room.moving = false }
                        .buttonStyle(RoomButtonStyle(kind: .secondary, height: 32, wide: true))
                        .keyboardShortcut(.cancelAction)
                    Button(RoomCopy.moveWith(room.moveTicks.count)) { room.move() }
                        .buttonStyle(RoomButtonStyle(kind: .primary, height: 32))
                        .fixedSize()
                        .disabled(room.moveTicks.isEmpty)
                        .keyboardShortcut(.defaultAction)
                }
                .padding(.top, 14)
            }
            .foregroundStyle(.silk)
            .padding(.top, 22)
            .padding(.horizontal, 20)
            .padding(.bottom, 16)
            .frame(width: 340)
            .roomDialogGlass(radius: 26)
        }
        .transition(.opacity)
    }
}

// MARK: The QR code

struct RoomQRCode: View {
    let link: String
    var size: CGFloat = 112

    var body: some View {
        Group {
            if let image = Self.image(link) {
                Image(decorative: image, scale: 1).interpolation(.none).resizable()
            } else {
                Color.clear
            }
        }
        .frame(width: size, height: size)
        .padding(6)
        .background(Palette.silk, in: .rect(cornerRadius: 8))
        .accessibilityLabel(RoomCopy.qrLabel)
    }

    static func image(_ text: String) -> CGImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(text.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else { return nil }
        let colored = output.applyingFilter("CIFalseColor", parameters: [
            "inputColor0": CIColor(red: 0x16 / 255, green: 0x1A / 255, blue: 0x2E / 255),
            "inputColor1": CIColor(red: 0xE4 / 255, green: 0xE0 / 255, blue: 0xD6 / 255),
        ])
        return CIContext().createCGImage(colored, from: colored.extent)
    }
}

// MARK: Up next and Skip

/// The room's Up next card, in the player's bottom trailing corner.
struct RoomUpNextCard: View {
    let room: RoomModel
    let up: RoomUpNext
    var art: Image?

    var body: some View {
        RoomClock { now in
            HStack(spacing: 12) {
                RoundedRectangle(cornerRadius: 8).fill(Palette.artPlaceholder)
                    .overlay { art?.resizable().aspectRatio(contentMode: .fill) }
                    .frame(width: 96, height: 54)
                    .clipShape(.rect(cornerRadius: 8))
                VStack(alignment: .leading, spacing: 0) {
                    Text(RoomCopy.upNextForRoom).font(.system(size: 12)).foregroundStyle(.haze)
                    Text(RoomCopy.upNextEpisode(up.episode.episode, up.episode.name))
                        .font(.system(size: 14, weight: .semibold)).lineLimit(1)
                    Text(Self.when(up, now: now))
                        .font(.system(size: 12)).foregroundStyle(.haze).monospacedDigit()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(10)
            .frame(width: 300)
            .background(.ultraThinMaterial, in: .rect(cornerRadius: 16))
            .background(Palette.glass.opacity(0.6), in: .rect(cornerRadius: 16))
            .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
            .foregroundStyle(.silk)
            .contextMenu {
                Button(RoomCopy.cancel) { room.cancelUpNext() }
                Button(RoomCopy.playNextNow) { room.playNextNow() }
            }
        }
    }

    /// "Plays for everyone in 8 s", or whom it waits for.
    static func when(_ up: RoomUpNext, now: Int64) -> String {
        guard let ends = up.endsAtMs else { return RoomCopy.waitingToFinish(up.waitingFor) }
        let seconds = Int((Double(ends - now) / 1000).rounded(.up))
        return RoomCopy.playsForEveryoneIn(max(0, seconds))
    }
}
