// The E-readers group of the Reading settings, and on the Mac the OPDS feed it opens: whether
// e-readers at home can browse this Mac's downloads and folders, the address and password they
// are given, what is shared and who fetched what. The password is the engine's to keep; it is
// shown and copied here and never written to a log.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

enum FeedWords {
    /// "Today 08:12", or the weekday within the last week, else the date.
    static func when(_ seconds: Int64, now: Date = .now) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let calendar = Calendar.current
        if calendar.isDateInToday(date) || calendar.isDateInYesterday(date) {
            let style = DateFormatter()
            style.locale = AirTime.locale
            style.dateStyle = .medium
            style.timeStyle = .short
            style.doesRelativeDateFormatting = true
            return style.string(from: date)
        }
        if now.timeIntervalSince(date) < 6 * 86_400 {
            return date.formatted(.dateTime.weekday(.wide).locale(AirTime.locale))
        }
        return date.formatted(.dateTime.day().month(.abbreviated).locale(AirTime.locale))
    }

    /// The password in the groups of four a person reads off to type it: "4821 9930".
    static func spaced(_ password: String) -> String {
        stride(from: 0, to: password.count, by: 4).map { start in
            let from = password.index(password.startIndex, offsetBy: start)
            let to = password.index(from, offsetBy: min(4, password.count - start))
            return String(password[from..<to])
        }.joined(separator: " ") // i18n-skip
    }

    static func name(_ shelf: FeedShelf) -> String {
        switch shelf {
        case .downloads: L.opdsShareDownloads
        case .mangaFolders: L.opdsShareManga
        case .novelFolders: L.opdsShareNovels
        }
    }

    static func count(_ share: FeedShare) -> String {
        switch share.shelf {
        case .downloads: L.opdsShareDownloadsCount(n: Int(share.count))
        case .mangaFolders, .novelFolders: L.opdsShareSeriesCount(n: Int(share.count))
        }
    }
}

// MARK: - Send to e-reader's row

/// The e-reader Send to e-reader offers first, chosen on this device from the ones the engine lists.
struct SendTargetMenu: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let sending = store.reading.sending
        let current = sending.defaultTarget
        Menu {
            Picker(L.ereadersSendRow, selection: Binding(
                get: { current?.id },
                set: { if let id = $0 { sending.choose(target: id) } }
            )) {
                ForEach(sending.targets, id: \.id) { Text($0.name).tag(Optional($0.id)) }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            MenuValue(text: current?.name ?? "–", phone: !Device.isMac && !Device.isPad) // i18n-skip
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .disabled(current == nil)
        .accessibilityLabel(L.ereadersSendRow)
        .task { sending.loadTargets() }
    }
}

// MARK: - The Mac's feed

#if os(macOS)
import AppKit

/// The OPDS feed's settings, in the Reading pane under a way back to it.
struct FeedPane: View {
    @Environment(AppStore.self) private var store
    let back: () -> Void
    @State private var port = ""
    @State private var copied: String?
    @State private var askingNewPassword = false
    @FocusState private var portFocused: Bool

    var body: some View {
        let sending = store.reading.sending
        let feed = sending.feed
        Button(action: back) {
            HStack(spacing: 6) {
                Image(systemName: "chevron.left").font(.mac(11, weight: .semibold))
                Text(L.settingsGroupReading).font(.mac(13))
            }
            .foregroundStyle(.silk)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityLabel(L.settingsGroupReading)
        SettingsGroup(title: L.ereadersFeedRow, footnote: L.opdsFooter(device: Device.name)) {
            SettingsRow(title: L.settingsOpdsFeed, note: L.opdsShareNote) {
                Toggle(L.settingsOpdsFeed, isOn: Binding(
                    get: { feed?.on ?? false },
                    set: { on in
                        // The feed never starts without a password, so the first time makes one.
                        sending.setFeed(on: on, password: on && feed?.passwordSet != true ? SendModel.newPassword() : nil)
                    }
                ))
                .toggleStyle(.switch)
                .tint(.amber)
                .labelsHidden()
                .disabled(feed == nil)
            }
            SettingsRow(title: L.opdsAddress) {
                HStack(spacing: 8) {
                    Text(addressWords(feed))
                        .font(.mac(12, design: feed?.addresses.first == nil ? .default : .monospaced))
                        .foregroundStyle(feed?.addresses.first == nil ? .haze : .silk)
                        .lineLimit(2)
                        .multilineTextAlignment(.trailing)
                        .truncationMode(.middle)
                        .textSelection(.enabled)
                        .id(addressWords(feed))
                        .transition(.opacity)
                    if let address = feed?.addresses.first {
                        copyButton(address)
                    }
                }
                .animation(.standard, value: addressWords(feed))
            }
            SettingsRow(title: L.opdsPassword, note: L.opdsPasswordNote) {
                HStack(spacing: 8) {
                    if let password = feed?.password {
                        Text(FeedWords.spaced(password))
                            .font(.mac(12, design: .monospaced))
                            .foregroundStyle(.silk)
                            .textSelection(.enabled)
                        copyButton(password)
                    }
                    if feed?.passwordSet == true {
                        Button(L.opdsNewPassword) { askingNewPassword = true }
                            .buttonStyle(SecondaryCapsuleStyle(height: 24, raisedSurface: true, fontSize: 12))
                            .fixedSize()
                            .confirmationDialog(L.opdsNewPasswordTitle, isPresented: $askingNewPassword, titleVisibility: .visible) {
                                Button(L.opdsNewPassword) {
                                    sending.setFeed(on: feed?.on ?? false, password: SendModel.newPassword())
                                }
                                Button(L.actionCancel, role: .cancel) {}
                            } message: {
                                Text(L.opdsNewPasswordWarning)
                            }
                    }
                }
            }
            SettingsRow(title: L.opdsPort, note: portNote(feed)) {
                TextField(L.opdsPort, text: $port, prompt: Text(verbatim: "8484")) // i18n-skip
                    .textFieldStyle(.plain)
                    .font(.mac(12, design: .monospaced))
                    .multilineTextAlignment(.trailing)
                    .focused($portFocused)
                    .frame(width: 64)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 3)
                    .background(.faintRule.opacity(0.5), in: .rect(cornerRadius: 6))
                    .overlay {
                        RoundedRectangle(cornerRadius: 6)
                            .strokeBorder(portFocused ? Palette.amber : .clear, lineWidth: 1.5)
                    }
                    .onSubmit { savePort() }
                    .onChange(of: portFocused) { _, focused in if !focused { savePort() } }
                    .accessibilityLabel(L.opdsPort)
            }
        }
        SettingsGroup(title: L.opdsShares) {
            ForEach(feed?.shares ?? [], id: \.shelf) { share in
                SettingsRow(title: FeedWords.name(share.shelf), note: FeedWords.count(share)) {
                    Toggle(FeedWords.name(share.shelf), isOn: Binding(
                        get: { share.on },
                        set: { sending.setShare(share.shelf, on: $0) }
                    ))
                    .toggleStyle(.checkbox)
                    .tint(.amber)
                    .labelsHidden()
                }
            }
        }
        if feed?.on == true {
            SettingsGroup(title: L.opdsFetchedBy) {
                if feed?.clients.isEmpty ?? true {
                    SettingsRow(title: L.opdsFetchedNone)
                }
                ForEach(feed?.clients ?? [], id: \.rowKey) { client in
                    SettingsRow(
                        title: client.name.isEmpty ? L.opdsReaderUnnamed(address: client.address) : client.name,
                        note: L.opdsFetchedFiles(n: Int(client.files))
                    ) {
                        Text(FeedWords.when(client.last))
                            .font(.mac(12))
                            .foregroundStyle(.haze)
                            .monospacedDigit()
                    }
                }
            }
        }
        Color.clear.frame(height: 0)
            .task {
                port = store.settings.row(.opdsPort)?.value ?? ""
                if sending.stageFeed() { return }
                // The engine tells the pane when the feed or who fetched changes.
                sending.loadFeed()
            }
    }

    private func copyButton(_ text: String) -> some View {
        Button(copied == text ? L.actionDone : L.actionCopy) {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            copied = text
            Task {
                try? await Task.sleep(for: .seconds(2))
                if copied == text { copied = nil }
            }
        }
        .buttonStyle(SecondaryCapsuleStyle(height: 24, raisedSurface: true, fontSize: 12))
        .fixedSize()
        .animation(nil, value: copied)
    }

    /// Kept as digits in 1024 to 65535, the ports a program may listen on; anything else is
    /// emptied, which picks a free one.
    private func savePort() {
        let digits = port.filter(\.isNumber)
        let kept = UInt16(digits).flatMap { (1024...65535).contains($0) ? String($0) : nil } ?? ""
        port = kept
        store.setSetting(.opdsPort, to: .str(kept))
    }

    /// What the address row says: the address, why there is none, or that it is starting.
    private func addressWords(_ feed: FeedView?) -> String {
        guard let feed, feed.on else { return L.opdsAddressOff }
        if let address = feed.addresses.first { return address }
        switch feed.pause {
        case .noNetwork?: return L.opdsPausedNone
        case .vpnOnly?: return L.opdsPausedVpn
        case nil: return L.opdsStarting
        }
    }

    /// What the port row says under it: that a free one is picked when it is empty, or that the
    /// one asked for was busy.
    private func portNote(_ feed: FeedView?) -> String {
        if let feed, feed.on, let wanted = UInt16(port), wanted != 0, feed.port != wanted {
            return L.opdsPortMoved(wanted: Int(wanted), port: Int(feed.port))
        }
        return port.isEmpty ? L.opdsPortNote : ""
    }
}

private extension FeedClient {
    /// Two readers may share an address, or one may change its name.
    var rowKey: String { address + "|" + name } // i18n-skip
}
#endif
