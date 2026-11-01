// Add a media server (the lawful-sources board): a sheet of steps. Find the server by its address
// (and by the network on the Mac), sign in with a password or Quick Connect, choose its libraries.
// 520 pt wide on the Mac, a large sheet on iPhone and iPad. Every error stands under the field
// that caused it, in words from the catalogue, and the field keeps its text.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

extension ServerFailure {
    /// The sentence under the field or button that caused it.
    var words: String {
        switch self {
        case .unreachable(let address): L.serversErrorUnreachable(address: address)
        case .notJellyfin: L.serversErrorNotJellyfin
        case .tooOld(let version): L.serversErrorTooOld(version: version)
        case .credentials: L.serversErrorCredentials
        case .quickConnectOff: L.serversErrorQcOff
        case .quickConnectExpired: L.serversQcExpired
        case .plainHttp: L.serversErrorPlainHttp
        case .certificate(let host): L.serversErrorCertificate(host: host)
        case .signedOut: L.serversRowSignedOut
        case .badAddress: L.serversErrorBadAddress
        case .other: L.serversErrorOther
        }
    }
}

struct AddServerSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var address = ""
    @State private var user = ""
    @State private var password = ""
    @State private var report = true
    /// The step has waited 600 ms: the button shows its spinner.
    @State private var busySlow = false
    /// The broadcast has run 600 ms without an answer: the looking row shows.
    @State private var lookingShown = false
    @FocusState private var focus: Field?

    private enum Field { case address, user, password }

    #if os(macOS)
    private static let title = Font.mac(15, weight: .semibold)
    private static let body = Font.mac(13)
    private static let small = Font.mac(12)
    private static let fieldHeight: CGFloat = 32
    private static let discovers = true
    #else
    private static let title = Font.ui(20, weight: .semibold)
    private static let body = Font.ui(17)
    private static let small = Font.ui(13)
    private static let fieldHeight: CGFloat = 48
    /// Sending a broadcast on iPhone and iPad needs an entitlement a free account cannot have.
    private static let discovers = false
    #endif

    private var servers: ServersModel { store.servers }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Group {
                switch servers.step {
                case .address: addressStep
                case .signIn: signInStep
                case .quickConnect: quickConnectStep
                case .libraries: librariesStep
                case .certificate: certificateStep
                }
            }
            .id(servers.step)
            .transition(reduceMotion ? .opacity : .opacity.combined(with: .offset(x: 24)))
        }
        .animation(.standard, value: servers.step)
        .padding(30)
        .sheetWidth(520)
        .background(.raised)
        .task {
            store.beginAddServer(discover: Self.discovers)
            #if FIXTURES
            if navigator.certificateLaunch { store.servers.fixtureCertificate(testingCertificateCheck()) }
            #endif
            focus = .address
        }
        .onChange(of: servers.added) { _, added in
            if added != nil { dismiss() }
        }
        .onChange(of: servers.discovering) { _, on in
            if !on { lookingShown = false }
        }
        .task(id: servers.discovering ? servers.found.count : -1) {
            lookingShown = false
            guard servers.discovering, servers.found.isEmpty else { return }
            try? await Task.sleep(for: .milliseconds(600))
            lookingShown = servers.discovering && servers.found.isEmpty
        }
        .task(id: servers.busy) {
            busySlow = false
            guard servers.busy else { return }
            try? await Task.sleep(for: .milliseconds(600))
            busySlow = servers.busy
        }
        .onDisappear {
            if servers.step == .quickConnect { store.cancelQuickConnect() }
        }
        .closesOnEscape { dismiss() }
    }

    // MARK: Find

    private var addressStep: some View {
        VStack(alignment: .leading, spacing: 0) {
            heading(L.serversAddTitle)
            if Self.discovers { onNetwork.padding(.top, 16) }
            Text(L.serversAddress).font(Self.small).foregroundStyle(.haze).padding(.top, 18)
            field(L.serversAddress, text: $address, prompt: L.serversAddressHint, focus: .address)
                .padding(.top, 6)
                .onSubmit { check() }
            if !Self.discovers {
                Text(L.serversAddressNote).font(Self.small).foregroundStyle(.haze).padding(.top, 8)
            }
            if let failure = servers.failure, servers.failedStep == .address {
                errorLine(failure.words)
            }
            buttons {
                Button(L.serversContinue) { check() }
                    .buttonStyle(PrimaryCapsuleStyle(height: Self.buttonHeight, fontSize: Self.buttonFont))
                    .keyboardShortcut(.defaultAction)
                    .disabled(address.trimmingCharacters(in: .whitespaces).isEmpty || servers.busy)
            }
        }
    }

    @ViewBuilder
    private var onNetwork: some View {
        VStack(alignment: .leading, spacing: 4) {
            if !servers.found.isEmpty || lookingShown || (servers.discoveryDone && servers.found.isEmpty) {
                Text(L.serversOnNetwork).groupHeaderStyle().macLineHeight(14)
            }
            ForEach(servers.found, id: \.id) { found in
                Button {
                    address = found.address
                    check()
                } label: {
                    HStack {
                        Text(found.name).font(Self.body).foregroundStyle(.silk)
                        Text(found.address).font(Self.small).foregroundStyle(.haze)
                        Spacer(minLength: 0)
                    }
                    .padding(.horizontal, 12)
                    .frame(height: 36)
                    .background(Palette.selection.opacity(address == found.address ? 1 : 0), in: .rect(cornerRadius: 8))
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .transition(.opacity)
            }
            if lookingShown {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L.serversLooking).font(Self.small).foregroundStyle(.haze)
                }
                .transition(.opacity)
            } else if servers.discoveryDone, servers.found.isEmpty {
                Text(L.serversNoneFound).font(Self.small).foregroundStyle(.haze).transition(.opacity)
            }
        }
        .animation(reduceMotion ? .fadeIn : .standard, value: servers.found)
        .animation(.fadeIn, value: lookingShown)
        .animation(.fadeIn, value: servers.discoveryDone)
    }

    private func check() {
        guard !address.trimmingCharacters(in: .whitespaces).isEmpty, !servers.busy else { return }
        store.checkServer(address: address)
    }

    // MARK: Sign in

    private var signInStep: some View {
        let name = servers.checked?.name ?? address
        return VStack(alignment: .leading, spacing: 0) {
            heading(L.serversSignInTitle(server: name))
            Text(L.serversUser).font(Self.small).foregroundStyle(.haze).padding(.top, 18)
            field(L.serversUser, text: $user, prompt: nil, focus: .user).padding(.top, 6)
                .onSubmit { focus = .password }
            Text(L.serversPassword).font(Self.small).foregroundStyle(.haze).padding(.top, 14)
            field(L.serversPassword, text: $password, prompt: nil, focus: .password, secure: true).padding(.top, 6)
                .onSubmit { signIn() }
            if let failure = servers.failure, servers.failedStep == .signIn {
                errorLine(failure.words)
            }
            if servers.checked?.quickConnect == true, servers.failure != .quickConnectOff {
                Button(L.serversUseQuickConnect) { store.startQuickConnect() }
                    .buttonStyle(.plain)
                    .font(Self.small)
                    .foregroundStyle(.amber)
                    .padding(.top, 14)
            }
            Text(L.serversTokenNote(store: L.secureStoreDevice))
                .font(Self.small).foregroundStyle(.haze).padding(.top, 14)
                .fixedSize(horizontal: false, vertical: true)
            buttons {
                busy
                Button(L.serversSignIn) { signIn() }
                    .buttonStyle(PrimaryCapsuleStyle(height: Self.buttonHeight, fontSize: Self.buttonFont))
                    .keyboardShortcut(.defaultAction)
                    .disabled(user.isEmpty || servers.busy)
            }
        }
        .task { focus = .user }
    }

    private func signIn() {
        guard !user.isEmpty, !servers.busy else { return }
        store.signInToServer(user: user, password: password)
    }

    // MARK: Quick Connect

    private var quickConnectStep: some View {
        VStack(alignment: .leading, spacing: 0) {
            heading(L.serversQcTitle)
            Text(servers.code ?? "······")
                .font(.system(size: 40, weight: .semibold, design: .rounded))
                .monospacedDigit()
                .kerning(4)
                .foregroundStyle(servers.code == nil ? .haze : .silk)
                .padding(.top, 20)
                .accessibilityLabel(servers.code.map { $0.map(String.init).joined(separator: " ") } ?? "")
            Text(L.serversQcSteps).font(Self.body).foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true).padding(.top, 12)
            if let failure = servers.failure, failure == .quickConnectExpired || failure == .quickConnectOff {
                errorLine(failure.words)
                if failure == .quickConnectExpired {
                    Button(L.serversQcNewCode) { store.startQuickConnect() }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        .padding(.top, 10)
                }
            } else if servers.code != nil {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L.serversQcWaiting).font(Self.small).foregroundStyle(.haze)
                }
                .padding(.top, 16)
            }
            buttons {
                Button(L.serversUsePassword) {
                    store.cancelQuickConnect()
                    store.servers.toPassword()
                }
                .buttonStyle(SecondaryCapsuleStyle(height: Self.buttonHeight, raisedSurface: true, fontSize: Self.buttonFont))
            }
        }
    }

    // MARK: Libraries

    private var librariesStep: some View {
        VStack(alignment: .leading, spacing: 0) {
            heading(L.serversLibrariesTitle)
            Text(L.serversLibrariesBody).font(Self.body).foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true).padding(.top, 4)
            VStack(spacing: 0) {
                ForEach(servers.choices, id: \.id) { library in
                    Toggle(isOn: Binding(get: { library.ticked }, set: { _ in store.servers.toggle(library.id) })) {
                        VStack(alignment: .leading, spacing: 1) {
                            Text(library.name).font(Self.body).foregroundStyle(.silk)
                            Text(library.kind ?? "").font(Self.small).foregroundStyle(.haze)
                        }
                    }
                    #if os(macOS)
                    .toggleStyle(.checkbox)
                    #endif
                    .tint(.amber)
                    .padding(.vertical, 8)
                }
            }
            .padding(.top, 14)
            Toggle(isOn: $report) {
                VStack(alignment: .leading, spacing: 1) {
                    Text(L.serversReport).font(Self.body).foregroundStyle(.silk)
                    Text(L.serversReportLine).font(Self.small).foregroundStyle(.haze)
                }
            }
            .tint(.amber)
            .padding(.top, 14)
            buttons {
                Button(L.serversAdd) { store.finishAddingServer(report: report) }
                    .buttonStyle(PrimaryCapsuleStyle(height: Self.buttonHeight, fontSize: Self.buttonFont))
                    .keyboardShortcut(.defaultAction)
                    .disabled(!servers.choices.contains(where: \.ticked))
            }
        }
    }

    // MARK: Check this server

    /// Its own step between Find and Sign in. Both buttons are secondary and Return does nothing:
    /// trusting a certificate is never the default.
    private var certificateStep: some View {
        VStack(alignment: .leading, spacing: 0) {
            heading(L.serversCertTitle)
            if let check = servers.certificate {
                Text(L.serversCertBody(host: check.host))
                    .font(Self.body).foregroundStyle(.silk)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 12)
                // The fingerprint in groups of four, in monospace, to compare by eye.
                Text(check.groups.joined(separator: " "))
                    .font(.system(size: 13, design: .monospaced))
                    .foregroundStyle(.silk)
                    .textSelection(.enabled)
                    .padding(14)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(.ground, in: .rect(cornerRadius: 8))
                    .padding(.top, 14)
                    .accessibilityLabel(check.groups.joined(separator: ", "))
            }
            if let failure = servers.failure, servers.failedStep == .address {
                errorLine(failure.words)
            }
            HStack(spacing: 10) {
                Spacer()
                busy
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(height: Self.buttonHeight, raisedSurface: true, fontSize: Self.buttonFont))
                    .keyboardShortcut(.cancelAction)
                Button(L.serversCertAllow) { store.trustCertificate() }
                    .buttonStyle(SecondaryCapsuleStyle(height: Self.buttonHeight, raisedSurface: true, fontSize: Self.buttonFont))
                    .disabled(servers.busy)
            }
            .padding(.top, 22)
        }
    }

    // MARK: Parts

    private func heading(_ text: String) -> some View {
        Text(text).font(Self.title).foregroundStyle(.silk).accessibilityAddTraits(.isHeader)
    }

    private func errorLine(_ text: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Circle().fill(.vermilion).frame(width: 6, height: 6)
            Text(text).font(Self.small).foregroundStyle(.silk).fixedSize(horizontal: false, vertical: true)
        }
        .padding(.top, 10)
        .transition(.opacity)
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private var busy: some View {
        if busySlow { ProgressView().controlSize(.small) }
    }

    private func buttons(@ViewBuilder _ primary: () -> some View) -> some View {
        HStack(spacing: 10) {
            Spacer()
            if servers.step == .address { busy }
            Button(L.actionCancel) { dismiss() }
                .buttonStyle(SecondaryCapsuleStyle(height: Self.buttonHeight, raisedSurface: true, fontSize: Self.buttonFont))
                .keyboardShortcut(.cancelAction)
            primary()
        }
        .padding(.top, 22)
    }

    private func field(_ title: String, text: Binding<String>, prompt: String?, focus which: Field, secure: Bool = false) -> some View {
        Group {
            if secure {
                SecureField(title, text: text, prompt: prompt.map { Text($0).foregroundStyle(.haze) })
            } else {
                TextField(title, text: text, prompt: prompt.map { Text($0).foregroundStyle(.haze) })
                    #if os(iOS)
                    .textInputAutocapitalization(.never)
                    .keyboardType(.URL)
                    #endif
            }
        }
        .textFieldStyle(.plain)
        .font(Self.body)
        .autocorrectionDisabled()
        .focused($focus, equals: which)
        .padding(.horizontal, 12)
        .frame(height: Self.fieldHeight)
        .background(.ground, in: .rect(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
        .labelsHidden()
    }

    #if os(macOS)
    private static let buttonHeight: CGFloat = 28
    private static let buttonFont: CGFloat = 13
    #else
    private static let buttonHeight: CGFloat = 48
    private static let buttonFont: CGFloat = 17
    #endif
}
