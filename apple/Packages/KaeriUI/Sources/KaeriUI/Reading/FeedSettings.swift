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
                    Text(feed?.addresses.first ?? L.opdsAddressOff)
                        .font(.mac(12, design: feed?.addresses.first == nil ? .default : .monospaced))
                        .foregroundStyle(feed?.addresses.first == nil ? .haze : .silk)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .textSelection(.enabled)
                    if let address = feed?.addresses.first {
                        copyButton(address)
                    }
                }
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
                    Button(L.opdsNewPassword) {
                        sending.setFeed(on: feed?.on ?? false, password: SendModel.newPassword())
                    }
                    .buttonStyle(SecondaryCapsuleStyle(height: 24, raisedSurface: true, fontSize: 12))
                    .fixedSize()
                    .disabled(feed == nil)
                }
            }
            SettingsRow(title: L.opdsPort, note: portNote(feed)) {
                TextField(L.opdsPort, text: $port, prompt: Text(verbatim: "8484")) // i18n-skip
                    .textFieldStyle(.plain)
                    .font(.mac(12, design: .monospaced))
                    .multilineTextAlignment(.trailing)
                    .frame(width: 70)
                    .onSubmit { savePort() }
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
                ForEach(feed?.clients ?? [], id: \.address) { client in
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
                // The list of who fetched changes while the pane is open.
                while !Task.isCancelled {
                    sending.loadFeed()
                    try? await Task.sleep(for: .seconds(4))
                }
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

    private func savePort() {
        let digits = port.filter(\.isNumber)
        port = digits
        store.setSetting(.opdsPort, to: .str(digits))
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
#endif
