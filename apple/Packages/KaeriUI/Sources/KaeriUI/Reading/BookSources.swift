// Sources for books: folders, servers and plugins, each with what it holds and its health in
// words. Add a source offers three kinds and nothing more: a folder, a server, a plugin store
// joined with an invite. Kaeri never names or suggests a source.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI
import UniformTypeIdentifiers

/// The list of book sources, under Your sources.
struct BookSourcesList: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var matchesShown = false
    @State private var aboutDemo = false

    var body: some View {
        let view = store.bookSources.view
        VStack(alignment: .leading, spacing: 22) {
            // Empty once the engine said so, not while the list is on its way.
            if store.bookSources.view != nil, store.bookSources.isEmpty {
                PlainEmpty(title: L.sourcesAddTitle, line: L.bookAddLine) {
                    Button(L.sourcesAddTitle) { navigator.addBookSource = .choose }
                        .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                }
            }
            if let view, !view.folders.isEmpty {
                let unsure = view.groups.filter { $0.work == nil && !$0.kept }.count
                SettingsGroup(title: L.bookSourcesFolders) {
                    ForEach(view.folders.filter { !$0.removed }, id: \.id) { folder in
                        SettingsRow(title: folder.name, note: Self.folderLine(folder),
                                    noteColor: folder.scan == .missing ? .silk : .haze) {
                            HStack(spacing: 8) {
                                if unsure > 0, folder.id == view.folders.first?.id {
                                    Button(L.bookSourcesCheckMatches(n: unsure)) { matchesShown = true }
                                        .buttonStyle(SecondaryCapsuleStyle(height: 24, fontSize: 12))
                                        .fixedSize()
                                }
                                Menu {
                                    Button(L.bookSourcesScan) { store.bookSources.scan() }
                                    Button(L.actionRemove, role: .destructive) { store.bookSources.removeFolder(folder.id) }
                                } label: {
                                    Image(systemName: "ellipsis").foregroundStyle(.haze)
                                }
                                .menuStyle(.button)
                                .buttonStyle(.plain)
                                .menuIndicator(.hidden)
                                .fixedSize()
                                .accessibilityLabel(L.actionMore)
                            }
                        }
                    }
                }
            }
            if let view, !view.servers.isEmpty {
                SettingsGroup(title: L.bookSourcesServers) {
                    ForEach(view.servers, id: \.id) { server in
                        SettingsRow(title: server.name, note: Self.serverLine(server, justAdded: store.cameIn.justAdded.contains(server.id))) {
                            HStack(spacing: 8) {
                                if server.signedOut {
                                    Label(L.serversRowSignedOut, systemImage: "exclamationmark.triangle.fill")
                                        .labelStyle(BookHealthLabel())
                                }
                                Menu {
                                    Toggle(L.bookSourcesReportProgress, isOn: Binding(
                                        get: { server.reportProgress },
                                        set: { store.bookSources.setReportProgress(server.id, on: $0) }
                                    ))
                                    Toggle(L.bookSourcesUse, isOn: Binding(
                                        get: { !server.off },
                                        set: { store.bookSources.setOff(server.id, off: !$0) }
                                    ))
                                    Divider()
                                    Button(L.actionRemove, role: .destructive) { store.bookSources.removeServer(server.id) }
                                } label: {
                                    Image(systemName: "ellipsis").foregroundStyle(.haze)
                                }
                                .menuStyle(.button)
                                .buttonStyle(.plain)
                                .menuIndicator(.hidden)
                                .fixedSize()
                                .accessibilityLabel(L.actionMore)
                            }
                        }
                    }
                }
            }
            if let view, !view.plugins.isEmpty {
                SettingsGroup(title: L.bookSourcesPlugins) {
                    ForEach(view.plugins, id: \.id) { plugin in
                        SettingsRow(title: plugin.name, note: Self.pluginLine(plugin)) {
                            Toggle(plugin.name, isOn: Binding(
                                get: { !plugin.off },
                                set: { store.bookSources.setOff(plugin.id, off: !$0) }
                            ))
                            .toggleStyle(.switch)
                            .labelsHidden()
                            .tint(.silk)
                        }
                    }
                }
            }
            if store.demo.state.reading {
                SettingsGroup(title: nil) {
                    SettingsRow(title: L.sourcesDemoName, note: L.sourcesDemoBooksLine) {
                        Button(aboutDemo ? L.actionDone : L.sourcesDemoAbout) { aboutDemo.toggle() }
                            .buttonStyle(SecondaryCapsuleStyle(height: 24, raisedSurface: true, fontSize: 12))
                            .fixedSize()
                    }
                    if aboutDemo {
                        DemoBooksAbout()
                            .padding(.horizontal, 14).padding(.vertical, 12)
                            .transition(.opacity)
                    }
                }
                .animation(.standard, value: aboutDemo)
            }
            if !store.bookSources.isEmpty {
                HStack(spacing: 4) {
                    Text(L.bookSourcesPluginsFooter)
                    Button(L.navPluginStores) { navigator.open(.pluginStores) }
                        .buttonStyle(.plain)
                        .underline()
                        .foregroundStyle(.silk)
                        .pointerHover()
                }
                .font(.plat(mac: 12, touch: 13))
                .foregroundStyle(.haze)
                .padding(.top, -14)
                .padding(.leading, 14)
            }
        }
        .task { store.bookSources.load() }
        .sheet(isPresented: $matchesShown) { CheckMatchesSheet() }
        .folderCameInSheet(store)
        .refreshable { store.bookSources.scan() }
        // Shots: a prelaunch state's sheet, staged with nothing asked of a real server.
        .onLaunchSheet("came-in", navigator) { // i18n-skip: a shot's sheet name
            guard let scene = PrelaunchScene.named() else { return }
            switch scene {
            case .folder, .folderNothing:
                store.cameIn.stage(scene)
            case .inviteJoined, .pluginsInstalled, .storeOther, .storeEmpty, .storeUnreachable:
                store.cameIn.stage(scene)
                navigator.invite = ""
            case .afterwards:
                store.cameIn.stage(scene)
            default:
                AddBookServerStep.shot = scene
                navigator.addBookSource = .server
            }
        }
    }

    /// "~/Books/Manga · 64 series", or what a scan or a missing drive says.
    static func folderLine(_ folder: BookFolderRow) -> String {
        if folder.skipped > 0, case .done = folder.scan, let path = folder.path {
            return L.sourceRowFolder(path: path.replacingOccurrences(of: NSHomeDirectory(), with: "~"), // i18n-skip
                                     series: Int(folder.series), skipped: Int(folder.skipped))
        }
        var parts: [String] = []
        if let path = folder.path { parts.append(path.replacingOccurrences(of: NSHomeDirectory(), with: "~")) } // i18n-skip
        switch folder.scan {
        case .scanning(let found): parts.append(L.bookSourcesScanning(n: Int(found)))
        case .missing: parts.append(L.bookSourcesMissing)
        case .done: parts.append(L.bookSourcesSeries(n: Int(folder.series)))
        }
        return parts.joined(separator: " · ") // i18n-skip
    }

    /// "Kavita · 212 series · progress goes back to the server".
    static func serverLine(_ server: BookServerRow, justAdded: Bool = false) -> String {
        if server.empty { return L.sourceRowServerEmpty(kind: BookServerWords.kind(server.kind)) }
        if justAdded {
            return L.sourceRowServer(kind: BookServerWords.kind(server.kind), series: Int(server.series), libraries: Int(server.libraries))
        }
        var parts = [BookServerWords.kind(server.kind)]
        if server.name != server.address { parts.append(server.address) }
        parts.append(L.bookSourcesSeries(n: Int(server.series)))
        if server.off { parts.append(L.bookSourcesOff) }
        if server.reportProgress { parts.append(L.bookSourcesProgressBack) }
        return parts.joined(separator: " · ") // i18n-skip
    }

    /// "From Kim's store · reading · version 1.4".
    static func pluginLine(_ plugin: BookPluginRow) -> String {
        guard let store = plugin.store else { return L.bookSourcesVersion(version: plugin.version) }
        return L.sourceRowPlugin(store: store, medium: L.mediumAnswerRead.lowercased(), version: plugin.version)
    }
}

enum BookServerWords {
    static func kind(_ kind: BookServerKind) -> String {
        switch kind {
        case .komga: "Komga" // i18n-skip
        case .kavita: "Kavita" // i18n-skip
        case .opds: L.bookSourcesOpds
        }
    }

    static func failure(_ failure: BookServerFailure) -> String {
        switch failure {
        case .server(let failure): failure.words
        case .notABookServer: L.bookSourcesNotABookServer
        }
    }
}

/// A health word with the warning glyph, the one vermilion on the row.
private struct BookHealthLabel: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 6) {
            configuration.icon.foregroundStyle(.vermilion).font(.ui(11))
            configuration.title.foregroundStyle(.silk)
        }
        .font(.plat(mac: 12, touch: 13))
    }
}

/// Your sources' toolbar with reading on: Add a source, secondary.
struct BookSourcesToolbar: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        Button {
            navigator.addBookSource = .choose
        } label: {
            Label(L.sourcesAddTitle, systemImage: "plus").labelStyle(.titleAndIcon)
        }
    }
}

/// The book sources on a page of their own.
struct BookSourcesScreen: View {
    var body: some View {
        ScrollView {
            BookSourcesList()
                .frame(maxWidth: 640, alignment: .leading)
                .padding(.horizontal, Device.isMac ? 24 : 16)
                .padding(.vertical, Device.isMac ? 20 : 12)
                .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .background(.ground)
    }
}

/// Your sources for what Kaeri is for: the shows' sources, the books', or both behind Shows or
/// Reading.
struct SourcesScreen: View {
    @Environment(AppStore.self) private var store
    @State private var reading = false

    var body: some View {
        switch store.medium.answer {
        case .watch: ProvidersScreen()
        case .read: BookSourcesScreen()
        case .both:
            VStack(alignment: .leading, spacing: 0) {
                SourcesScopePicker(reading: $reading)
                    .padding(.horizontal, Device.isMac ? 24 : 16)
                    .padding(.top, 8)
                    .padding(.bottom, 4)
                ZStack {
                    if reading {
                        BookSourcesScreen().transition(.opacity)
                    } else {
                        ProvidersScreen().transition(.opacity)
                    }
                }
                .animation(.standard, value: reading)
            }
            .background(.ground)
        }
    }
}

private struct SourcesScopePicker: View {
    @Binding var reading: Bool

    var body: some View {
        Picker(L.libraryScopeLabel, selection: $reading) {
            Text(L.libraryScopeShows).tag(false)
            Text(L.libraryScopeReading).tag(true)
        }
        .pickerStyle(.segmented)
        .labelsHidden()
        .frame(maxWidth: Device.isMac ? nil : .infinity)
        .fixedSize(horizontal: Device.isMac, vertical: false)
    }
}

#if os(iOS)
/// Your sources in the iPhone's Settings sheet. Add a source opens over the sheet from here.
struct PhoneYourSourcesPage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var reading = false

    var body: some View {
        Group {
            switch store.medium.answer {
            case .watch: PhoneProvidersPage()
            case .read: books
            case .both:
                VStack(spacing: 0) {
                    SourcesScopePicker(reading: $reading).padding(.horizontal, 16).padding(.vertical, 8)
                    if reading { books } else { PhoneProvidersPage() }
                }
                .background(.ground)
            }
        }
        // Whichever list shows: the phone's providers page names itself too.
        .navigationTitle(L.sourcesTitle)
        .navigationBarTitleDisplayMode(.inline)
        .sheet(item: Binding(get: { navigator.addBookSource }, set: { navigator.addBookSource = $0 })) { step in
            AddBookSourceSheet(start: step)
        }
    }

    private var books: some View {
        BookSourcesScreen()
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(L.sourcesAddTitle, systemImage: "plus") { navigator.addBookSource = .choose }
                }
            }
    }
}
#endif

// MARK: - Add a source

/// Add a source: a folder, a server, or a plugin store. Nothing else.
struct AddBookSourceSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    @State private var picking = false
    @State private var step: AddBookSourceStep

    init(start: AddBookSourceStep = .choose) {
        _step = State(initialValue: start)
    }

    var body: some View {
        Group {
            if step == .server {
                // Its own header on touch, which reaches the sheet's edges.
                AddBookServerStep().padding(Device.isMac ? 24 : 0)
            } else {
                choose.padding(Device.isMac ? 24 : 20)
            }
        }
        .sheetWidth(480)
        .background(.raised)
        .closesOnEscape { dismiss() }
        #if os(iOS)
        .fileImporter(isPresented: $picking, allowedContentTypes: [.folder]) { result in
            guard case .success(let url) = result else { return }
            BookFolders.add(url, to: store)
            dismiss()
        }
        #endif
    }

    private var choose: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(L.sourcesAddTitle)
                .font(.plat(mac: 17, touch: 22, weight: .bold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            Text(L.bookAddLine)
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 4)
            VStack(spacing: 0) {
                kind(L.bookAddFolder, line: L.bookAddFolderLine(device: Device.name)) {
                    #if os(macOS)
                    FolderPicker.choose { path in
                        store.bookSources.addFolder(FolderRef(path: path, treeUri: nil, bookmark: nil, name: nil))
                        dismiss()
                    }
                    #else
                    picking = true
                    #endif
                }
                Rectangle().fill(Palette.faintRule).frame(height: 1)
                kind(L.bookAddServer, line: L.bookAddServerLine) {
                    store.bookSources.resetServerSheet()
                    withAnimation(.standard) { step = .server }
                }
                Rectangle().fill(Palette.faintRule).frame(height: 1)
                kind(L.bookAddStore, line: L.bookAddStoreLine) {
                    dismiss()
                    navigator.invite = ""
                }
            }
            .background(Palette.ground.opacity(0.4), in: .rect(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Palette.faintRule, lineWidth: 1))
            .padding(.top, 16)
            HStack {
                Spacer()
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, raisedSurface: true, fontSize: Device.isMac ? 13 : 17))
                    .keyboardShortcut(.cancelAction)
            }
            .padding(.top, 20)
        }
    }

    private func kind(_ title: String, line: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.plat(mac: 13, touch: 17, weight: .semibold)).foregroundStyle(.silk)
                    Text(line).font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 8)
                Image(systemName: "chevron.right").font(.ui(12, weight: .semibold)).foregroundStyle(.haze)
                    .accessibilityHidden(true)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, Device.isMac ? 10 : 14)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
    }
}

enum BookFolders {
    /// A folder from the system's picker, kept by a security-scoped bookmark the engine hands
    /// back when it reads.
    @MainActor
    static func add(_ url: URL, to store: AppStore) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        let bookmark = try? url.bookmarkData(options: [], includingResourceValuesForKeys: nil, relativeTo: nil)
        store.bookSources.addFolder(FolderRef(path: url.path(percentEncoded: false), treeUri: nil,
                                              bookmark: bookmark, name: url.lastPathComponent))
    }
}

/// A pasted Kavita OPDS link: `https://host/api/opds/{key}`. The key signs in, so no password is
/// asked, and only its last four characters are ever shown.
struct KavitaLink {
    let host: String
    let last4: String

    init?(_ address: String) {
        let text = address.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let url = URL(string: text), let host = url.host,
              url.scheme == "http" || url.scheme == "https" else { return nil }
        let parts = url.path.split(separator: "/").map(String.init)
        guard let at = parts.firstIndex(of: "opds"), at >= 1, parts[at - 1] == "api",
              parts.indices.contains(at + 1), parts[at + 1].count >= 4 else { return nil }
        self.host = host
        self.last4 = String(parts[at + 1].suffix(4))
    }
}

/// Add a server: the address, a Kavita link or a user and password, looked at as they are typed;
/// what it holds is said before anything is added (its libraries, to pick), and why nothing
/// came in when nothing did. Add server is the sheet's one amber action.
struct AddBookServerStep: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var address = ""
    @State private var user = ""
    @State private var password = ""
    @State private var reportProgress = true
    @State private var unticked: Set<String> = []
    @State private var staged = false
    @FocusState private var focus: Field?

    private enum Field { case address, user, password }

    /// For shots: the sheet opens in this state, with nothing asked of a real server.
    @MainActor static var shot: PrelaunchScene?
    /// A pasted Kavita link the sheet opens with, looked at as soon as it is up.
    @MainActor static var prefill: String?

    private func stage(_ scene: PrelaunchScene) {
        staged = true
        address = scene == .kavitaLink || scene == .found || scene == .wrongKey || scene == .looking
            ? "https://books.home.example/api/opds/9c1e7a2b4d5f3f9a" // i18n-skip: a made-up link
            : "books.home.example" // i18n-skip: a made-up host
        if KavitaLink(address) == nil { user = "reader"; password = "example-password" } // i18n-skip: made up
        store.cameIn.stage(scene)
    }

    var body: some View {
        #if os(iOS)
        NavigationStack {
            ScrollView { form.padding(.horizontal, 20).padding(.vertical, 12) }
                .scrollBounceBehavior(.basedOnSize)
                .background(.raised)
                .navigationTitle(L.bookAddServerTitle)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button(L.actionCancel) { dismiss() } }
                    ToolbarItem(placement: .confirmationAction) { action.disabled(actionOff) }
                }
        }
        .presentationBackground(.raised)
        .modifier(lifecycle)
        #else
        VStack(alignment: .leading, spacing: 0) {
            Text(L.bookAddServerTitle)
                .font(.mac(17, weight: .bold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            form.padding(.top, 16)
            HStack(spacing: 10) {
                Spacer()
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(height: 28, raisedSurface: true, fontSize: 13))
                    .keyboardShortcut(.cancelAction)
                action
                    .buttonStyle(PrimaryCapsuleStyle(height: 28, fontSize: 13))
                    .keyboardShortcut(.defaultAction)
                    .disabled(actionOff)
            }
            .padding(.top, 22)
        }
        .modifier(lifecycle)
        #endif
    }

    private var outcome: CameIn? { store.cameIn.outcome }

    /// What a look found, when it found something to add, or an empty catalogue that can be added.
    private var found: ServerFound? {
        switch outcome {
        case .found(let found)?: found
        case .empty(.emptyCatalogue(let found))?: found
        default: nil
        }
    }

    private var failed: Bool {
        guard case .empty(let why)? = outcome else { return false }
        if case .emptyCatalogue = why { return false }
        return true
    }

    /// Try again after a failed look, else Add server.
    @ViewBuilder
    private var action: some View {
        if failed {
            Button(L.actionTryAgain) { look() }
        } else {
            Button(L.bookAddServerButton) { add() }
        }
    }

    private var actionOff: Bool {
        if failed { return false }
        guard let found else { return true }
        return !found.libraries.isEmpty && found.libraries.allSatisfy { unticked.contains($0.id) }
    }

    private var lifecycle: ServerStepLifecycle {
        ServerStepLifecycle(
            address: address, user: user, password: password, staged: staged,
            onAppear: {
                store.cameIn.reset()
                if let shot = Self.shot { Self.shot = nil; stage(shot) } else if let link = Self.prefill { Self.prefill = nil; address = link } else { focus = .address }
            },
            look: look,
            added: { dismiss() })
    }

    private var ready: Bool {
        if KavitaLink(address) != nil { return true }
        return !trimmed.isEmpty && !user.isEmpty && !password.isEmpty
    }

    private var trimmed: String { address.trimmingCharacters(in: .whitespacesAndNewlines) }

    private func look() {
        guard ready, !staged else { return }
        if KavitaLink(address) != nil {
            store.cameIn.probe(address: trimmed, user: nil, password: nil)
        } else {
            store.cameIn.probe(address: trimmed, user: user, password: password)
        }
    }

    private func add() {
        guard let found else { return }
        let all = found.libraries.map(\.id)
        let picked = all.filter { !unticked.contains($0) }
        store.cameIn.addServer(found, libraries: picked.count == all.count ? [] : picked, reportProgress: reportProgress)
    }

    private var form: some View {
        let link = KavitaLink(address)
        return VStack(alignment: .leading, spacing: 0) {
            label(L.bookServerAddress)
            addressField(link)
                .padding(.top, 6)
            if address.isEmpty {
                note(L.bookServerAddressHint).padding(.top, 8)
            } else if link != nil {
                note(L.bookServerKavitaKey).padding(.top, 8)
                note(L.bookServerKeyKept(store: "the Keychain")).padding(.top, 4) // i18n-skip: the system's name for it
            } else {
                label(L.bookServerUsername).padding(.top, 16)
                field(L.bookServerUsername, text: $user, focus: .user).padding(.top, 6)
                label(L.bookServerPassword).padding(.top, 12)
                field(L.bookServerPassword, text: $password, focus: .password, secure: true).padding(.top, 6)
                    .onSubmit { look() }
            }
            result.padding(.top, 14)
        }
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : .standard, value: store.cameIn.slow)
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : .standard, value: outcome)
    }

    /// The answer, crossfading in place: the waiting line after 600 ms, what was found, or why not.
    @ViewBuilder
    private var result: some View {
        let host = found?.host ?? trimmedHost
        if store.cameIn.looking, store.cameIn.slow {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L.bookServerLooking).foregroundStyle(.haze)
            }
            .font(.plat(mac: 12, touch: 15))
            .transition(.opacity)
        } else if case .found(let found)? = outcome {
            VStack(alignment: .leading, spacing: 12) {
                CameInNote(kind: .found,
                           title: L.bookServerFoundTitle(series: Int(found.series ?? 0), libraries: found.libraries.count),
                           line: L.bookServerFoundLine(kind: BookServerWords.kind(found.kind), host: found.host))
                if !found.libraries.isEmpty { libraries(found) }
                progressSwitch(host: found.host)
            }
            .transition(.opacity)
        } else if case .empty(let why)? = outcome {
            Group {
                switch why {
                case .unreachable(let host):
                    CameInNote(kind: .failed, title: L.bookServerErrUnreachable(host: host), line: L.bookServerErrUnreachableLine)
                case .wrongPassword(let host, let kind):
                    CameInNote(kind: .failed, title: L.bookServerErrPassword(host: host),
                               line: L.bookServerErrPasswordLine(kind: BookServerWords.kind(kind)))
                case .wrongKey(let host):
                    CameInNote(kind: .failed, title: L.bookServerErrKey(host: host), line: L.bookServerErrKeyLine)
                case .notABookServer:
                    CameInNote(kind: .failed, title: L.bookSourcesNotABookServer, line: L.bookServerNotBooksLine)
                case .emptyCatalogue(let found):
                    VStack(alignment: .leading, spacing: 6) {
                        CameInNote(kind: .info, title: L.bookServerEmpty(host: found.host), line: L.bookServerEmptyLine)
                        note(L.bookServerEmptyRecheck).padding(.leading, 20)
                    }
                default:
                    EmptyView()
                }
            }
            .transition(.opacity)
        }
    }

    private var trimmedHost: String { KavitaLink(address)?.host ?? trimmed }

    /// One row per library, ticked by default.
    private func libraries(_ found: ServerFound) -> some View {
        CameInCard {
            ForEach(Array(found.libraries.enumerated()), id: \.element.id) { index, library in
                if index > 0 { Rectangle().fill(Palette.faintRule).frame(height: 1).padding(.leading, 12) }
                Toggle(isOn: Binding(
                    get: { !unticked.contains(library.id) },
                    set: { on in if on { unticked.remove(library.id) } else { unticked.insert(library.id) } }
                )) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(library.name).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                        if let series = library.series {
                            Text(L.bookServerLibraryLine(series: Int(series)))
                                .font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                        }
                    }
                }
                #if os(macOS)
                .toggleStyle(.checkbox)
                #endif
                .tint(.amber)
                .padding(.horizontal, 12).padding(.vertical, 8)
            }
        }
    }

    private func progressSwitch(host: String) -> some View {
        CameInCard {
            Toggle(isOn: $reportProgress) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(L.bookServerSyncProgress).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                    Text(L.bookServerSyncProgressLine(host: host))
                        .font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .toggleStyle(.switch)
            .tint(.silk)
            .padding(.horizontal, 12).padding(.vertical, 10)
        }
    }

    private func label(_ text: String) -> some View {
        Text(text).font(.plat(mac: 12, touch: 13)).foregroundStyle(.haze)
    }

    private func note(_ text: String) -> some View {
        Text(text).font(.plat(mac: 12, touch: 13)).foregroundStyle(.haze).fixedSize(horizontal: false, vertical: true)
    }

    /// The address, with a "Key ••••3f9a" chip inside it for a Kavita link.
    private func addressField(_ link: KavitaLink?) -> some View {
        HStack(spacing: 8) {
            field(L.bookServerAddress, text: $address, focus: .address, bare: true)
            if let link {
                HStack(spacing: 4) {
                    Image(systemName: "key").font(.ui(11))
                    Text(L.bookServerKey(last: link.last4)).font(.system(size: Device.isMac ? 12 : 14, design: .monospaced))
                }
                .foregroundStyle(.silk)
                .padding(.horizontal, 8).padding(.vertical, 4)
                .background(Palette.raised, in: .rect(cornerRadius: 6))
                .fixedSize()
                .transition(.opacity)
            }
        }
        .padding(.horizontal, 12)
        .frame(height: Device.isMac ? 32 : 48)
        .background(.ground, in: .rect(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
    }

    private func field(_ title: String, text: Binding<String>, focus which: Field, secure: Bool = false, bare: Bool = false) -> some View {
        Group {
            if secure {
                SecureField(title, text: text)
            } else {
                TextField(title, text: text, prompt: Text(which == .address ? "books.home.example" : "").foregroundStyle(.haze)) // i18n-skip
                    #if os(iOS)
                    .textInputAutocapitalization(.never)
                    .keyboardType(which == .address ? .URL : .default)
                    #endif
            }
        }
        .textFieldStyle(.plain)
        .font(.plat(mac: 13, touch: 17))
        .autocorrectionDisabled()
        .focused($focus, equals: which)
        .modifier(FieldBox(bare: bare))
        .labelsHidden()
    }
}

private struct FieldBox: ViewModifier {
    let bare: Bool
    func body(content: Content) -> some View {
        if bare {
            content
        } else {
            content
                .padding(.horizontal, 12)
                .frame(height: Device.isMac ? 32 : 48)
                .background(.ground, in: .rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
        }
    }
}

/// The server sheet's timing: a look 500 ms after the typing pauses, and gone once added.
private struct ServerStepLifecycle: ViewModifier {
    @Environment(AppStore.self) private var store
    let address: String
    let user: String
    let password: String
    let staged: Bool
    let onAppear: () -> Void
    let look: () -> Void
    let added: () -> Void

    func body(content: Content) -> some View {
        content
            .onAppear(perform: onAppear)
            .task(id: address + "\u{0}" + user + "\u{0}" + password) {
                guard !staged else { return }
                try? await Task.sleep(for: .milliseconds(500))
                guard !Task.isCancelled else { return }
                look()
            }
            .onChange(of: store.cameIn.outcome) { _, outcome in
                if case .serverAdded? = outcome { added() }
            }
    }
}

// MARK: - Check matches

/// Folder names beside their AniList match: sure ones can be changed, unsure ones confirmed,
/// unmatched ones found by hand or kept as Your files.
private struct CheckMatchesSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var finding: BookGroupRow?
    @State private var query = ""

    var body: some View {
        let groups = (store.bookSources.view?.groups ?? []).filter { !$0.kept }
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(L.bookSourcesCheckMatchesTitle)
                    .font(.plat(mac: 17, touch: 22, weight: .bold))
                    .foregroundStyle(.silk)
                Spacer()
                Button(L.actionDone) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 40, raisedSurface: true, fontSize: Device.isMac ? 13 : 17))
                    .keyboardShortcut(.defaultAction)
            }
            if let group = finding {
                find(group)
            } else {
                ScrollView {
                    VStack(spacing: 0) {
                        ForEach(groups.filter { $0.work == nil }, id: \.group) { group in
                            row(group)
                            Rectangle().fill(Palette.faintRule).frame(height: 1)
                        }
                    }
                }
                .padding(.top, 12)
                Text(L.bookSourcesUnmatchedNote)
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 12)
            }
        }
        .padding(Device.isMac ? 24 : 20)
        .sheetWidth(520)
        .frame(minHeight: Device.isMac ? 420 : nil)
        .background(.raised)
        .closesOnEscape { dismiss() }
    }

    private func row(_ group: BookGroupRow) -> some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(group.name).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk).lineLimit(1)
                Text(group.matchedTitle ?? L.bookSourcesNoMatch).font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze).lineLimit(1)
            }
            Spacer(minLength: 8)
            Button(L.bookSourcesKeep) { store.bookSources.keep(group: group.group) }
                .buttonStyle(.plain)
                .font(.plat(mac: 12, touch: 15))
                .foregroundStyle(.haze)
            Button(L.bookSourcesFind) {
                query = group.matchedTitle ?? group.name
                finding = group
                store.bookSources.findMatch(group: group.group, query: query)
            }
            .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 24 : 36, raisedSurface: true, fontSize: Device.isMac ? 12 : 15))
            .fixedSize()
        }
        .padding(.vertical, 10)
    }

    private func find(_ group: BookGroupRow) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            TextField(L.searchPlaceholder, text: $query)
                .textFieldStyle(.plain)
                .font(.plat(mac: 13, touch: 17))
                .padding(.horizontal, 12)
                .frame(height: Device.isMac ? 32 : 44)
                .background(.ground, in: .rect(cornerRadius: 8))
                .onSubmit { store.bookSources.findMatch(group: group.group, query: query) }
            ScrollView {
                VStack(spacing: 0) {
                    ForEach(store.bookSources.matches[group.group] ?? [], id: \.anilist) { choice in
                        Button {
                            store.bookSources.chooseMatch(group: group.group, anilist: choice.anilist)
                            finding = nil
                        } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(choice.title).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk).lineLimit(1)
                                Text(L.libraryMatchPercent(percent: Int(choice.confidence)))
                                    .font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 8)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            Button(L.actionBack) { finding = nil }
                .buttonStyle(.plain)
                .foregroundStyle(.silk)
        }
        .padding(.top, 12)
    }
}
