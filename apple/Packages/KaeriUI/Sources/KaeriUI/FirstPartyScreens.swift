// The first-party sources on the Mac and iPad (the lawful-sources board): Your media, Media servers
// and Licensed services as sections above the plugins in Sources, an Add menu, and the page each
// row opens in the detail column. The iPhone has the same rows in its Settings sheet
// (PhoneFirstParty).

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// What Sources has selected: the prefix says which kind of row.
enum SourceKey {
    static let licensed = "licensed"
    static func server(_ id: String) -> String { "server:" + id }
    static func folder(_ path: String) -> String { "folder:" + path }
    static func serverId(_ key: String?) -> String? { key?.hasPrefix("server:") == true ? String(key!.dropFirst(7)) : nil }
    static func folderPath(_ key: String?) -> String? { key?.hasPrefix("folder:") == true ? String(key!.dropFirst(7)) : nil }
}

/// A device that has no folder to offer: only the Mac reads folders in this release.
let devicePicksFolders: Bool = {
    #if os(macOS)
    true
    #else
    false
    #endif
}()

extension AppStore {
    /// Whether Sources has anything first-party to show: a folder or a server.
    var hasFirstParty: Bool {
        !servers.rows.isEmpty || !local.folders.isEmpty || !(licensed.view?.settings.services.isEmpty ?? true)
    }

    /// The row Sources selects before the person selects one: the first first-party row when there
    /// is one, else the first plugin (`nil`, which the screens read as the first row).
    var defaultSourceKey: String? {
        if devicePicksFolders, let folder = local.folders.first { return SourceKey.folder(folder.path) }
        if let server = servers.rows.first { return SourceKey.server(server.id) }
        return nil
    }

    /// A launch argument's `server` or `folder`, as the first such row.
    func resolvedSourceKey(_ key: String?) -> String? {
        switch key {
        case "server": servers.rows.first.map { SourceKey.server($0.id) } ?? key
        case "folder": local.folders.first.map { SourceKey.folder($0.path) } ?? key
        default: key
        }
    }

    /// The name of a region, in the viewer's language, in the viewer's language.
    static func regionName(_ code: String) -> String {
        Locale.current.localizedString(forRegionCode: code) ?? code
    }
}

// MARK: Add

/// The Add button of Sources: a folder (on the Mac), a media server, a plugin store, or the demo
/// while it can be added back.
struct AddSourceMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var height: CGFloat = 28
    var fontSize: CGFloat = 13

    var body: some View {
        Menu {
            if devicePicksFolders {
                Button(L.sourcesAddFolder) { FolderPicker.choose { store.addFolder($0) } }
            }
            Button(L.sourcesAddServer) { navigator.addServerShown = true }
            Button(L.sourcesAddStore) { navigator.invite = "" }
            if store.demo.state.offer {
                Button(L.sourcesDemoAdd) { store.addDemo() }
            }
        } label: {
            Text(L.sourcesAdd)
        }
        .menuStyle(.button)
        .menuIndicator(.hidden)
        .buttonStyle(SecondaryCapsuleStyle(height: height, fontSize: fontSize))
        .fixedSize()
    }
}

/// The system folder picker, for the Mac. The prompt and message are from the catalogue.
enum FolderPicker {
    @MainActor
    static func choose(_ picked: @escaping (String) -> Void) {
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.prompt = L.libraryPickerPrompt
        panel.message = L.libraryPickerMessage
        if panel.runModal() == .OK, let url = panel.url { picked(url.path(percentEncoded: false)) }
        #endif
    }
}

// MARK: Sections in the list

/// Your media, Media servers and Licensed services, above the plugins.
struct FirstPartySections: View {
    @Environment(AppStore.self) private var store
    @Binding var selection: String?
    let current: String?

    var body: some View {
        // Once a second only while a lookup is waiting on AniList, to count the wait down.
        TimelineView(.periodic(from: .now, by: 1)) { context in
            content(now: context.date)
        }
        .task {
            store.loadServers()
            store.loadLicensed()
            if devicePicksFolders { store.loadLocal() }
        }
    }

    private func content(now: Date) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if devicePicksFolders, !store.local.folders.isEmpty {
                header(L.sourcesSectionMedia)
                ForEach(store.local.folders, id: \.path) { folder in
                    FirstPartyLine(
                        title: URL(filePath: folder.path).lastPathComponent,
                        line: folderLine(folder, now: now),
                        warn: !folder.available,
                        selected: current == SourceKey.folder(folder.path),
                        coverUrl: cover(of: folder)?.coverUrl, coverColor: cover(of: folder)?.plateColor
                    )
                    .onTapGesture { selection = SourceKey.folder(folder.path) }
                    .transition(.opacity)
                }
            }
            if !store.servers.rows.isEmpty {
                header(L.sourcesSectionServers)
                ForEach(store.servers.rows, id: \.id) { row in
                    FirstPartyLine(
                        title: row.name, line: serverLine(row, now: now), warn: row.signedOut || row.notAnswering,
                        selected: current == SourceKey.server(row.id)
                    )
                    .onTapGesture { selection = SourceKey.server(row.id) }
                    .transition(.opacity)
                }
            }
            header(L.sourcesSectionLicensed)
            FirstPartyLine(
                title: L.licensedTitle, line: licensedLine, warn: false, selected: current == SourceKey.licensed
            )
            .onTapGesture { selection = SourceKey.licensed }
        }
        .animation(.standard, value: store.servers.rows.map(\.id))
        .animation(.standard, value: store.local.folders.map(\.path))
    }

    /// The most recent matched title in a folder: the last one the scan found.
    private func cover(of folder: FolderRow) -> Entry? {
        store.local.groups(in: folder.path, .matched).reversed()
            .lazy.compactMap { $0.anilist.flatMap { store.titles[$0] } }.first
    }

    private func header(_ text: String) -> some View {
        Text(text)
            .groupHeaderStyle()
            .accessibilityAddTraits(.isHeader)
            .macLineHeight(14)
            .padding(EdgeInsets(top: 10, leading: 18, bottom: 8, trailing: 0))
    }

    private func folderLine(_ folder: FolderRow, now: Date) -> String {
        if !folder.available { return L.libraryRowDriveOffline }
        switch folder.scan {
        case .scanning(let files)?:
            return L.libraryRowScanning(files: Int(files))
        case .matching(let progress)?:
            return Self.matchingLine(progress, left: store.local.waitLeft(folder.path, at: now))
        case nil:
            let summary = L.libraryRowSummary(
                episodes: L.libraryEpisodes(count: Int(folder.episodes)),
                series: L.libraryRowSeries(count: Int(folder.series)))
            return folder.needsLook > 0
                ? summary + " · " + L.libraryRowNeedsLook(count: Int(folder.needsLook)) : summary
        }
    }

    private func serverLine(_ row: ServerRow, now: Date) -> String {
        if row.signedOutHere { return L.serversRowSignedOutHere }
        if row.signedOut { return L.serversRowSignedOut }
        if row.notAnswering { return Self.notAnsweringLine(row.lastSeen, now: now) }
        if let progress = row.matching {
            return Self.matchingLine(progress, left: store.servers.waitLeft(row.id, at: now))
        }
        let summary = L.serversRowSummaryApple(series: Int(row.series))
        let unmatched = Int(row.series) - Int(row.matched)
        return unmatched > 0 ? summary + " · " + L.serversRowNeedsLook(count: unmatched) : summary
    }

    /// "Not answering · last seen 14:20", or "Not answering" before the server ever answered.
    static func notAnsweringLine(_ lastSeen: Int64?, now: Date) -> String {
        guard let lastSeen else { return L.sourcesStatusNotAnswering }
        return L.serversRowNotAnswering(time: seenTime(Date(timeIntervalSince1970: TimeInterval(lastSeen)), now: now))
    }

    /// When a server last answered, in the locale's own style: the short time when that was
    /// today ("14:20"), and the short date with the time before that ("1 Nov, 03:24").
    nonisolated static func seenTime(_ seen: Date, now: Date, calendar: Calendar = .current, locale: Locale = .current) -> String {
        var style = Date.FormatStyle(locale: locale, calendar: calendar, timeZone: calendar.timeZone)
        style = calendar.isDate(seen, inSameDayAs: now)
            ? style.hour().minute()
            : style.day().month(.abbreviated).hour().minute()
        return seen.formatted(style)
    }

    /// "Matching · 31 of 58 series", then "Waiting for AniList · 12 s" while its limit holds a lookup.
    static func matchingLine(_ progress: MatchProgress, left: Int?) -> String {
        let line = L.libraryRowMatching(done: Int(progress.done), total: Int(progress.total))
        guard progress.waitSeconds != nil, let left else { return line }
        return line + " · " + L.libraryRowWaiting(seconds: left)
    }

    private var licensedLine: String {
        guard let line = store.licensed.rowLine(regionName: AppStore.regionName) else { return L.licensedRowNone }
        return line
    }
}

/// The small cover that leads a source row: the art of a matched title, or a plate with the
/// first letter of the name until there is art.
struct RowCover: View {
    @Environment(\.displayScale) private var scale
    let title: String
    let url: String?
    let color: Color?

    private let size = CGSize(width: 28, height: 40)

    var body: some View {
        ZStack {
            if url != nil {
                ArtworkView(url: url, color: color, maxPixelSize: Int(size.height * 1.5 * scale))
            } else {
                CoverPlate(color: color)
                Text(String(title.first.map { Character($0.uppercased()) } ?? " "))
                    .font(.mac(13, weight: .semibold))
                    .foregroundStyle(.haze)
            }
        }
        .frame(width: size.width, height: size.height)
        .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: size.width)))
        .overlay(
            RoundedRectangle(cornerRadius: ArtShape.coverRadius(width: size.width))
                .strokeBorder(Palette.glassEdge, lineWidth: 1)
        )
        .accessibilityHidden(true)
    }
}

/// One first-party row: its name and its line, selected like a plugin's.
struct FirstPartyLine: View {
    let title: String
    let line: String
    let warn: Bool
    let selected: Bool
    /// The cover of a matched title, when there is one to stand beside the row.
    var coverUrl: String? = nil
    var coverColor: Color? = nil
    /// Whether the row leads with art or a letter plate at all.
    var leading = true

    var body: some View {
        HStack(spacing: 12) {
            if leading { RowCover(title: title, url: coverUrl, color: coverColor) }
            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.mac(13, weight: .medium))
                    .macLineHeight(17)
                    .foregroundStyle(selected ? .amber : .silk)
                    .lineLimit(1)
                Text(line)
                    .font(.mac(12))
                    .macLineHeight(15)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                    .monospacedDigit()
                    .contentTransition(.opacity)
                    .id(line)
                    .transition(.opacity)
                    .animation(.fadeIn, value: line)
            }
            Spacer(minLength: 0)
            if warn { Circle().fill(.vermilion).frame(width: 6, height: 6) }
            Image(systemName: "chevron.right")
                .font(.mac(11, weight: .semibold))
                .foregroundStyle(.haze)
        }
        .padding(.horizontal, 12)
        .frame(height: leading ? 52 : 44)
        .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
        .contentShape(.rect(cornerRadius: 8))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

// MARK: Pages

/// A page's heading and the line beneath it.
private struct PageHeading: View {
    let title: String
    var line: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(.mac(22, weight: .bold)).macLineHeight(28).foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            if let line {
                Text(line).font(.mac(13)).foregroundStyle(.haze).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// The Licensed services page: Region, Your services, and the switch for the others.
struct LicensedServicesPage: View {
    @Environment(AppStore.self) private var store
    @State private var showAll = false
    private static let firstShown = 8

    var body: some View {
        let view = store.licensed.view
        let available = view?.available ?? []
        let shown = showAll ? available : Array(available.prefix(Self.firstShown))
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                PageHeading(title: L.licensedTitle, line: L.licensedBody)
                SettingsGroup(footnote: L.licensedRegionLine) {
                    SettingsRow(title: L.licensedRegion) {
                        Menu {
                            Button(L.licensedRegionSystem) { store.setLicensedRegion(nil) }
                            Divider()
                            ForEach(view?.regions ?? [], id: \.self) { code in
                                Button(AppStore.regionName(code)) { store.setLicensedRegion(code) }
                            }
                        } label: {
                            MenuValue(
                                text: view?.settings.region.map(AppStore.regionName) ?? L.licensedRegionSystem,
                                phone: false)
                        }
                        .menuStyle(.button)
                        .buttonStyle(.plain)
                    }
                }
                SettingsGroup(title: L.licensedYours) {
                    if view == nil || available.isEmpty {
                        SettingsRow(title: L.licensedLoading)
                    }
                    ForEach(shown, id: \.self) { service in
                        let on = store.licensed.uses(service)
                        Button { toggle(service) } label: {
                            SettingsRow(title: service) {
                                if on { Image(systemName: "checkmark").foregroundStyle(.amber) }
                            }
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(on ? .isSelected : [])
                    }
                    if !showAll, available.count > Self.firstShown {
                        Button { withAnimation(.standard) { showAll = true } } label: {
                            SettingsRow(title: L.licensedShowAll(count: available.count))
                        }
                        .buttonStyle(.plain)
                    }
                }
                SettingsGroup(footnote: L.licensedShowOthersLine) {
                    SettingsRow(title: L.licensedShowOthers) {
                        Toggle(L.licensedShowOthers, isOn: Binding(
                            get: { view?.settings.showOthers ?? true },
                            set: { store.setLicensedShowOthers($0) }
                        ))
                        .toggleStyle(.switch).tint(.amber).labelsHidden()
                    }
                }
            }
            .padding(EdgeInsets(top: 18, leading: 24, bottom: 24, trailing: 24))
        }
        .scrollIndicators(.never)
        .task { store.loadLicensed() }
    }

    private func toggle(_ service: String) {
        var services = store.licensed.yours
        if let at = services.firstIndex(where: { $0.caseInsensitiveCompare(service) == .orderedSame }) {
            services.remove(at: at)
        } else {
            services.append(service)
        }
        store.setLicensedServices(services)
    }
}

/// A server's page: who is signed in, its libraries, the report switch, and Remove server.
struct ServerPage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let id: String

    var body: some View {
        if let row = store.servers.row(id) {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    PageHeading(
                        title: row.name,
                        line: L.serversPageLine(address: row.address, version: row.version, user: row.user))
                    if row.signedOut || row.signedOutHere {
                        HStack(spacing: 8) {
                            Circle().fill(.vermilion).frame(width: 6, height: 6)
                            Text(row.signedOutHere ? L.serversRowSignedOutHere : L.serversRowSignedOut)
                                .font(.mac(13)).foregroundStyle(.silk)
                            Button(L.serversSignInAgain) { navigator.addServerShown = true }
                                .buttonStyle(SecondaryCapsuleStyle())
                        }
                    }
                    SettingsGroup(title: L.serversLibraries) {
                        ForEach(row.libraries, id: \.id) { library in
                            Button { toggle(library, in: row) } label: {
                                SettingsRow(title: library.name, note: library.kind) {
                                    if library.ticked { Image(systemName: "checkmark").foregroundStyle(.amber) }
                                }
                            }
                            .buttonStyle(.plain)
                            .accessibilityAddTraits(library.ticked ? .isSelected : [])
                        }
                    }
                    SettingsGroup(footnote: L.serversReportLine) {
                        SettingsRow(title: L.serversReport) {
                            Toggle(L.serversReport, isOn: Binding(
                                get: { row.report }, set: { store.setServerReport(row.id, $0) }))
                                .toggleStyle(.switch).tint(.amber).labelsHidden()
                        }
                    }
                    HStack(spacing: 8) {
                        Button(L.serversRefresh) { store.refreshServer(row.id) }
                        if !row.signedOut, !row.signedOutHere {
                            Button(L.serversSignOut) { store.signOutOfServer(row.id) }
                        }
                        Button(L.serversRemove) { store.removeServer(row.id) }
                    }
                    .buttonStyle(SecondaryCapsuleStyle())
                    Text(L.serversRemoveLine).font(.mac(12)).foregroundStyle(.haze).padding(.top, -12)
                }
                .padding(EdgeInsets(top: 18, leading: 24, bottom: 24, trailing: 24))
            }
            .scrollIndicators(.never)
        }
    }

    private func toggle(_ library: LibraryChoice, in row: ServerRow) {
        var ticked = row.libraries.filter(\.ticked).map(\.id)
        if library.ticked { ticked.removeAll { $0 == library.id } } else { ticked.append(library.id) }
        store.setServerLibraries(row.id, ticked)
    }
}

/// A folder's page: its state, Rescan, Show in Finder and Remove folder, then the lists.
struct FolderPage: View {
    @Environment(AppStore.self) private var store
    let path: String
    @State private var matching: LocalGroup?
    @State private var numbering: UnnumberedFile?

    var body: some View {
        if let folder = store.local.folders.first(where: { $0.path == path }) {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    PageHeading(
                        title: URL(filePath: folder.path).lastPathComponent,
                        line: (folder.path as NSString).abbreviatingWithTildeInPath)
                    TimelineView(.periodic(from: .now, by: 1)) { context in
                        Text(stateLine(folder, now: context.date))
                            .font(.mac(13)).foregroundStyle(.haze).monospacedDigit()
                            .contentTransition(.opacity)
                    }
                    HStack(spacing: 8) {
                        Button(L.libraryRescan) { store.rescanFolders() }
                        #if os(macOS)
                        Button(L.libraryShowFinder) {
                            NSWorkspace.shared.activateFileViewerSelecting([URL(filePath: folder.path)])
                        }
                        #endif
                        Button(L.libraryRemoveFolder) { store.removeFolder(folder.path) }
                    }
                    .buttonStyle(SecondaryCapsuleStyle())
                    Text(L.libraryRemoveLine).font(.mac(12)).foregroundStyle(.haze).padding(.top, -12)
                    list(.needsLook, title: L.libraryNeedsLook, folder: folder)
                    list(.notMatched, title: L.libraryNotMatched, folder: folder)
                    matched(folder)
                }
                .padding(EdgeInsets(top: 18, leading: 24, bottom: 24, trailing: 24))
            }
            .scrollIndicators(.never)
            .animation(.standard, value: store.local.view?.groups.map(\.id))
            .sheet(item: $matching) { group in FolderMatchSheet(group: group) }
            .sheet(item: $numbering) { file in SetEpisodeSheet(file: file) }
        }
    }

    private func stateLine(_ folder: FolderRow, now: Date) -> String {
        if !folder.available { return L.libraryRowDriveOffline }
        switch folder.scan {
        case .scanning(let files)?: return L.libraryRowScanning(files: Int(files))
        case .matching(let progress)?:
            return FirstPartySections.matchingLine(progress, left: store.local.waitLeft(folder.path, at: now))
        case nil: return L.libraryRowFiles(files: Int(folder.files))
        }
    }

    @ViewBuilder
    private func list(_ state: GroupState, title: String, folder: FolderRow) -> some View {
        let groups = store.local.groups(in: folder.path, state)
        let files = state == .notMatched ? store.local.unnumbered.filter { $0.folder == folder.path } : []
        if !groups.isEmpty || !files.isEmpty {
            SettingsGroup(title: title) {
                ForEach(groups, id: \.id) { group in
                    SettingsRow(title: group.name, note: note(group)) {
                        HStack(spacing: 8) {
                            Button(L.libraryChooseMatch) { matching = group }
                            if state == .notMatched {
                                Button(L.libraryKeepUnmatched) { store.keepLocal(group: group.id) }
                            }
                        }
                        .buttonStyle(SecondaryCapsuleStyle(height: 24, raisedSurface: true, fontSize: 12))
                    }
                    .transition(.opacity)
                }
                ForEach(files, id: \.path) { file in
                    SettingsRow(
                        title: file.place.isEmpty ? file.name : file.place + "/" + file.name,
                        note: L.libraryReasonNoEpisode
                    ) {
                        Button(L.librarySetEpisode) { numbering = file }
                            .buttonStyle(SecondaryCapsuleStyle(height: 24, raisedSurface: true, fontSize: 12))
                    }
                    .transition(.opacity)
                }
            }
        }
    }

    @ViewBuilder
    private func matched(_ folder: FolderRow) -> some View {
        let groups = store.local.groups(in: folder.path, .matched) + store.local.groups(in: folder.path, .kept)
        if !groups.isEmpty {
            SettingsGroup(title: L.libraryMatchedApple) {
                ForEach(groups, id: \.id) { group in
                    SettingsRow(
                        title: group.matchedTitle ?? group.name,
                        note: group.state == .kept ? L.libraryKept : L.libraryFiles(count: Int(group.files)))
                }
            }
        }
    }

    private func note(_ group: LocalGroup) -> String {
        var parts = [L.libraryFiles(count: Int(group.files))]
        switch group.state {
        case .needsLook:
            if let title = group.matchedTitle {
                parts.append(L.libraryBestGuess(title: title, percent: Int(group.confidence ?? 0)))
            }
        case .notMatched:
            parts.append(group.reason == .offline ? L.libraryReasonOffline : L.libraryReasonNoResult)
        default: break
        }
        return parts.joined(separator: " · ")
    }
}

extension LocalGroup: @retroactive Identifiable {}
extension UnnumberedFile: @retroactive Identifiable {
    public var id: String { path }
}

/// Which AniList title a group of files is: AniList is asked, and one click picks.
struct FolderMatchSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let group: LocalGroup
    @State private var query = ""
    @State private var pending: Task<Void, Never>?
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(L.libraryFixTitle).font(.mac(15, weight: .semibold)).foregroundStyle(.silk)
            Text("\(group.name) · \(L.libraryFiles(count: Int(group.files)))")
                .font(.mac(12)).foregroundStyle(.haze).padding(.top, 2)
            TextField(L.libraryFixSearch, text: $query)
                .textFieldStyle(.plain)
                .font(.mac(13))
                .padding(.horizontal, 12)
                .frame(height: 32)
                .background(.ground, in: .rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
                .focused($focused)
                .padding(.top, 14)
                .onChange(of: query) { _, text in search(after: .milliseconds(300), text) }
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    if let found = store.local.matches, found.group == group.id {
                        if found.offline {
                            Text(L.libraryFixOffline).font(.mac(12)).foregroundStyle(.haze).padding(8)
                        } else if found.choices.isEmpty {
                            Text(L.libraryFixNone).font(.mac(12)).foregroundStyle(.haze).padding(8)
                        }
                        ForEach(found.choices, id: \.anilist) { choice in row(choice) }
                    } else {
                        Text(L.libraryFixHint).font(.mac(12)).foregroundStyle(.haze).padding(8)
                    }
                }
                .padding(.top, 8)
                .animation(.fadeIn, value: store.local.matches)
            }
            .frame(height: 260)
            HStack {
                Button(L.libraryLeaveUnmatched) {
                    store.keepLocal(group: group.id)
                    dismiss()
                }
                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                Spacer()
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
            }
            .padding(.top, 12)
        }
        .padding(24)
        .sheetWidth(480)
        .background(.raised)
        .task {
            // The name read from the files is searched at once: an empty query says so.
            query = group.matchedTitle ?? group.name
            store.findLocalMatch(group: group.id, query: "")
            focused = true
        }
        .onDisappear {
            pending?.cancel()
            store.local.clearMatches()
        }
        .closesOnEscape { dismiss() }
    }

    private func search(after delay: Duration, _ text: String) {
        pending?.cancel()
        pending = Task {
            try? await Task.sleep(for: delay)
            guard !Task.isCancelled else { return }
            store.findLocalMatch(group: group.id, query: text)
        }
    }

    /// "TV · 2026 · 12 episodes · 71% match".
    private static func facts(_ choice: MatchChoice) -> String {
        [
            choice.format.map(TitleWords.format),
            choice.year.map { String($0) },
            choice.episodes.flatMap { $0 > 0 ? L.episodesCount(count: Int($0)) : nil },
        ]
        .compactMap { $0 }
        .joined(separator: " · ")
    }

    private func row(_ choice: MatchChoice) -> some View {
        Button {
            store.chooseLocalMatch(group: group.id, title: choice.anilist)
            dismiss()
        } label: {
            VStack(alignment: .leading, spacing: 1) {
                Text(choice.title).font(.mac(13)).foregroundStyle(.silk).lineLimit(1)
                Text(Self.facts(choice) + " · " + L.libraryMatchPercent(percent: Int(choice.confidence)))
                    .font(.mac(12)).foregroundStyle(.haze).lineLimit(1)
                if !choice.fits {
                    Text(L.libraryFixCountMismatch(titleEpisodes: Int(choice.episodes ?? 0), files: Int(group.files)))
                        .font(.mac(12)).foregroundStyle(.haze).lineLimit(1)
                }
            }
            .opacity(choice.fits ? 1 : 0.6)
            .padding(.horizontal, 8)
            .frame(maxWidth: .infinity, minHeight: 46, alignment: .leading)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
    }
}

/// Which episode a file is, for a file whose name has no number. Season 0 holds specials.
struct SetEpisodeSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let file: UnnumberedFile
    @State private var season = 1
    @State private var episode = 1

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L.libraryEpisodeTitle).font(.mac(15, weight: .semibold)).foregroundStyle(.silk)
            Text(file.name).font(.mac(12)).foregroundStyle(.haze).lineLimit(2)
            Stepper(value: $season, in: 0...99) {
                Text("\(L.librarySeason) \(season)").font(.mac(13)).foregroundStyle(.silk).monospacedDigit()
            }
            Stepper(value: $episode, in: 1...999) {
                Text("\(L.libraryEpisode) \(episode)").font(.mac(13)).foregroundStyle(.silk).monospacedDigit()
            }
            HStack {
                Spacer()
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button(L.libraryApply) {
                    store.setEpisode(group: file.group, path: file.path, season: UInt32(season), episode: UInt32(episode))
                    dismiss()
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 28, fontSize: 13))
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .sheetWidth(380)
        .background(.raised)
        .closesOnEscape { dismiss() }
    }
}

// MARK: Licensed services on a title page

extension AppStore {
    /// Leave for a service: the engine remembers it, and the system opens the address in the
    /// browser, or the service's own app when it has claimed the link.
    @MainActor
    func openService(_ id: AnilistId, link: ServiceLink, episode: UInt32?) {
        guard let url = URL(string: link.url) else { return }
        openLicensed(id, service: link.service, url: link.url, episode: episode)
        #if os(macOS)
        NSWorkspace.shared.open(url)
        #else
        UIApplication.shared.open(url)
        #endif
    }
}

/// What a title page says under its buttons about the services: Also on, the services they don't
/// use, and in the third case Choose your services. Asks the engine when the page opens and when
/// the page learns whether a source plays the title.
///
/// While the next title's answer is on its way, as when a season is picked on the Mac's title
/// page, the last title's lines keep their room, empty, so the title and the buttons above them
/// stay where they are until it lands.
struct LicensedLines: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let id: AnilistId
    let font: Font
    /// The last answer shown, whose room is kept while the next is on its way.
    @State private var held: TitleAvailability?

    var body: some View {
        let playsHere = !store.titles.unlisted(id)
        let known = store.licensed.availability[id]
        let shown = known ?? held
        lines(shown)
            .opacity(known == nil ? 0 : 1)
            .allowsHitTesting(known != nil)
            .accessibilityHidden(known == nil)
            .font(font)
            .padding(.top, shown.map { $0.case == .playsHere && $0.others.isEmpty ? 0 : 10 } ?? 0)
            .animation(.fadeIn, value: known)
            .onChange(of: known, initial: true) { if let known { held = known } }
            .task(id: "\(id)-\(playsHere)-\(store.licensed.yours)") {
                store.loadAvailability(id, playsHere: playsHere)
            }
    }

    private func lines(_ availability: TitleAvailability?) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            if let a = availability {
                switch a.case {
                case .playsHere:
                    others(a)
                case .playsAlsoOn, .onService:
                    if !a.alsoOn.isEmpty {
                        HStack(spacing: 4) {
                            ForEach(Array(a.alsoOn.enumerated()), id: \.element.service) { index, link in
                                Button { store.openService(id, link: link, episode: nil) } label: {
                                    HStack(spacing: 4) {
                                        Text(index == 0 ? L.licensedAlsoOn(services: link.service) : link.service)
                                        Image(systemName: "arrow.up.right").font(.system(size: 9, weight: .semibold))
                                    }
                                }
                                .buttonStyle(.plain)
                                .foregroundStyle(.silk)
                            }
                        }
                    }
                    others(a)
                case .notInSources:
                    if !a.others.isEmpty {
                        Text(L.licensedListedOn(services: a.others.formatted(.list(type: .and))))
                            .foregroundStyle(.haze)
                    }
                    if a.chooseServices || !a.others.isEmpty {
                        Button(L.licensedChooseServices) { navigator.openLicensedServices() }
                            .buttonStyle(.plain)
                            .foregroundStyle(.amber)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func others(_ a: TitleAvailability) -> some View {
        if !a.others.isEmpty {
            Text(L.licensedOthers(services: a.others.formatted(.list(type: .and)))).foregroundStyle(.haze)
        }
    }
}
