// Sources for books: folders, servers and plugins, each with what it holds and its health in
// words. Add a source offers three kinds and nothing more: a folder, a server, a plugin store
// joined with an invite. Kaeri never names or suggests a source.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI
import UniformTypeIdentifiers

/// A page a book source's row opens: the demo, a server or a plugin.
enum BookSourcePage: Hashable {
    case demo
    case server(String)
    case plugin(String)
}

/// The list of book sources, under Your sources: the demo first, then folders, servers and
/// plugins. A row opens the source's own page.
struct BookSourcesList: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Binding var page: BookSourcePage?
    @State private var matchesShown = false

    var body: some View {
        let view = store.bookSources.view
        VStack(alignment: .leading, spacing: 22) {
            // Empty once the engine said so, not while the list is on its way.
            if store.bookSources.view != nil, store.bookSources.isEmpty {
                PlainEmpty(title: L.sourcesEmptyTitle, line: L.bookAddLine) {
                    Button(L.sourcesAddTitle) { navigator.addBookSource = .choose }
                        .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                }
            }
            if store.demo.state.reading {
                SettingsGroup(title: nil) {
                    BookSourceRow(title: L.sourcesDemoName, note: L.sourcesDemoBooksLine, open: { page = .demo }) {
                        SourceTile(systemImage: "book")
                    } trailing: {
                        RowChevron()
                    }
                }
            }
            if let view, !view.folders.isEmpty {
                let unsure = view.groups.filter { $0.work == nil && !$0.kept }.count
                SettingsGroup(title: L.bookSourcesFolders) {
                    ForEach(view.folders.filter { !$0.removed }, id: \.id) { folder in
                        BookSourceRow(title: folder.name, note: Self.folderLine(folder),
                                      noteColor: folder.scan == .missing ? .silk : .haze, open: nil) {
                            SourceTile(systemImage: "folder")
                        } trailing: {
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
                        BookServerSourceRow(server: server, note: Self.serverLine(server, justAdded: store.cameIn.justAdded.contains(server.id)),
                                            open: { page = .server(server.id) }) {
                            if server.signedOut {
                                Label(L.serversRowSignedOut, systemImage: "exclamationmark.triangle.fill")
                                    .labelStyle(BookHealthLabel())
                            }
                        }
                    }
                }
            }
            if let view, !view.plugins.isEmpty {
                SettingsGroup(title: L.bookSourcesPlugins) {
                    ForEach(view.plugins, id: \.id) { plugin in
                        BookSourceRow(title: plugin.name, note: Self.pluginLine(plugin), open: { page = .plugin(plugin.id) }) {
                            SourceTile(systemImage: "powerplug")
                        } trailing: {
                            RowChevron()
                        }
                    }
                }
            }
            if !store.bookSources.isEmpty {
                SettingsFootnote(L.bookSourcesPluginsFooter)
                    .padding(.top, -12)
            }
        }
        .task { store.bookSources.load() }
        .sheet(isPresented: $matchesShown) { CheckMatchesSheet() }
        .folderCameInSheet(store)
        .refreshable { store.bookSources.scan() }
        // Shots: a prelaunch state's sheet, staged with nothing asked of a real server.
        .onLaunchSheet("demo-about", navigator) { page = .demo } // i18n-skip: a shot's sheet name
        .onLaunchSheet("came-in", navigator) { // i18n-skip: a shot's sheet name
            guard let scene = PrelaunchScene.named() else { return }
            switch scene {
            case .folder, .folderNothing, .folderAlready, .folderUnreadable:
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

    /// "Kavita · 212 series in 2 libraries". What became of the progress sent to it is the line
    /// under this one (ServerProgressModel), so this no longer says progress goes back.
    static func serverLine(_ server: BookServerRow, justAdded: Bool = false) -> String {
        if server.empty { return L.sourceRowServerEmpty(kind: BookServerWords.rowKind(server.kind)) }
        if justAdded {
            return L.sourceRowServer(kind: BookServerWords.rowKind(server.kind), series: Int(server.series), libraries: Int(server.libraries))
        }
        if server.libraries > 0 && !server.off {
            return L.sourceRowServerSeries(kind: BookServerWords.rowKind(server.kind), series: Int(server.series), libraries: Int(server.libraries))
        }
        var parts = [BookServerWords.rowKind(server.kind)]
        if server.name != server.address, server.name != server.progressHost { parts.append(server.address) }
        parts.append(L.bookSourcesSeries(n: Int(server.series)))
        if server.off { parts.append(L.bookSourcesOff) }
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

    /// The kind at the head of a server's row: "OPDS" there, "OPDS catalogue" in a sentence.
    static func rowKind(_ kind: BookServerKind) -> String {
        kind == .opds ? "OPDS" : Self.kind(kind) // i18n-skip: a format's name
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

    /// Whether the toolbar offers Add a source: once a source is listed. With none, the empty
    /// state's own button is the one Add a source, and while the list is on its way there is
    /// nothing to add to yet.
    @MainActor static func shows(_ store: AppStore) -> Bool {
        store.medium.answer != .watch && !store.bookSources.isEmpty
    }

    var body: some View {
        Button {
            navigator.addBookSource = .choose
        } label: {
            Label(L.sourcesAddTitle, systemImage: "plus").labelStyle(.titleAndIcon)
        }
    }
}

/// The book sources on a page of their own. A row's page is pushed on the stack on iPhone and
/// iPad; on the Mac it takes the list's place, with Back in the toolbar (MacShell).
struct BookSourcesScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    #if os(iOS)
    @State private var pushed: BookSourcePage?
    #endif

    var body: some View {
        #if os(iOS)
        scroll { BookSourcesList(page: $pushed) }
            .navigationDestination(item: $pushed) { page in
                scroll { BookSourcePageView(page: page) }
                    .navigationTitle(page.title(store))
                    .modifier(PushedTitle(title: page.title(store)))
            }
            .onChange(of: pushed.map { $0.exists(store) } ?? true) { _, there in
                if !there { pushed = nil }
            }
        #else
        @Bindable var navigator = navigator
        ZStack(alignment: .topLeading) {
            if let page = navigator.bookSourcePage {
                scroll { BookSourcePageView(page: page) }
                    .id(page)
                    .transition(.opacity)
            } else {
                scroll { BookSourcesList(page: $navigator.bookSourcePage) }
                    .transition(.opacity)
            }
        }
        .animation(.standard, value: navigator.bookSourcePage)
        // A source removed while its page shows, here or on another device: back to the list.
        .onChange(of: navigator.bookSourcePage.map { $0.exists(store) } ?? true) { _, there in
            if !there { navigator.bookSourcePage = nil }
        }
        #endif
    }

    private func scroll<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        ScrollView {
            content()
                .frame(maxWidth: 640, alignment: .leading)
                .padding(.horizontal, Device.isMac ? 24 : 16)
                .padding(.vertical, Device.isMac ? 20 : 12)
                .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .background(.ground)
    }
}

#if os(iOS)
/// A pushed source page's title: inline on iPhone, the boards' title on iPad.
private struct PushedTitle: ViewModifier {
    let title: String

    func body(content: Content) -> some View {
        if Device.isPad {
            content.boardTitle(title)
        } else {
            content.navigationBarTitleDisplayMode(.inline)
        }
    }
}
#endif

extension BookSourcePage {
    /// The page's title: About the demo, or the server's or the plugin's name.
    @MainActor
    func title(_ store: AppStore) -> String {
        switch self {
        case .demo: L.sourcesDemoAbout
        case .server(let id): store.bookSources.view?.servers.first { $0.id == id }?.name ?? ""
        case .plugin(let id): store.bookSources.view?.plugins.first { $0.id == id }?.name ?? ""
        }
    }

    /// Whether what the page shows is still there.
    @MainActor
    func exists(_ store: AppStore) -> Bool {
        switch self {
        case .demo: store.demo.state.reading
        case .server(let id): store.bookSources.view?.servers.contains { $0.id == id } ?? false
        case .plugin(let id): store.bookSources.view?.plugins.contains { $0.id == id } ?? false
        }
    }
}

/// A book source's own page.
struct BookSourcePageView: View {
    let page: BookSourcePage

    var body: some View {
        switch page {
        case .demo: BookDemoPage()
        case .server(let id): BookServerPage(id: id)
        case .plugin(let id): BookPluginPage(id: id)
        }
    }
}

/// About the demo: what it is, its credits with each work's cover, where it reaches, and Remove
/// demo.
struct BookDemoPage: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            Text(L.sourcesDemoBooksWhat)
                .font(.plat(mac: 13, touch: 17))
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
            SettingsGroup(title: L.demoCredits) {
                ForEach(store.demo.reading.works, id: \.id) { work in
                    BookSourceRow(title: work.series, note: demoCreditLine(work), open: nil) {
                        BookCover(url: work.cover, width: Device.isMac ? 28 : 36, radius: 4)
                    } extra: {
                        if let cover = work.coverCredit {
                            Text(L.demoCreditCover(title: cover.title, artist: cover.artist, year: String(cover.year)))
                                .font(.plat(mac: 12, touch: 15))
                                .foregroundStyle(.haze)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        // The file's page on Commons, which the licence asks the credit to link,
                        // and the licence itself: both links, in the one link colour.
                        let page = work.filePageUrl.flatMap(URL.init(string:))
                        let licence = work.licenceUrl.flatMap(URL.init(string:))
                        if page != nil || licence != nil {
                            ViewThatFits(in: .horizontal) {
                                HStack(spacing: 14) {
                                    if let page { Link(L.sourcesDemoLinkCommons, destination: page) }
                                    if let licence { Link(L.sourcesDemoLinkLicence, destination: licence) }
                                }
                                VStack(alignment: .leading, spacing: 6) {
                                    if let page { Link(L.sourcesDemoLinkCommons, destination: page) }
                                    if let licence { Link(L.sourcesDemoLinkLicence, destination: licence) }
                                }
                            }
                            .font(.plat(mac: 12, touch: 15))
                            .foregroundStyle(.water)
                            .padding(.top, 2)
                        }
                    } trailing: {
                        EmptyView()
                    }
                }
            }
            SettingsGroup(title: nil) {
                line(L.sourcesDemoHosts)
                line(L.sourcesDemoBooksPrivate)
            }
            SettingsGroup(title: nil) {
                BookSourceRow(title: L.sourcesDemoRemove, open: { store.removeDemo() }) {
                    SourceTile(systemImage: "trash")
                } trailing: {
                    EmptyView()
                }
            }
        }
    }

    private func line(_ text: String) -> some View {
        Text(text)
            .font(.plat(mac: 13, touch: 17))
            .foregroundStyle(.silk)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(EdgeInsets(top: Device.isMac ? 10 : 14, leading: 14, bottom: Device.isMac ? 10 : 14, trailing: 14))
    }
}

/// A book server's page: its row with what became of the progress sent to it, Send progress and
/// Use, and Remove.
struct BookServerPage: View {
    @Environment(AppStore.self) private var store
    let id: String

    var body: some View {
        if let server = store.bookSources.view?.servers.first(where: { $0.id == id }) {
            VStack(alignment: .leading, spacing: 22) {
                SettingsGroup(title: nil) {
                    BookServerSourceRow(server: server, note: BookSourcesList.serverLine(server), open: nil) {
                        EmptyView()
                    }
                }
                SettingsGroup(title: nil) {
                    SettingsRow(title: L.bookSourcesReportProgress) {
                        BookSourceSwitch(title: L.bookSourcesReportProgress, isOn: Binding(
                            get: { server.reportProgress },
                            set: { store.bookSources.setReportProgress(server.id, on: $0) }
                        ))
                    }
                    SettingsRow(title: L.bookSourcesUse) {
                        BookSourceSwitch(title: L.bookSourcesUse, isOn: Binding(
                            get: { !server.off },
                            set: { store.bookSources.setOff(server.id, off: !$0) }
                        ))
                    }
                }
                SettingsGroup(title: nil) {
                    BookSourceRow(title: L.actionRemove, open: { store.bookSources.removeServer(server.id) }) {
                        SourceTile(systemImage: "trash")
                    } trailing: {
                        EmptyView()
                    }
                }
            }
        }
    }
}

/// A book plugin's page: its row and Use.
struct BookPluginPage: View {
    @Environment(AppStore.self) private var store
    let id: String

    var body: some View {
        if let plugin = store.bookSources.view?.plugins.first(where: { $0.id == id }) {
            VStack(alignment: .leading, spacing: 22) {
                SettingsGroup(title: nil) {
                    BookSourceRow(title: plugin.name, note: BookSourcesList.pluginLine(plugin), open: nil) {
                        SourceTile(systemImage: "powerplug")
                    } trailing: {
                        EmptyView()
                    }
                }
                SettingsGroup(title: nil) {
                    SettingsRow(title: L.bookSourcesUse) {
                        BookSourceSwitch(title: L.bookSourcesUse, isOn: Binding(
                            get: { !plugin.off },
                            set: { store.bookSources.setOff(plugin.id, off: !$0) }
                        ))
                    }
                }
            }
        }
    }
}

/// A switch at a row's end, its label kept for VoiceOver.
private struct BookSourceSwitch: View {
    let title: String
    @Binding var isOn: Bool

    var body: some View {
        Toggle(title, isOn: $isOn)
            .toggleStyle(.switch)
            .labelsHidden()
            .tint(.silk)
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

/// Settings' Sources row: what it says beside its name, for what Kaeri is for. Shows say whether
/// torrents are on; reading counts its sources; both count every source.
enum SourcesRowWords {
    @MainActor static func value(_ store: AppStore) -> String {
        switch store.medium.answer {
        case .watch: store.settings.isOn(.torrents) ? L.settingsTorrentsValueOn : L.settingsTorrentsValueOff
        case .read: L.settingsSourceCount(count: store.bookSources.count)
        case .both: L.settingsSourceCount(count: store.providers.rows.count + store.bookSources.count)
        }
    }
}

struct SourcesScopePicker: View {
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
    @State private var reading = false

    var body: some View {
        Group {
            switch store.medium.answer {
            case .watch: PhoneProvidersPage()
            case .read: PhoneBookSources()
            case .both:
                VStack(spacing: 0) {
                    SourcesScopePicker(reading: $reading).padding(.horizontal, 16).padding(.vertical, 8)
                    if reading { PhoneBookSources() } else { PhoneProvidersPage() }
                }
                .background(.ground)
            }
        }
        // Whichever list shows: the phone's providers page names itself too. Reading alone has
        // the board's large Your sources.
        .navigationTitle(store.medium.answer == .read ? L.sourcesYour : L.sourcesTitle)
        .navigationBarTitleDisplayMode(store.medium.answer == .read ? .large : .inline)
    }
}

/// The book sources on iPhone, with Add a source in the bar once one is listed, opening over the
/// Settings sheet.
struct PhoneBookSources: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        BookSourcesScreen()
            .toolbar {
                if BookSourcesToolbar.shows(store) {
                    ToolbarItem(placement: .primaryAction) {
                        Button(L.sourcesAddTitle, systemImage: "plus") { navigator.addBookSource = .choose }
                    }
                }
            }
            .sheet(item: Binding(get: { navigator.addBookSource }, set: { navigator.addBookSource = $0 })) { step in
                AddBookSourceSheet(start: step)
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
                    #if FIXTURES
                    // The UI tests' folder, as the system's picker would hand it back.
                    if let folder = BookFolders.fixture("KAERI_FIXTURE_FOLDER") {
                        BookFolders.add(folder, to: store)
                        dismiss()
                        return
                    }
                    #endif
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

    #if FIXTURES
    /// A folder the UI tests name in the environment, in place of the system's picker.
    static func fixture(_ name: String) -> URL? {
        guard let path = ProcessInfo.processInfo.environment[name], !path.isEmpty else { return nil }
        return URL(filePath: path, directoryHint: .isDirectory)
    }
    #endif
}

/// A pasted Kavita OPDS link: `https://host/api/opds/{key}`. The key signs in, so no password is
/// asked, and only its last four characters are ever shown.
struct KavitaLink {
    let host: String
    let last4: String
    /// The link up to its key, "https://host/api/opds/", which the address field shows.
    let base: String
    /// The key and anything after it, kept off screen.
    let key: String

    init?(_ address: String) {
        let text = address.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let url = URL(string: text), let host = url.host,
              url.scheme == "http" || url.scheme == "https" else { return nil }
        let parts = url.path.split(separator: "/").map(String.init)
        guard let at = parts.firstIndex(of: "opds"), at >= 1, parts[at - 1] == "api",
              parts.indices.contains(at + 1), parts[at + 1].count >= 4 else { return nil }
        self.host = host
        self.last4 = String(parts[at + 1].suffix(4))
        guard let end = text.range(of: "/api/opds/")?.upperBound else { return nil } // i18n-skip: a path
        base = String(text[..<end])
        key = String(text[end...])
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
    /// A pasted Kavita link's key: the field keeps only the link up to it, and the chip its last four.
    @State private var key: String?
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
            : scene == .unreachable ? "comics.home.example" : "books.home.example" // i18n-skip: made-up hosts
        hideKey()
        if link == nil { user = "reader"; password = "example-password" } // i18n-skip: made up
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
                if unreachable {
                    Button(L.actionTryAgain) { look() }
                        .buttonStyle(SecondaryCapsuleStyle(height: 28, raisedSurface: true, fontSize: 13))
                }
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

    /// A server that didn't answer: the sheet keeps the address alone, Add server waits, and Try
    /// again is the other action, beside it on the Mac and under the note on iPhone and iPad.
    private var unreachable: Bool {
        if case .empty(.unreachable)? = outcome { true } else { false }
    }

    /// Try again after a failed look, else Add server.
    @ViewBuilder
    private var action: some View {
        if failed, !unreachable {
            Button(L.actionTryAgain) { look() }
        } else {
            Button(L.bookAddServerButton) { add() }
        }
    }

    private var actionOff: Bool {
        if unreachable { return true }
        if failed { return false }
        guard let found else { return true }
        return !found.libraries.isEmpty && found.libraries.allSatisfy { unticked.contains($0.id) }
    }

    private var lifecycle: ServerStepLifecycle {
        ServerStepLifecycle(
            address: full, user: user, password: password, staged: staged,
            onAppear: {
                store.cameIn.reset()
                if let shot = Self.shot { Self.shot = nil; stage(shot) } else if let link = Self.prefill { Self.prefill = nil; address = link; hideKey() } else { focus = .address }
            },
            look: look,
            added: { dismiss() })
    }

    private var ready: Bool {
        if link != nil { return true }
        return !trimmed.isEmpty && !user.isEmpty && !password.isEmpty
    }

    private var trimmed: String { address.trimmingCharacters(in: .whitespacesAndNewlines) }

    /// The address as typed, with a pasted Kavita link's key put back after it.
    private var full: String { key.map { trimmed + $0 } ?? trimmed }

    private var link: KavitaLink? { KavitaLink(full) }

    /// A Kavita link was pasted: the key leaves the field, so only its last four are ever shown.
    /// Typing never moves it, and editing the link once its key is held lets the key go.
    private func hideKey(pasted: Bool = true) {
        if key != nil {
            if !trimmed.hasSuffix("/api/opds/") { key = nil } // i18n-skip: a path
            return
        }
        guard pasted, let pasted = KavitaLink(trimmed), !pasted.key.isEmpty else { return }
        key = pasted.key
        address = pasted.base
    }

    private func look() {
        guard ready, !staged else { return }
        if link != nil {
            store.cameIn.probe(address: full, user: nil, password: nil)
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
        let link = self.link
        return VStack(alignment: .leading, spacing: 0) {
            label(L.bookServerAddress)
            addressField(link)
                .padding(.top, 6)
            if address.isEmpty {
                note(L.bookServerAddressHint).padding(.top, 8)
            } else if link != nil {
                note(L.bookServerKavitaKey).padding(.top, 8)
                note(L.bookServerKeyKept(store: "the Keychain")).padding(.top, 4) // i18n-skip: the system's name for it
            } else if !unreachable {
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
                    VStack(spacing: 18) {
                        CameInNote(kind: .failed, title: L.bookServerErrUnreachable(host: host), line: L.bookServerErrUnreachableLine)
                            .frame(maxWidth: .infinity, alignment: .leading)
                        #if os(iOS)
                        Button(L.actionTryAgain) { look() }
                            .font(.ui(17, weight: .semibold))
                            .foregroundStyle(.silk)
                            .frame(minHeight: 44)
                            .buttonStyle(.plain)
                        #endif
                    }
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

    private var trimmedHost: String { link?.host ?? trimmed }

    /// One row per library, ticked by default.
    private func libraries(_ found: ServerFound) -> some View {
        CameInCard {
            ForEach(Array(found.libraries.enumerated()), id: \.element.id) { index, library in
                if index > 0 { Rectangle().fill(Palette.faintRule).frame(height: 1).padding(.leading, 12) }
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(library.name).font(.plat(mac: 13, touch: 17)).foregroundStyle(.silk)
                        if let series = library.series {
                            Text(L.bookServerLibraryLine(series: Int(series)))
                                .font(.plat(mac: 12, touch: 15)).foregroundStyle(.haze)
                        }
                    }
                    Spacer(minLength: 8)
                    Toggle(library.name, isOn: Binding(
                        get: { !unticked.contains(library.id) },
                        set: { on in if on { unticked.remove(library.id) } else { unticked.insert(library.id) } }
                    ))
                    .labelsHidden()
                    #if os(macOS)
                    .toggleStyle(.checkbox)
                    #endif
                    .tint(.amber)
                }
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
            .tint(.amber)
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
                .onChange(of: address) { old, new in hideKey(pasted: new.count - old.count > 1) }
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
