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

    var body: some View {
        let view = store.bookSources.view
        VStack(alignment: .leading, spacing: 22) {
            if store.bookSources.isEmpty {
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
                        SettingsRow(title: server.name, note: Self.serverLine(server)) {
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
        #if FIXTURES
        // Shots: the add-server sheet with a server found, and with it added and its series counted.
        .onLaunchSheet("add-book-server", navigator) { // i18n-skip: a shot's sheet name
            AddBookServerStep.shot = .found
            navigator.addBookSource = .server
        }
        .onLaunchSheet("add-book-server-added", navigator) { // i18n-skip: a shot's sheet name
            AddBookServerStep.shot = .added
            navigator.addBookSource = .server
        }
        #endif
    }

    /// "~/Books/Manga · 64 series", or what a scan or a missing drive says.
    static func folderLine(_ folder: BookFolderRow) -> String {
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
    static func serverLine(_ server: BookServerRow) -> String {
        var parts = [BookServerWords.kind(server.kind)]
        if server.name != server.address { parts.append(server.address) }
        parts.append(L.bookSourcesSeries(n: Int(server.series)))
        if server.off { parts.append(L.bookSourcesOff) }
        if server.reportProgress { parts.append(L.bookSourcesProgressBack) }
        return parts.joined(separator: " · ") // i18n-skip
    }

    /// "From Kim's store · reading · version 1.4".
    static func pluginLine(_ plugin: BookPluginRow) -> String {
        var parts: [String] = []
        if let store = plugin.store { parts.append(L.bookSourcesFromStore(store: store)) }
        parts.append(L.bookSourcesVersion(version: plugin.version))
        return parts.joined(separator: " · ") // i18n-skip
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
                AddBookServerStep()
            } else {
                choose
            }
        }
        .padding(Device.isMac ? 24 : 20)
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

/// Add a server: the address is tried as it is typed, and what was found is said before the
/// sign-in. Add server is the sheet's one amber action.
private struct AddBookServerStep: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var address = ""
    @State private var user = ""
    @State private var password = ""
    @State private var slow = false
    @State private var staged = false
    @FocusState private var focus: Field?

    private enum Field { case address, user, password }

    #if FIXTURES
    enum Shot { case found, added }
    /// For shots: the sheet opens in this state, with nothing asked of a real server.
    @MainActor static var shot: Shot?

    private func stage(_ shot: Shot) {
        let sources = store.bookSources
        staged = true
        address = "books.home.example" // i18n-skip: a made-up host
        user = "reader" // i18n-skip: a made-up name
        password = "example-password" // i18n-skip: a made-up password
        sources.showcase(.serverChecked(address: address, kind: .kavita, name: "Books", version: "0.8")) // i18n-skip
        if shot == .added {
            sources.showcase(.serverAdded(id: "fixture-server", kind: .kavita, series: 212)) // i18n-skip
        }
    }
    #endif

    var body: some View {
        let sources = store.bookSources
        VStack(alignment: .leading, spacing: 0) {
            Text(L.bookAddServerTitle)
                .font(.plat(mac: 17, touch: 22, weight: .bold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            label(L.serversAddress).padding(.top, 16)
            field(L.serversAddress, text: $address, prompt: "books.home.example", focus: .address) // i18n-skip
                .padding(.top, 6)
            if let checked = sources.checked, checked.address == address.trimmingCharacters(in: .whitespaces) {
                HStack(spacing: 6) {
                    WaterDot()
                    // Once signed in, how many series it lists, when the server said for sure.
                    if let added = sources.added, added.kind == checked.kind, let series = added.series {
                        Text(L.sourcesFoundServerSeries(kind: BookServerWords.kind(checked.kind), n: Int(series)))
                    } else {
                        Text(L.sourcesFoundServer(kind: BookServerWords.kind(checked.kind)))
                    }
                }
                .font(.plat(mac: 12, touch: 15))
                .foregroundStyle(.silk)
                .padding(.top, 8)
                label(L.serversUser).padding(.top, 16)
                field(L.serversUser, text: $user, prompt: nil, focus: .user).padding(.top, 6)
                label(L.serversPassword).padding(.top, 12)
                field(L.serversPassword, text: $password, prompt: nil, focus: .password, secure: true).padding(.top, 6)
                    .onSubmit { add() }
                Text(L.bookServerNote)
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 10)
            } else if sources.checking, slow {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text(L.bookServerLooking).foregroundStyle(.haze)
                }
                .font(.plat(mac: 12, touch: 15))
                .padding(.top, 8)
            }
            if let failed = sources.failed {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.vermilion).font(.ui(11))
                        .accessibilityHidden(true)
                    Text(BookServerWords.failure(failed.failure))
                        .foregroundStyle(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .font(.plat(mac: 12, touch: 15))
                .padding(.top, 10)
                .accessibilityElement(children: .combine)
            }
            HStack(spacing: 10) {
                Spacer()
                if sources.added != nil {
                    Button(L.actionDone) { dismiss() }
                        .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                        .keyboardShortcut(.defaultAction)
                } else {
                    Button(L.actionCancel) { dismiss() }
                        .buttonStyle(SecondaryCapsuleStyle(height: buttonHeight, raisedSurface: true, fontSize: buttonFont))
                        .keyboardShortcut(.cancelAction)
                    if sources.checked == nil, sources.failed != nil {
                        Button(L.actionTryAgain) { sources.checkServer(trimmed) }
                            .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                            .keyboardShortcut(.defaultAction)
                    } else {
                        Button(L.bookAddServerButton) { add() }
                            .buttonStyle(PrimaryCapsuleStyle(height: buttonHeight, fontSize: buttonFont))
                            .keyboardShortcut(.defaultAction)
                            .disabled(sources.checked == nil || user.isEmpty || password.isEmpty || sources.checking)
                    }
                }
            }
            .padding(.top, 22)
        }
        .animation(.standard, value: sources.checked?.address)
        .onAppear {
            // A server added before is not this one.
            sources.resetServerSheet()
            focus = .address
            #if FIXTURES
            if let shot = Self.shot {
                Self.shot = nil
                focus = nil
                stage(shot)
            }
            #endif
        }
        .task(id: address) {
            // Tried as it is typed, once the typing pauses.
            guard !trimmed.isEmpty, !staged else { return }
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            sources.checkServer(trimmed)
        }
        .task(id: sources.checking) {
            slow = false
            guard sources.checking else { return }
            try? await Task.sleep(for: .milliseconds(600))
            slow = sources.checking
        }
        // Added: gone at once, unless there is a count of its series to say first, with Done.
        .onChange(of: sources.added?.id) { _, id in
            if id != nil, sources.added?.series == nil { dismiss() }
        }
    }

    private var trimmed: String { address.trimmingCharacters(in: .whitespaces) }

    private func add() {
        guard store.bookSources.checked != nil, !user.isEmpty, !password.isEmpty else { return }
        store.bookSources.signIn(address: trimmed, user: user, password: password)
    }

    private var buttonHeight: CGFloat { Device.isMac ? 28 : 48 }
    private var buttonFont: CGFloat { Device.isMac ? 13 : 17 }

    private func label(_ text: String) -> some View {
        Text(text).font(.plat(mac: 12, touch: 13)).foregroundStyle(.haze)
    }

    private func field(_ title: String, text: Binding<String>, prompt: String?, focus which: Field, secure: Bool = false) -> some View {
        Group {
            if secure {
                SecureField(title, text: text)
            } else {
                TextField(title, text: text, prompt: prompt.map { Text($0).foregroundStyle(.haze) })
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
        .padding(.horizontal, 12)
        .frame(height: Device.isMac ? 32 : 48)
        .background(.ground, in: .rect(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
        .labelsHidden()
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
