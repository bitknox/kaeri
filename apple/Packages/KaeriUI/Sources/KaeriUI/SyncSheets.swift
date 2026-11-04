// The sheets of sync: how to share, which cloud folder, the recovery key, a code to pair with, and
// the wait for a sign-in page. One set for the three Apple devices, drawn by SyncKit.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI
import UniformTypeIdentifiers
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Presents whichever sheet `sync.sheet` names, and runs the web sign-in the engine asks for.
struct SyncPresenter: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(\.webSignIn) private var webSignIn

    func body(content: Content) -> some View {
        @Bindable var sync = store.sync
        content
            .sheet(item: $sync.sheet) { sheet in
                SyncSheetHost(sheet: sheet)
                    .environment(store)
                    #if os(iOS)
                    .presentationDetents([.large])
                    .presentationBackground(Palette.ground)
                    .interactiveDismissDisabled(store.sync.holdsSheet)
                    #endif
            }
            .task(id: WebPage(request: store.sync.signIn, opens: store.sync.pageOpens)) { await runWebSignIn() }
    }

    /// The page shown for a sign-in: a new step, or Open the page again, opens it anew.
    private struct WebPage: Equatable {
        let request: SyncSignInRequest?
        let opens: Int
    }

    private func runWebSignIn() async {
        let sync = store.sync
        let opens = sync.pageOpens
        guard let request = sync.signIn, case .sheet(let url, let scheme) = request.step,
              let link = URL(string: url) else { return }
        do {
            let callback: URL
            if let system = webSignIn as? SystemWebSignIn {
                callback = try await system.signIn(url: link, callbackScheme: scheme, shared: true)
            } else {
                callback = try await webSignIn.signIn(url: link, callbackScheme: scheme)
            }
            sync.signInReturned(request.service)
            sync.send(.finishSyncSignIn(service: request.service, callback: callback.absoluteString))
        } catch let error as NSError {
            // The page closed because its request moved on: a newer page took its place, it was
            // opened again, or the engine ended it and has said why. Only a page whose view went
            // while it was still the engine's request ends that sign-in here.
            if Task.isCancelled {
                if sync.signIn == request, sync.pageOpens == opens { sync.cancelSignIn() }
                return
            }
            // Closing the page is the person's choice, so nothing is said; anything else is a refusal.
            let closed = error.domain == "com.apple.AuthenticationServices.WebAuthenticationSession" && error.code == 1
            if closed { sync.signInClosed() } else { sync.signInRefused(request.service) }
        }
    }
}

private struct SyncSheetHost: View {
    let sheet: SyncSheet

    var body: some View {
        switch sheet {
        case .picker: CloudPickerSheet()
        case .nextcloud: NextcloudSheet()
        case .webDav: WebDavSheet()
        case .recoveryKey: RecoveryKeySheet()
        case .restore(let kind): RestoreSheet(kind: kind)
        case .pairEnter: PairEnterSheet()
        case .pairShow: PairShowSheet()
        case .profileAdd: ProfileSheet(editing: nil)
        case .profileEdit(let id): ProfileSheet(editing: id)
        }
    }
}

private extension SyncModel {
    func close() { sheet = nil }
}

// MARK: Cloud picker

/// The cloud folder's sheet: the list of services, then, in the same sheet, the wait for the
/// chosen one's page, the sign-in finishing, the folder being made, and the recovery key or what
/// went wrong. Each step crossfades into the next.
struct CloudPickerSheet: View {
    @Environment(AppStore.self) private var store
    @State private var importing = false

    var body: some View {
        let sync = store.sync
        ZStack {
            Group {
                switch sync.cloudStep {
                case nil: list(sync)
                case .key?: RecoveryKeySheet()
                case let step?: CloudStepView(step: step, deviceName: name)
                }
            }
            .id(Self.phase(sync.cloudStep))
            .transition(.opacity)
        }
        .animation(.standard, value: Self.phase(sync.cloudStep))
        .onDisappear { store.sync.pickerClosed() }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.folder]) { result in
            if case .success(let url) = result {
                _ = url.startAccessingSecurityScopedResource()
                sync.startDirectory(url.path, deviceName: name)
            }
        }
    }

    private static func phase(_ step: CloudStep?) -> String {
        switch step {
        case nil: "list"
        case .waiting?: "waiting"
        case .connecting?: "connecting"
        case .settingUp?: "setting-up"
        case .failed?: "failed"
        case .key?: "key"
        }
    }

    @ViewBuilder
    private func list(_ sync: SyncModel) -> some View {
        let current = sync.switching ? sync.status.folder?.kind : nil
        SyncSheetFrame(title: L.cloudPickTitle, cancel: { sync.close() }) {
            SyncSection(footer: current.map { L.cloudSwitchFoot(service: SyncModel.name($0)) + " " + L.cloudPickFoot } ?? L.cloudPickFoot) {
                ForEach(sync.offered, id: \.service) { view in
                    let inUse = current == view.service
                    SyncChoice(title: SyncModel.name(view.service), line: inUse ? L.cloudInUse : line(view.service), glyph: glyph(view.service)) {
                        sync.choose(view.service, deviceName: name)
                    }
                    .disabled(inUse)
                    .opacity(inUse ? 0.38 : 1)
                    if case .unavailable = view.way {
                        SyncNote(text: L.tvCloudNeedsPhoneNone)
                    }
                }
            }
            SyncSection(header: L.cloudOther) {
                SyncChoice(title: L.cloudWebdav, line: current == .webDav ? L.cloudInUse : L.cloudWebdavLine, glyph: "server.rack") {
                    sync.sheet = .webDav
                }
                .disabled(current == .webDav)
                .opacity(current == .webDav ? 0.38 : 1)
                SyncChoice(title: L.cloudFolder(kind: Device.name), line: L.cloudFolderLine, glyph: "folder") {
                    importing = true
                }
            }
        } bottom: {
            #if os(macOS)
            SyncButton(title: L.actionCancel) { sync.close() }
            #endif
        }
    }

    private var name: String { store.sync.deviceName.isEmpty ? DeviceNaming.system : store.sync.deviceName }

    private func line(_ kind: SyncFolderKind) -> String {
        switch kind {
        case .googleDrive: L.cloudGoogleLine
        case .dropbox: L.cloudDropboxLine
        case .oneDrive: L.cloudOnedriveLine
        case .nextcloud: L.cloudNextcloudLine
        default: ""
        }
    }

    private func glyph(_ kind: SyncFolderKind) -> String {
        kind == .nextcloud ? "cloud" : "externaldrive.connected.to.line.below"
    }
}

/// A step of the cloud folder's sheet after the list: its title, a line with an indicator or the
/// error's glyph, and its buttons. Past the page's answer Cancel is dimmed: the engine can't stop
/// the token fetch or the first write half way.
private struct CloudStepView: View {
    @Environment(AppStore.self) private var store
    let step: CloudStep
    let deviceName: String

    var body: some View {
        let sync = store.sync
        let busy = step.underWay && !waiting
        SyncSheetFrame(title: title, cancel: { sync.cancelCloudStep() }, cancelDimmed: busy) {
            switch step {
            case .waiting(let kind):
                CloudStatusLine(text: L.cloudWaiting(service: SyncModel.name(kind)), busy: true)
                SyncNote(text: pageLine(kind))
            case .connecting:
                CloudStatusLine(text: L.cloudConnectingLine, busy: true, color: .haze)
            case .settingUp(let kind):
                CloudStatusLine(text: L.cloudSettingUpLine(service: SyncModel.name(kind)), busy: true, color: .haze)
            case .failed(let kind, let problem, let folder):
                CloudStatusLine(text: Self.problemLine(kind, problem, folder: folder), busy: false)
            case .key:
                EmptyView()
            }
        } bottom: {
            #if os(macOS)
            switch step {
            case .waiting:
                HStack(spacing: 10) {
                    SyncButton(title: L.cloudOpenAgain) { sync.openPageAgain() }
                    Spacer(minLength: 0)
                    SyncButton(title: L.actionCancel) { sync.cancelCloudStep() }
                }
                .frame(maxWidth: .infinity)
                .layoutPriority(1)
            case .failed:
                SyncButton(title: L.actionCancel) { sync.cancelCloudStep() }
                SyncButton(title: L.actionTryAgain, primary: true) { sync.tryAgain(deviceName: deviceName) }
            default:
                SyncButton(title: L.actionCancel) {}
                    .disabled(true)
            }
            #else
            switch step {
            case .waiting:
                SyncButton(title: L.cloudOpenAgain, wide: true) { sync.openPageAgain() }
            case .failed:
                SyncButton(title: L.actionTryAgain, primary: true, wide: true) { sync.tryAgain(deviceName: deviceName) }
            default:
                EmptyView()
            }
            #endif
        }
    }

    private var waiting: Bool {
        if case .waiting = step { return true }
        return false
    }

    private var title: String {
        switch step {
        case .waiting(let kind):
            #if os(macOS)
            L.cloudBrowserTitle(service: SyncModel.name(kind))
            #else
            L.cloudSheetWait
            #endif
        case .connecting(let kind): L.cloudConnecting(service: SyncModel.name(kind))
        case .settingUp: L.cloudSettingUp
        case .failed(_, _, folder: true): L.cloudErrSetupTitle
        case .failed(let kind, _, _): L.cloudErrSigninTitle(service: SyncModel.name(kind))
        case .key: L.rkeyTitle
        }
    }

    private func pageLine(_ kind: SyncFolderKind) -> String {
        #if os(macOS)
        L.cloudBrowserLine(service: SyncModel.name(kind))
        #else
        L.cloudSheetLine(service: SyncModel.name(kind))
        #endif
    }

    /// What went wrong, in a line: refused, timed out, offline, or the sign-in or the folder
    /// failing otherwise. The engine's own words go to Messages.
    static func problemLine(_ kind: SyncFolderKind, _ problem: SyncProblem, folder: Bool) -> String {
        let name = SyncModel.name(kind)
        switch problem {
        case .signInDenied: return L.cloudErrDeniedShort(service: name)
        case .signInExpired: return L.cloudErrTimeout
        case .unreachable: return L.cloudErrOffline(service: name)
        default: return folder ? L.cloudErrTitle(service: name) : L.cloudErrSigninOther(service: name)
        }
    }
}

/// The line under a step's title: an indeterminate indicator, or the error's glyph, and words.
private struct CloudStatusLine: View {
    let text: String
    let busy: Bool
    var color: Color = .silk

    var body: some View {
        HStack(alignment: busy ? .center : .firstTextBaseline, spacing: 8) {
            if busy {
                ProgressView().controlSize(.small)
            } else {
                Image(systemName: "exclamationmark.circle.fill")
                    .font(.plat(mac: 13, touch: 17))
                    .foregroundStyle(.vermilion)
                    .accessibilityHidden(true)
            }
            Text(text)
                .font(.plat(mac: 13, touch: 17))
                .foregroundStyle(color)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
        #if os(iOS)
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets(top: 12, leading: 16, bottom: 4, trailing: 16))
        #endif
    }
}

// MARK: Nextcloud and WebDAV

struct NextcloudSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL
    @State private var address = ""
    @State private var host = ""

    var body: some View {
        let sync = store.sync
        let waiting = nextcloudStep(sync)
        SyncSheetFrame(title: waiting != nil ? L.cloudNextcloudWaitTitle(host: host) : "Nextcloud", // i18n-skip
                       cancel: { waiting != nil ? sync.cancelSignIn() : sync.close() }) {
            if let waiting {
                SyncNote(text: L.cloudNextcloudWaitLine)
                HStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    Text(L.cloudWaiting(service: "Nextcloud")) // i18n-skip
                        .font(.plat(mac: 13, touch: 15)).foregroundStyle(.haze)
                }
                if let timeout = sync.problem, case .unreachable = timeout {
                    SyncProblemLine(text: L.cloudErrTimeout)
                }
            } else {
                SyncField(label: L.cloudNextcloudAddress, text: $address, prompt: "cloud.example", // i18n-skip
                          invalid: sync.problem != nil, note: sync.problem == nil ? L.cloudNextcloudAddressNote : nil,
                          contentType: .url)
                if sync.problem != nil { SyncProblemLine(text: L.cloudErrNoServer(host: hostOf(address))) }
            }
        } bottom: {
            if let waiting {
                SyncButton(title: L.actionCancel) { sync.cancelSignIn() }
                SyncButton(title: L.cloudOpenAgain, primary: true) {
                    if let url = URL(string: waiting) { openURL(url) }
                }
            } else {
                #if os(macOS)
                SyncButton(title: L.actionCancel) { sync.close() }
                #endif
                SyncButton(title: L.cloudConnect, primary: true, wide: touch) {
                    host = hostOf(address)
                    sync.startNextcloud(address: address, deviceName: name)
                }
                .disabled(address.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }

    private var touch: Bool {
        #if os(iOS)
        true
        #else
        false
        #endif
    }

    private var name: String { store.sync.deviceName.isEmpty ? DeviceNaming.system : store.sync.deviceName }

    private func nextcloudStep(_ sync: SyncModel) -> String? {
        guard let request = sync.signIn, request.service == .nextcloud else { return nil }
        switch request.step {
        case .browser(let url), .page(let url): return url
        case .finishing: return ""
        default: return nil
        }
    }

    private func hostOf(_ typed: String) -> String {
        let trimmed = typed.trimmingCharacters(in: .whitespaces)
        return URL(string: trimmed.contains("://") ? trimmed : "https://" + trimmed)?.host ?? trimmed
    }
}

struct WebDavSheet: View {
    @Environment(AppStore.self) private var store
    @State private var address = ""
    @State private var user = ""
    @State private var password = ""

    var body: some View {
        let sync = store.sync
        SyncSheetFrame(title: L.cloudWebdav, cancel: { sync.close() }) {
            SyncField(label: L.cloudNextcloudAddress, text: $address, prompt: "dav.example", contentType: .url) // i18n-skip
            SyncField(label: L.serversUser, text: $user, invalid: sync.problem != nil)
            SyncField(label: L.accountsPassword, text: $password, secure: true, invalid: sync.problem != nil)
            if sync.problem != nil { SyncProblemLine(text: L.cloudErrWebdav) }
        } bottom: {
            #if os(macOS)
            SyncButton(title: L.actionCancel) { sync.close() }
            #endif
            SyncButton(title: L.cloudConnect, primary: true, wide: true) {
                sync.startWebDav(address: address, user: user, password: password,
                                 deviceName: sync.deviceName.isEmpty ? DeviceNaming.system : sync.deviceName)
            }
            .disabled(address.isEmpty || user.isEmpty)
        }
    }
}

// MARK: Recovery key

/// The key as the engine says it: letters and digits, in groups of four.
func groupedKey(_ key: String) -> [String] {
    let clean = key.filter { $0.isLetter || $0.isNumber }.uppercased()
    return stride(from: 0, to: clean.count, by: 4).map { start in
        let from = clean.index(clean.startIndex, offsetBy: start)
        return String(clean[from..<clean.index(from, offsetBy: min(4, clean.count - start))])
    }
}

private struct KeyDocument: FileDocument {
    static let readableContentTypes: [UTType] = [.plainText]
    let text: String
    init(text: String) { self.text = text }
    init(configuration: ReadConfiguration) throws { text = "" }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}

struct RecoveryKeySheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var qr = false
    @State private var saving = false
    @State private var copied = false

    var body: some View {
        let sync = store.sync
        let key = sync.recoveryKey ?? ""
        let groups = groupedKey(key)
        SyncSheetFrame(title: L.rkeyTitle) {
            SyncNote(text: L.rkeyLine)
            keyCard(groups)
            SyncSection(footer: sync.keyCameFromDevices ? nil : L.rkeyAgain(kind: Device.name)) {
                SyncChoice(title: copied ? L.actionDone : L.rkeyCopy) {
                    copy(groups.joined(separator: " "))
                }
                #if os(macOS)
                SyncChoice(title: L.rkeySaveMac + "…") { saving = true }
                #else
                SyncChoice(title: L.rkeySaveFiles) { saving = true }
                #endif
                SyncChoice(title: L.rkeyQr) { withAnimation(reduceMotion ? nil : .standard) { qr.toggle() } }
            }
            if qr {
                InviteQR(link: groups.joined(), device: false, caption: false, label: L.rkeyQr)
                    .frame(maxWidth: .infinity)
                    .transition(.opacity)
            }
        } bottom: {
            SyncButton(title: L.actionDone, primary: true, wide: touch) { sync.keySaved() }
        }
        .fileExporter(isPresented: $saving, document: KeyDocument(text: groups.joined(separator: " ") + "\n"),
                      contentType: .plainText, defaultFilename: "Kaeri recovery key") { _ in } // i18n-skip
    }

    private var touch: Bool {
        #if os(iOS)
        true
        #else
        false
        #endif
    }

    private func keyCard(_ groups: [String]) -> some View {
        let rows = stride(from: 0, to: groups.count, by: 3).map { Array(groups[$0..<min($0 + 3, groups.count)]) }
        return VStack(spacing: 6) {
            ForEach(rows.indices, id: \.self) { row in
                HStack(spacing: 18) {
                    ForEach(rows[row], id: \.self) { Text($0) }
                }
            }
        }
        .font(.system(size: touch ? 22 : 20, weight: .medium, design: .monospaced))
        .foregroundStyle(.silk)
        .frame(maxWidth: .infinity)
        .padding(.vertical, 18)
        .background(touch ? Palette.raised : Palette.ground, in: .rect(cornerRadius: 12))
        #if os(iOS)
        .listRowBackground(Color.clear)
        .listRowInsets(EdgeInsets(top: 8, leading: 16, bottom: 8, trailing: 16))
        #endif
        .textSelection(.enabled)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(groups.joined(separator: ", "))
    }

    private func copy(_ text: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #else
        UIPasteboard.general.setItems([[UTType.plainText.identifier: text]], options: [.localOnly: true, .expirationDate: Date().addingTimeInterval(120)])
        #endif
        copied = true
        Task {
            try? await Task.sleep(for: .seconds(2))
            copied = false
        }
    }
}

struct RestoreSheet: View {
    @Environment(AppStore.self) private var store
    let kind: SyncFolderKind
    @State private var key = ""

    var body: some View {
        let sync = store.sync
        let wrong: Bool = { if let problem = sync.problem, case .wrongRecoveryKey = problem { return true } else { return false } }()
        SyncSheetFrame(title: L.rkeyEnterTitle, cancel: { sync.close() }) {
            SyncNote(text: L.rkeyEnterLineGeneric(service: SyncModel.name(kind)))
            SyncField(label: L.rkeyRow, text: $key, prompt: "XXXX XXXX XXXX", invalid: wrong, // i18n-skip
                      note: wrong ? nil : L.rkeyEnterNote)
            if wrong { SyncProblemLine(text: L.rkeyWrong) }
        } bottom: {
            #if os(macOS)
            SyncButton(title: L.actionCancel) { sync.close() }
            #endif
            SyncButton(title: L.rkeyRestore, primary: true, wide: true) {
                sync.join(kind, key: key, deviceName: sync.deviceName.isEmpty ? DeviceNaming.system : sync.deviceName)
            }
            .disabled(key.filter { $0.isLetter || $0.isNumber }.count < 24)
        }
    }
}

// MARK: Pairing

/// This device's address on the local network, for the code the other device types or scans.
enum LocalAddress {
    static var current: String {
        var found = "127.0.0.1" // i18n-skip
        var list: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&list) == 0, let first = list else { return found }
        defer { freeifaddrs(list) }
        for pointer in sequence(first: first, next: { $0.pointee.ifa_next }) {
            let item = pointer.pointee
            guard item.ifa_addr.pointee.sa_family == UInt8(AF_INET), String(cString: item.ifa_name).hasPrefix("en") else { continue }
            var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            if getnameinfo(item.ifa_addr, socklen_t(item.ifa_addr.pointee.sa_len), &host, socklen_t(host.count), nil, 0, NI_NUMERICHOST) == 0 {
                found = String(cString: host)
                break
            }
        }
        return found
    }
}

struct PairShowSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.dismiss) private var dismiss
    /// First launch's "Set up from another device": a new device shows the code, and Restore
    /// from a cloud folder is the other way.
    var first = false
    var restore: () -> Void = {}
    /// Leaving the page, where it is not a sheet of its own (first launch's page).
    var leave: (() -> Void)?
    /// First launch: the pairing is done and the flow ends.
    var finished: () -> Void = {}

    var body: some View {
        let sync = store.sync
        Group {
            if case .paired(let device) = sync.pair {
                PairedContent(device: device, first: first, done: { if first { finished() } else { dismiss() } })
                    .transition(.opacity)
            } else {
                showing(sync)
                    .transition(.opacity)
            }
        }
        .animation(reduceMotion ? nil : .standard, value: sync.pair)
        .task {
            if !sync.staged { sync.resetPair() }
            if !sync.staged { sync.invite(deviceName: sync.deviceName.isEmpty ? DeviceNaming.system : sync.deviceName) }
            // First launch: the device that pairs is asked for invites to its plugin stores, with
            // the pairing, so the switch on the paired page can join them.
            if first, !sync.staged { sync.joinStores(from: nil) }
        }
        .onChange(of: sync.devices.count) { old, new in
            if new > old, sync.invite != nil, let other = sync.others.last { sync.stage(pair: .paired(other.name)) }
        }
        .onDisappear { sync.pairSheetClosed() }
    }

    private func showing(_ sync: SyncModel) -> some View {
        SyncSheetFrame(title: first ? L.firstSetupOther : L.pairShowTitle, cancel: { sync.close(); if let leave { leave() } else { dismiss() } }) {
            if first { SyncNote(text: L.firstSetupLine) }
            if let invite = sync.invite {
                let address = LocalAddress.current
                let code = Text(spaced(invite.code))
                    .font(.system(size: touch ? 40 : 34, weight: .semibold, design: .monospaced))
                    .foregroundStyle(.silk)
                    .accessibilityLabel(invite.code.map(String.init).joined(separator: " "))
                let qr = InviteQR(link: "kaeri://pair/\(invite.code)@\(address):\(invite.port)", device: false, caption: false, label: L.pairShowTitle)
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .center, spacing: 28) { qr; code }
                    VStack(alignment: .center, spacing: 20) { qr; code }
                }
                .frame(maxWidth: .infinity)
                #if os(iOS)
                .listRowBackground(Color.clear)
                #endif
                // Where a network that hides devices from each other can reach this one: a small haze
                // line under the code.
                Text(address + ":" + String(invite.port))
                    #if os(macOS)
                    .font(.mac(11)).macLineHeight(15)
                    #else
                    .font(.ui(12))
                    #endif
                    .monospacedDigit()
                    .foregroundStyle(.haze)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .textSelection(.enabled)
                    #if os(iOS)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets(top: 0, leading: 16, bottom: 4, trailing: 16))
                    #endif
                TimelineView(.periodic(from: .now, by: 1)) { context in
                    let left = max(0, Int(invite.until.timeIntervalSince(context.date)))
                    if left == 0 {
                        SyncProblemLine(text: L.pairErrExpired)
                    } else {
                        SyncNote(text: (first ? L.pairSameWifi : L.pairShowScan) + " " + L.pairGoodFor(left: String(format: "%d:%02d", left / 60, left % 60)))
                            .monospacedDigit()
                    }
                }
                #if os(iOS)
                .listRowBackground(Color.clear)
                #endif
            } else {
                ProgressView().frame(maxWidth: .infinity)
            }
        } bottom: {
            if first {
                #if os(macOS)
                SyncButton(title: L.actionCancel) { sync.close(); leave?() }
                #endif
                SyncButton(title: L.firstRestore, wide: touch) { sync.stopInvite(); restore() }
            } else {
                SyncButton(title: L.actionDone, primary: true, wide: touch) { sync.close() }
            }
        }
    }

    private func spaced(_ code: String) -> String {
        code.count == 6 ? code.prefix(3) + " " + code.suffix(3) : code
    }

    private var touch: Bool {
        #if os(iOS)
        true
        #else
        false
        #endif
    }
}

struct PairEnterSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var code = ""
    @State private var address = ""
    @State private var scanning = false
    @FocusState private var focused: Bool

    var body: some View {
        let sync = store.sync
        Group {
            if case .paired(let device) = sync.pair {
                PairedContent(device: device)
            } else {
                entering(sync)
            }
        }
        .animation(reduceMotion ? nil : .standard, value: sync.pair)
    }

    private func entering(_ sync: SyncModel) -> some View {
        let failed: PairProblem? = { if case .failed(let problem) = sync.pair { return problem } else { return nil } }()
        return SyncSheetFrame(title: L.devicesAdd, cancel: { sync.close() }) {
            SyncNote(text: L.pairEnterLine)
            TVCodeBoxes(code: $code, focused: $focused, touch: touch, locked: sync.pair == .working, submit: submit)
                .frame(maxWidth: .infinity, alignment: .leading)
                #if os(iOS)
                .listRowBackground(Color.clear)
                #endif
            if let failed {
                SyncProblemLine(text: message(failed, device: sync.pairFailedDevice))
            } else {
                SyncNote(text: L.pairSameWifi)
            }
            #if os(iOS)
            if TVCodeScanner.isSupported {
                SyncChoice(title: L.pairScan, glyph: "qrcode.viewfinder") { scanning = true }
            }
            SyncChoice(title: L.pairShowInstead, glyph: "number") { sync.sheet = .pairShow }
            #endif
        } bottom: {
            #if os(macOS)
            SyncButton(title: L.pairShowInstead) { sync.sheet = .pairShow }
            #endif
            SyncButton(title: sync.pair == .working ? L.pairPairing : L.pairButton, primary: true, wide: touch, action: submit)
                .disabled(code.count < 6 || sync.pair == .working)
        }
        .onAppear { focused = true }
        .onChange(of: code) { if sync.pair != .working, sync.pair != .idle { sync.resetPair() } }
        #if os(iOS)
        .sheet(isPresented: $scanning) {
            TVCodeScanner(scheme: "kaeri://pair/") { link in
                scanning = false
                read(link)
            }
            .ignoresSafeArea()
        }
        #endif
    }

    private func submit() {
        let sync = store.sync
        guard code.count == 6 else { return }
        let name = sync.deviceName.isEmpty ? DeviceNaming.system : sync.deviceName
        if address.isEmpty {
            sync.pairWithCode(code, deviceName: name)
        } else {
            sync.pairWith(address: address, code: code, deviceName: name)
        }
    }

    /// `kaeri://pair/042917@192.168.1.20:4510`: a scanned code carries the address, so it needs no search.
    private func read(_ link: String) {
        let body = link.replacingOccurrences(of: "kaeri://pair/", with: "")
        let parts = body.split(separator: "@", maxSplits: 1).map(String.init)
        guard parts.count == 2 else { return }
        code = parts[0]
        address = parts[1]
        submit()
    }

    private func message(_ problem: PairProblem, device: String?) -> String {
        switch problem {
        case .code: L.pairErrCode(device: device ?? L.devicesOtherDevice)
        case .notFound: L.pairErrNotFound
        case .expired: L.pairErrExpired
        }
    }

    private var touch: Bool {
        #if os(iOS)
        true
        #else
        false
        #endif
    }
}

/// Paired: the sheet's content crossfades in place to this.
struct PairedContent: View {
    @Environment(AppStore.self) private var store
    let device: String
    var first = false
    var done: () -> Void = {}
    @State private var copySignIns: Bool
    @State private var joinStore = true
    init(device: String, first: Bool = false, done: @escaping () -> Void = {}) {
        self.device = device
        self.first = first
        self.done = done
        _copySignIns = State(initialValue: first)
    }

    var body: some View {
        let sync = store.sync
        let trackers = store.accounts.trackers.filter(\.connected).map { AccountsModel.name($0.tracker) }
        // The stores this device can join through the paired one, whose invite is held for it.
        let offer = first ? sync.status.storesToJoin.first(where: { $0.inviteFrom == device }) : nil
        SyncSheetFrame(title: L.pairOk(device: device)) {
            SyncNote(text: first ? L.firstOkLine : L.pairOkLine)
            if !trackers.isEmpty || first {
                SyncSection(footer: nil) {
                    #if os(macOS)
                    SettingsRow(title: L.pairCopySignins(device: device), note: L.pairCopySigninsLine(trackers: trackers.isEmpty ? AccountsModel.name("anilist") : trackers.joined(separator: ", "))) {
                        Toggle("", isOn: $copySignIns).toggleStyle(.switch).tint(.amber).labelsHidden()
                    }
                    #else
                    Toggle(isOn: $copySignIns) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(L.pairCopySignins(device: device)).font(.ui(17)).foregroundStyle(.silk)
                            Text(L.pairCopySigninsLine(trackers: trackers.isEmpty ? AccountsModel.name("anilist") : trackers.joined(separator: ", "))).font(.ui(13)).foregroundStyle(.haze)
                        }
                    }
                    .tint(.amber)
                    .listRowBackground(Palette.raised)
                    #endif
                }
            }
            if let offer {
                SyncSection(footer: nil) {
                    #if os(macOS)
                    SettingsRow(title: L.firstJoinStore(store: offer.name), note: L.firstJoinStoreLine(device: device, kind: Device.name)) {
                        Toggle("", isOn: $joinStore).toggleStyle(.switch).tint(.amber).labelsHidden()
                    }
                    #else
                    Toggle(isOn: $joinStore) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(L.firstJoinStore(store: offer.name)).font(.ui(17)).foregroundStyle(.silk)
                            Text(L.firstJoinStoreLine(device: device, kind: Device.name)).font(.ui(13)).foregroundStyle(.haze)
                        }
                    }
                    .tint(.amber)
                    .listRowBackground(Palette.raised)
                    #endif
                }
            }
        } bottom: {
            SyncButton(title: first ? L.onboardingContinue : L.actionDone, primary: true, wide: Device.name != "Mac") {
                if copySignIns { sync.copySignIns(from: device, [.trackers]) }
                sync.resetPair()
                sync.close()
                if joinStore, let offer { sync.joinStores(from: device, [offer.address]) }
                done()
            }
        }
    }
}
