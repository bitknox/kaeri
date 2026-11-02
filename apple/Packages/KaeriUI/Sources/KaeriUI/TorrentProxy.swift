// The optional proxy for torrent traffic: the words for it, the form's draft, and the rows that
// show its check. A proxy, new or changed, is saved only once the check through it passes.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The words for the proxy, as its form and the Torrents settings say them.
enum ProxyWords {
    static var checkNote: String { L.torrentsCheckNote }
    /// Edit proxy's line over its form.
    static var editNote: String { L.torrentsProxyKeepsSaved }
    /// Under the proxy's fields, on Edit proxy.
    static var passwordKept: String { L.torrentsPasswordKeep }
    /// While the check through a proxy runs.
    static var checkingProxy: String { L.torrentsCheckingProxy }
    /// Why a proxy was not saved.
    static var notSaved: String { L.torrentsProxyNotSaved(why: L.torrentsCheckNotMadeProxy) }

    /// "proxy.example:1080 · as mika": where the saved proxy is, and who it lets in.
    static func proxyLine(_ proxy: ProxyView) -> String {
        let place = "\(proxy.host):\(proxy.port)"
        guard let user = proxy.user, !user.isEmpty else { return place }
        return L.torrentsProxyLineUser(address: place, user: user)
    }

    /// The line under the torrent switch: off, direct, or through the saved proxy.
    static func summary(on: Bool, proxy: ProxyView?) -> String {
        guard on else { return L.torrentsOffDetail }
        guard let proxy else { return L.torrentsSummaryDirect }
        return L.torrentsSummaryProxyAt(host: proxy.host, port: String(proxy.port))
    }

    /// The saved proxy's check in words: a line when it needs one, a word for how it stands, and the dot's colour.
    static func words(_ status: ProxyStatus?) -> (String?, String, Color) {
        switch status {
        case .checking?: (nil, L.torrentsCheckRunning, .haze)
        case .passed?: (nil, L.torrentsCheckPassed, .water)
        case .failed?: (L.torrentsCheckFailedLine, L.torrentsCheckFailed, .vermilion)
        case .direct?, nil: (nil, L.torrentsCheckWaiting, .haze)
        }
    }
}

extension TorrentView {
    /// Where the saved proxy is, while torrents go through one.
    var proxyLine: String? { proxy.map(ProxyWords.proxyLine) }
}

/// Why the proxy form is open: a proxy being added, or the saved one's details.
enum ProxyFormMode: Equatable {
    case add, edit
}

/// The proxy form's fields before they are sent. The password left empty keeps the one already
/// kept.
struct ProxyDraft: Equatable {
    var host: String
    var port: String
    var user: String
    var password = ""

    @MainActor init(_ store: AppStore) {
        let proxy = store.settings.torrent?.proxy
        host = proxy?.host ?? ""
        port = proxy.map { String($0.port) } ?? ""
        user = proxy?.user ?? ""
    }

    /// The proxy as the engine takes it, when the fields make one.
    var proxy: TorrentProxy? {
        guard !host.trimmingCharacters(in: .whitespaces).isEmpty,
              let port = UInt16(port.trimmingCharacters(in: .whitespaces)), port > 0
        else { return nil }
        return TorrentProxy(
            host: host.trimmingCharacters(in: .whitespaces), port: port,
            user: user.isEmpty ? nil : user, password: password.isEmpty ? nil : password)
    }

    /// Whether it can be sent: a proxy needs its host and port.
    var complete: Bool { proxy != nil }

    /// Whether anything was typed that cannot make a proxy yet.
    var invalid: Bool { !complete && !(host.isEmpty && port.isEmpty) }
}

extension AppStore {
    /// Save the form: the proxy is checked first, and saved by `ProxySaving` once it passes.
    func check(_ draft: ProxyDraft) {
        guard let proxy = draft.proxy else { return }
        checkTorrentProxy(proxy)
    }
}

/// A proxy form saving a proxy: once the check through `checking` passes, it is saved and the
/// form goes; a failed one saved nothing, and the form stays to say why. Changing the form after
/// a failed check clears what it said. VoiceOver hears the check start and fail, as the line
/// under the form shows it.
struct ProxySaving: ViewModifier {
    @Environment(AppStore.self) private var store
    let checking: ProxyDraft?
    let draft: ProxyDraft?
    let saved: () -> Void

    func body(content: Content) -> some View {
        content
            .onChange(of: store.providers.trial) { _, trial in
                if trial == .checking {
                    Self.announce(ProxyWords.checkingProxy, urgent: false)
                } else if trial?.failed == true {
                    Self.announce(ProxyWords.notSaved, urgent: true)
                }
                guard trial?.passed == true, let proxy = checking?.proxy else { return }
                store.providers.clearTrial()
                store.setTorrentProxy(proxy)
                saved()
            }
            .onChange(of: draft) { before, _ in
                // Not the form's first filling in, as the form opens.
                if before != nil, store.providers.trial?.failed == true { store.providers.clearTrial() }
            }
            .onDisappear { store.providers.clearTrial() }
    }

    private static func announce(_ text: String, urgent: Bool) {
        var words = AttributedString(text)
        words.accessibilitySpeechAnnouncementPriority = urgent ? .high : .default
        AccessibilityNotification.Announcement(words).post()
    }
}

/// Where the form's check stands: under way, or why nothing was saved. Nothing before a check,
/// or once one passed.
struct ProxyCheckLine: View {
    let trial: ProxyTrial?
    var phone = false

    /// Whether there is anything to say.
    static func shows(_ trial: ProxyTrial?) -> Bool {
        trial == .checking || trial?.failed == true
    }

    var body: some View {
        switch trial {
        case .checking?:
            HStack(spacing: 8) {
                Working(true)
                Text(ProxyWords.checkingProxy).foregroundStyle(.haze)
            }
            .font(phone ? .ui(15) : .mac(12))
            .accessibilityElement(children: .combine)
        case .said?:
            if trial?.failed == true {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Circle()
                        .fill(.vermilion)
                        .frame(width: phone ? 8 : 6, height: phone ? 8 : 6)
                        .alignmentGuide(.firstTextBaseline) { $0[VerticalAlignment.center] + (phone ? 5 : 4) }
                    Text(ProxyWords.notSaved)
                        .foregroundStyle(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .font(phone ? .ui(15) : .mac(12))
                .accessibilityElement(children: .combine)
            }
        case nil:
            EmptyView()
        }
    }
}

/// Edit proxy, beside the route switch once a proxy is set: opens the proxy's form, filled in.
struct EditProxyButton: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var editing = false

    var body: some View {
        if store.settings.torrent?.proxy != nil {
            Button(L.torrentsEditProxy) { editing = true }
                .sheet(isPresented: $editing) { ProxySheet(mode: .edit) }
                .onLaunchSheet("proxy", navigator) { editing = true }
        }
    }
}

/// The saved proxy's check, in a Mac settings row.
struct ProxyCheckRow: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let (note, status, color) = ProxyWords.words(store.providers.proxyStatus)
        SettingsRow(title: L.torrentsLastCheck, note: note) {
            HStack(spacing: 10) {
                HStack(spacing: 6) {
                    Circle().fill(color).frame(width: 6, height: 6)
                    Text(status).font(.mac(13)).foregroundStyle(.silk)
                }
                Button(L.torrentsCheckNow) { store.checkProxy() }
                    .disabled(store.providers.proxyStatus == .checking)
            }
        }
    }
}
