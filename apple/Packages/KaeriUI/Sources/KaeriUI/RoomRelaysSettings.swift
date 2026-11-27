// Settings ▸ Playback ▸ Watch together ▸ Relays: the relays new rooms use, with the spares a room
// swaps in when its own stop answering. The list for new rooms takes adds and removes (never the
// last); a spare comes off with its ×. Restore the defaults puts the built-in ones back.

import KaeriDesign
import KaeriStore
import KaeriStrings
import SwiftUI

/// The Relays row of the Watch together group: how many relays new rooms use, and Edit ›.
struct RelaysRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        let room = store.room
        SettingsRow(
            title: L.roomRelays,
            note: room.relaySettings == nil ? nil : L.roomSettingsRelaysCount(count: room.newRoomRelays.count)
        ) {
            Button { navigator.settingsRelays = true } label: {
                HStack(spacing: 4) {
                    Text(L.roomSettingsRelaysEdit)
                    Image(systemName: "chevron.right").font(.system(size: 10, weight: .semibold))
                }
                .font(.mac(13))
                .foregroundStyle(.haze)
                .padding(.horizontal, 6)
                .frame(minHeight: 28)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
        }
        .task { if room.relaySettings == nil { room.loadRelays() } }
    }
}

/// The Relays page in the Mac and iPad Settings pane, in place of Playback's groups.
struct RelaysPane: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var adding = "wss://"
    @FocusState private var typing: Bool

    var body: some View {
        let room = store.room
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 6) {
                Button { navigator.settingsRelays = false } label: {
                    Image(systemName: "chevron.left").font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.silk)
                        .frame(width: 28, height: 28)
                        .background(Palette.silk.opacity(0.10), in: .circle)
                        .contentShape(.circle)
                }
                .buttonStyle(.plain)
                .keyboardShortcut(.cancelAction)
                .accessibilityLabel(L.actionBack)
                Text(L.roomRelays).font(.mac(20, weight: .bold)).accessibilityAddTraits(.isHeader)
            }
            Text(L.roomRelaysIntro)
                .font(.mac(13)).macLineHeight(19).foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 8)
            SettingsGroup(title: L.roomRelaysNewRooms, footnote: L.roomRelaysNewRoomsLine) {
                ForEach(room.newRoomRelays, id: \.self) { relay in
                    RelayRow(relay: relay, removable: room.newRoomRelays.count > 1) { room.removeRelay(relay) }
                }
                HStack(spacing: 8) {
                    TextField("", text: $adding)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12, design: .monospaced))
                        .padding(.horizontal, 10)
                        .frame(height: 26)
                        .background(Palette.silk.opacity(0.06), in: .rect(cornerRadius: 6))
                        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Palette.glassRule, lineWidth: 1))
                        .focused($typing)
                        #if os(macOS)
                        // The page opens with nothing focused: the first responder the window
                        // picks is let go.
                        .task {
                            try? await Task.sleep(for: .milliseconds(80))
                            if typing { typing = false }
                        }
                        #endif
                        .onSubmit(add)
                        .accessibilityLabel(L.roomRelayAdd)
                    Button(L.roomRelayAdd, action: add)
                        .buttonStyle(SecondaryCapsuleStyle(height: 26, fontSize: 12))
                        .disabled(RoomRelays.address(adding) == nil)
                }
                .padding(EdgeInsets(top: 6, leading: 14, bottom: 6, trailing: 12))
            }
            .padding(.top, 16)
            if let spares = room.relaySettings?.spares, !spares.isEmpty {
                SettingsGroup(title: L.roomRelaysSpares, footnote: L.roomRelaysSparesLine) {
                    ForEach(spares, id: \.self) { spare in
                        RelayRow(relay: spare, removable: true) { room.removeSpare(spare) }
                    }
                }
                .padding(.top, 20)
            }
            Button(L.roomRelaysRestore) { room.restoreRelays() }
                .buttonStyle(SecondaryCapsuleStyle(height: 28, fontSize: 13))
                .disabled(room.relaysAreDefault)
                .padding(.top, 16)
        }
        .task { room.loadRelays() }
    }

    private func add() {
        if store.room.addRelay(adding) { adding = "wss://" }
    }
}

/// A relay's address in mono, whole, with × to take it off the list when it can be.
private struct RelayRow: View {
    let relay: String
    var removable = false
    var remove: (() -> Void)?

    var body: some View {
        let shown = RoomRelays.shown(relay)
        HStack(spacing: 12) {
            Text(shown)
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(.silk)
                // A long address wraps whole: there's no other way to read it.
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let remove {
                Button(action: remove) {
                    Image(systemName: "xmark").font(.system(size: 11, weight: .semibold)).foregroundStyle(.haze)
                        .frame(width: 24, height: 24).contentShape(.rect)
                }
                .buttonStyle(.plain)
                .disabled(!removable)
                .opacity(removable ? 1 : 0.4)
                .accessibilityLabel(L.roomRelayRemove(relay: shown))
            }
        }
        .padding(EdgeInsets(top: 6, leading: 14, bottom: 6, trailing: 10))
        .frame(minHeight: 36)
    }
}

#if os(iOS)
/// The iPhone's Relays page in the Settings sheet: the same lists at 44 pt targets.
struct PhoneRelaysPage: View {
    @Environment(AppStore.self) private var store
    @State private var adding = "wss://"

    var body: some View {
        let room = store.room
        PhoneList {
            Section {
                ForEach(room.newRoomRelays, id: \.self) { relay in
                    PhoneRelayRow(relay: relay, removable: room.newRoomRelays.count > 1) { room.removeRelay(relay) }
                }
                HStack(spacing: 8) {
                    TextField("", text: $adding)
                        .font(.system(size: 14, design: .monospaced))
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .padding(.horizontal, 10)
                        .frame(height: 36)
                        .background(Palette.silk.opacity(0.06), in: .rect(cornerRadius: 10))
                        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Palette.glassRule, lineWidth: 1))
                        .onSubmit(add)
                        .accessibilityLabel(L.roomRelayAdd)
                    Button(L.roomRelayAdd, action: add)
                        .buttonStyle(SecondaryCapsuleStyle(height: 36, fontSize: 15))
                        .disabled(RoomRelays.address(adding) == nil)
                }
                .listRowBackground(Palette.raised)
            } header: {
                VStack(alignment: .leading, spacing: 18) {
                    Text(L.roomRelaysIntroShort)
                        .font(.system(size: 15)).foregroundStyle(.haze).textCase(nil)
                        .fixedSize(horizontal: false, vertical: true)
                    PhoneSectionHeader(L.roomRelaysNewRooms)
                }
            }
            if let spares = room.relaySettings?.spares, !spares.isEmpty {
                Section {
                    ForEach(spares, id: \.self) { spare in
                        PhoneRelayRow(relay: spare, removable: true) { room.removeSpare(spare) }
                    }
                } header: {
                    PhoneSectionHeader(L.roomRelaysSpares)
                } footer: {
                    Text(L.roomRelaysSparesLineShort)
                }
            }
            Section {
                Button(L.roomRelaysRestore) { room.restoreRelays() }
                    .font(.system(size: 17))
                    .foregroundStyle(room.relaysAreDefault ? Palette.haze : Palette.amber)
                    .disabled(room.relaysAreDefault)
                    .frame(maxWidth: .infinity, minHeight: 44)
                    .listRowBackground(Color.clear)
            }
        }
        .navigationTitle(L.roomRelays)
        .navigationBarTitleDisplayMode(.inline)
        .task { room.loadRelays() }
    }

    private func add() {
        if store.room.addRelay(adding) { adding = "wss://" }
    }
}

private struct PhoneRelayRow: View {
    let relay: String
    var removable = false
    var remove: (() -> Void)?

    var body: some View {
        let shown = RoomRelays.shown(relay)
        HStack(spacing: 8) {
            Text(shown)
                .font(.system(size: 15, design: .monospaced))
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let remove {
                Button(action: remove) {
                    Image(systemName: "xmark").font(.system(size: 15, weight: .semibold)).foregroundStyle(.haze)
                        .frame(width: 44, height: 44).contentShape(.rect)
                }
                .buttonStyle(.plain)
                .disabled(!removable)
                .opacity(removable ? 1 : 0.4)
                .accessibilityLabel(L.roomRelayRemove(relay: shown))
            }
        }
        .listRowBackground(Palette.raised)
    }
}
#endif
