// Providers on the Mac and a wide iPad: the sources in the order failover tries them, moved by
// dragging or ⌥⌘↑ and ⌥⌘↓, and the selected one's version, speed and what it may contact.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// What the Providers screen says of one source, from its row and the stores it came from.
struct ProviderInfo {
    let row: ProviderRow
    let name: String
    let origin: String
    /// The store that keeps it updated, and its listing there.
    let listing: (store: StoreRow, plugin: StorePluginRow)?

    var isDemo: Bool { row.isDemo }
    var isPlugin: Bool { row.kind == "plugin" }

    /// A newer version its store offers.
    var update: String? {
        if case .update = listing?.plugin.status { return listing?.plugin.version }
        return nil
    }

    /// The hosts it may contact: its store listing's, or what the plugin declares when it was added by hand.
    var hosts: [String] { listing?.plugin.hosts ?? row.hosts }
}

extension AppStore {
    /// What Providers says of `row`: its name, where it came from and its store's listing.
    func info(_ row: ProviderRow) -> ProviderInfo {
        let listing = stores.origin(of: row.id)
        let name: String
        let origin: String
        if row.isDemo {
            return ProviderInfo(row: row, name: L.sourcesDemoName, origin: L.sourcesDemoLine, listing: nil)
        }
        switch row.kind {
        case "remote":
            (name, origin) = (L.sourcesRemote, L.sourcesOriginConfig)
        case "demo":
            (name, origin) = (row.name, L.sourcesOriginBuiltIn)
        default:
            name = listing?.row.name ?? row.id
            if case .addedByHand = listing?.row.status {
                origin = L.sourcesOriginByHand
            } else {
                origin = listing.map { $0.store.name } ?? L.sourcesOriginByHand
            }
        }
        return ProviderInfo(row: row, name: name, origin: origin, listing: listing.map { ($0.store, $0.row) })
    }
}

/// One heading of the plugin part of the page and the rows under it.
struct SourceSection {
    let title: String
    let rows: [ProviderRow]
}

extension AppStore {
    /// The rows after the first-party kinds as the board draws them: each store's plugins under
    /// "From <store>", plugins put in by hand, then what is built in, the demo last.
    func sourceSections(_ rows: [ProviderRow]) -> [SourceSection] {
        var sections: [SourceSection] = []
        func add(_ title: String, _ row: ProviderRow) {
            if let i = sections.firstIndex(where: { $0.title == title }) {
                sections[i] = SourceSection(title: title, rows: sections[i].rows + [row])
            } else {
                sections.append(SourceSection(title: title, rows: [row]))
            }
        }
        var builtIn: [ProviderRow] = []
        for row in rows {
            switch row.kind {
            case "library", "server", "licensed", "catalogue":
                break
            case "remote", "demo":
                builtIn.append(row)
            default:
                let listing = info(row).listing
                if case .addedByHand? = listing?.plugin.status {
                    add(L.sourcesSectionPlugins, row)
                } else if let name = listing?.store.name {
                    add(L.sourcesOriginFrom(name: name), row)
                } else {
                    add(L.sourcesSectionPlugins, row)
                }
            }
        }
        if !builtIn.isEmpty {
            sections.append(SourceSection(title: L.sourcesSectionBuiltin, rows: builtIn))
        }
        return sections
    }
}

extension AppStore {
    /// Whether the engine has said there is nothing to watch from: no plugin.
    var hasNoSources: Bool {
        providers.loaded && providers.rows.isEmpty && !hasFirstParty
    }
}

/// Providers with no source: Nagare, what a source is and where one comes from, and the one way
/// to get one (the spec's empty states).
struct NoSources: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        EmptyState(
            title: L.sourcesEmptyTitle,
            line: devicePicksFolders ? L.sourcesEmptyBody : L.sourcesEmptyBodyMobile
        ) {
            // The one thing to do is amber: a folder where the device has them, else a server.
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 10) {
                    if devicePicksFolders {
                        Button(L.sourcesAddFolderButton) { FolderPicker.choose { store.addFolder($0) } }
                            .buttonStyle(PrimaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
                        Button(L.sourcesAddServerButton) { navigator.addServerShown = true }
                            .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
                    } else {
                        Button(L.sourcesAddServerButton) { navigator.addServerShown = true }
                            .buttonStyle(PrimaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
                    }
                }
                Button(L.storesJoinAction) { navigator.invite = "" }
                    .buttonStyle(.plain)
                    .font(.mac(13))
                    .foregroundStyle(.amber)
                if store.demo.state.offer {
                    Button(L.sourcesDemoAdd) { store.addDemo() }
                        .buttonStyle(.plain)
                        .font(.mac(13))
                        .foregroundStyle(.amber)
                }
            }
        }
    }
}

struct ProvidersScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var selection: String?

    var body: some View {
        let rows = store.orderedProviders
        Group {
            if store.hasNoSources {
                NoSources()
                    .task {
                        store.checkProviders()
                        if !store.stores.loaded { store.loadStores() }
                        store.loadServers()
                        if devicePicksFolders { store.loadLocal() }
                    }
            } else {
                list(rows)
            }
        }
        .sheet(isPresented: Bindable(navigator).addServerShown) { AddServerSheet() }
    }

    /// One page, with a row per source. A row pushes its own page, and Back returns to the list.
    private func list(_ rows: [ProviderRow]) -> some View {
        let shown = selection.map { store.resolvedSourceKey($0) ?? $0 }
        // A stack, not a Group, so the page and the list crossfade.
        return ZStack(alignment: .topLeading) {
            if let shown {
                VStack(alignment: .leading, spacing: 0) {
                    Button { selection = nil } label: {
                        Label(L.sourcesYour, systemImage: "chevron.left")
                            .font(.mac(13, weight: .medium))
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.amber)
                    .padding(EdgeInsets(top: 10, leading: 24, bottom: 0, trailing: 0))
                    .keyboardShortcut(.cancelAction)
                    HStack(alignment: .top, spacing: 0) {
                        if shown == SourceKey.licensed {
                            LicensedServicesPage()
                        } else if let id = SourceKey.serverId(shown) {
                            ServerPage(id: id)
                        } else if let path = SourceKey.folderPath(shown) {
                            FolderPage(path: path)
                        } else if let row = rows.first(where: { $0.id == shown }) {
                            ProviderDetail(info: store.info(row))
                                .id(row.id)
                        }
                    }
                    .frame(maxWidth: 720, alignment: .topLeading)
                }
                .transition(.opacity)
            } else {
                SourceOrderList(rows: rows, selection: $selection)
                    .transition(.opacity)
            }
        }
        .animation(.standard, value: selection)
        .padding(.leading, 6)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .task {
            store.checkProviders()
            if !store.stores.loaded { store.loadStores() }
            // The page asked for is opened once, and the next visit starts at the list.
            if selection == nil, let key = navigator.launchSourceKey { selection = key }
            navigator.launchSourceKey = nil
        }
        .onChange(of: navigator.launchSourceKey) { _, key in
            guard let key else { return }
            selection = key
            navigator.launchSourceKey = nil
        }
    }
}

/// Your sources, one page: what plays from this device, then the plugins in failover order (drag
/// one by its handle to move it). A section with no rows is left out.
struct SourceOrderList: View {
    @Environment(AppStore.self) private var store
    let rows: [ProviderRow]
    @Binding var selection: String?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(L.sourcesYourIntro)
                        .font(.mac(13))
                        .foregroundStyle(.haze)
                    Spacer(minLength: 0)
                    AddSourceMenu()
                }
                .padding(EdgeInsets(top: 8, leading: 18, bottom: 6, trailing: 0))
                FirstPartySections(selection: $selection, current: nil)
                if store.providers.rebuilding {
                    Text(L.sourcesRebuilding)
                        .font(.mac(12))
                        .foregroundStyle(.haze)
                        .padding(.horizontal, 18)
                        .padding(.top, 10)
                        .padding(.bottom, 6)
                }
                ForEach(store.sourceSections(rows), id: \.title) { section in
                    Text(section.title)
                        .groupHeaderStyle()
                        .accessibilityAddTraits(.isHeader)
                        .macLineHeight(14)
                        .padding(EdgeInsets(top: 10, leading: 18, bottom: 8, trailing: 0))
                    ForEach(section.rows, id: \.id) { row in
                        SourceLine(info: store.info(row), selected: false)
                            .onTapGesture { selection = row.id }
                            .accessibilityAction(named: L.sourcesMoveUp) { store.moveSource(row.id, by: -1) }
                            .accessibilityAction(named: L.sourcesMoveDown) { store.moveSource(row.id, by: 1) }
                            .modifier(DemoStays(isDemo: row.isDemo, id: row.id))
                            .dropDestination(for: String.self) { ids, _ in
                                guard let id = ids.first, id != row.id,
                                      let index = rows.firstIndex(where: { $0.id == row.id }) else { return false }
                                store.moveSource(id, to: index)
                                return true
                            }
                    }
                }
                VStack(alignment: .leading, spacing: 8) {
                    if store.demo.state.only { Text(L.sourcesDemoOnly) }
                    Text(L.sourcesOrderDetail)
                }
                .font(.mac(12))
                .macLineHeight(17)
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(EdgeInsets(top: 14, leading: 18, bottom: 0, trailing: 12))
            }
            .frame(maxWidth: 720, alignment: .leading)
            .padding(EdgeInsets(top: 4, leading: 4, bottom: 16, trailing: 8))
        }
        .scrollIndicators(.never)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(L.sourcesYour)
    }
}

/// One source in the order: its handle, name, where it came from, and how it stands.
struct SourceLine: View {
    let info: ProviderInfo
    let selected: Bool

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "line.3.horizontal")
                .font(.mac(11, weight: .semibold))
                .foregroundStyle(.haze)
                .frame(width: 12)
                .opacity(info.isDemo ? 0 : 1)
            VStack(alignment: .leading, spacing: 1) {
                Text(info.name)
                    .font(.mac(13, weight: .medium))
                    .macLineHeight(17)
                    .foregroundStyle(selected ? .amber : .silk)
                Text(info.origin)
                    .font(.mac(12))
                    .macLineHeight(15)
                    .foregroundStyle(.haze)
            }
            Spacer(minLength: 0)
            ProviderStandingLabel(info: info, selected: selected)
            Image(systemName: "chevron.right")
                .font(.mac(11, weight: .semibold))
                .foregroundStyle(.haze)
        }
        .padding(.horizontal, 12)
        .frame(height: 44)
        .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
        .contentShape(.rect(cornerRadius: 8))
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// How a source stands, beside its name: failing, held back, slow, or with an update waiting.
struct ProviderStandingLabel: View {
    let info: ProviderInfo
    var selected = false
    var size: CGFloat = 12

    var body: some View {
        switch info.row.standing {
        case .failing:
            // Silk words beside the vermilion dot, as the boards have it: the dot says what is
            // wrong, and red text would only shout it.
            label(L.sourcesStatusNotAnswering, dot: .vermilion, color: .silk)
        case .heldBack:
            label(L.sourcesStatusHeldBack, ring: true, color: .haze)
        case .off:
            label(L.valueOff, ring: true, color: .silk)
        case .slow:
            label(L.sourcesStatusSlow, dot: .haze, color: .haze)
        case .working:
            if info.update != nil {
                label(L.sourcesStatusUpdate, dot: .water, color: .silk)
            }
        }
    }

    private func label(_ text: String, dot: Color? = nil, ring: Bool = false, color: Color) -> some View {
        HStack(spacing: 6) {
            if ring {
                Circle().strokeBorder(.haze, lineWidth: 1.5).frame(width: 6, height: 6)
            } else if let dot {
                Circle().fill(dot).frame(width: 6, height: 6)
            }
            Text(text).font(.mac(size)).foregroundStyle(color)
        }
    }
}

/// The selected source: what it is, what can be done with it, and what it may contact.
struct ProviderDetail: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let info: ProviderInfo
    @State private var removing = false
    @State private var updating: StorePluginRow?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(info.name)
                    .font(.mac(22, weight: .bold))
                    .macLineHeight(28)
                    .foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader)
                Text(info.isPlugin && info.origin != L.sourcesOriginByHand ? L.sourcesOriginFrom(name: info.origin) : info.origin)
                    .font(.mac(13))
                    .foregroundStyle(.haze)
                    .padding(.top, 2)
                if let line = stateLine {
                    HStack(spacing: 8) {
                        Circle().fill(info.row.standing == .failing ? .vermilion : .haze).frame(width: 6, height: 6)
                        Text(line)
                            .font(.mac(13))
                            .macLineHeight(18)
                            .foregroundStyle(.haze)
                    }
                    .padding(.top, 14)
                }
                if info.row.torrents {
                    Text(L.sourcesPlaysTorrents)
                        .font(.mac(13))
                        .macLineHeight(18)
                        .foregroundStyle(.haze)
                        .padding(.top, 14)
                }
                if info.isDemo {
                    DemoAbout().padding(.top, 20)
                } else {
                    pluginActions
                    facts
                }
            }
            .padding(EdgeInsets(top: 18, leading: 24, bottom: 24, trailing: 24))
        }
        .scrollIndicators(.never)
        .confirmationDialog(L.sourcesRemoveTitle(name: info.name), isPresented: $removing) {
            Button(L.actionRemove, role: .destructive) { store.removePlugin(info.row.id) }
        } message: {
            Text(L.sourcesRemoveDetailDevice)
        }
        .sheet(item: $updating) { plugin in
            if let listing = info.listing { UpdatePluginSheet(store: listing.store, plugin: plugin) }
        }
    }

    private var stateLine: String? {
        switch info.row.standing {
        // The site's own error is in Messages and the bug report, not here (copy rule 5).
        case .failing: L.sourcesDetailNotAnsweringPlain
        case .heldBack: L.sourcesDetailHeldBack
        default: nil
        }
    }

    private var pluginActions: some View {
        HStack(spacing: 8) {
            if let version = info.update, let plugin = info.listing?.plugin {
                Button(L.sourcesUpdateTo(version: version)) { updating = plugin }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
            }
            if info.row.standing == .failing {
                let checking = store.jobs.running.contains { $0.activity == "checking providers" }
                Button(L.actionCheckAgain) { store.checkProviders() }
                    .buttonStyle(SecondaryCapsuleStyle())
                    .disabled(checking)
                Working(checking)
            }
            // Off without removing it: its source leaves the chain until it is on again.
            let disabled = store.isPluginDisabled(info.row.id)
            Button(disabled ? L.actionEnable : L.actionDisable) { store.setPluginDisabled(info.row.id, !disabled) }
                .buttonStyle(SecondaryCapsuleStyle())
            Button(L.actionRemove) { removing = true }
                .buttonStyle(SecondaryCapsuleStyle())
                .disabled(store.stores.isWorking(.remove(plugin: info.row.id)))
        }
        .padding(.top, 16)
    }

    private var facts: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(spacing: 0) {
                fact(L.sourcesVersion) {
                    if let listing = info.listing {
                        Text(listing.plugin.installedVersion).foregroundStyle(.silk)
                        if let update = info.update { Text(L.sourcesUpdateAvailableVersion(version: update)).foregroundStyle(.haze) }
                    } else {
                        Text(L.sourcesOriginNoStore).foregroundStyle(.haze)
                    }
                }
                Rectangle().fill(.faintRule).frame(height: 1)
                fact(L.sourcesResponseTime) {
                    if info.row.standing == .failing {
                        Text(L.sourcesNoAnswer).foregroundStyle(.silk)
                    } else if let latency = info.row.latencyMs {
                        Text(L.sourcesResponseMs(ms: Int(latency))).foregroundStyle(.silk).monospacedDigit()
                    } else {
                        Text(L.sourcesNotChecked).foregroundStyle(.haze)
                    }
                }
                Rectangle().fill(.faintRule).frame(height: 1)
                fact(L.sourcesContactsLabel) {
                    VStack(alignment: .leading, spacing: 6) {
                        if info.hosts.isEmpty {
                            Text(L.sourcesNoHosts).foregroundStyle(.haze)
                        }
                        ForEach(info.hosts, id: \.self) { Text($0).foregroundStyle(.silk) }
                    }
                }
            }
            .padding(.horizontal, 14)
            .background(.raised, in: .rect(cornerRadius: 12))
            Text("\(info.name) can contact only these hosts and cannot read your files.")
                .font(.mac(12))
                .macLineHeight(17)
                .foregroundStyle(.haze)
                .padding(EdgeInsets(top: 8, leading: 14, bottom: 0, trailing: 0))
        }
        .padding(.top, 12)
    }

    private func fact(_ label: String, @ViewBuilder value: () -> some View) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 0) {
            Text(label).foregroundStyle(.haze).frame(width: 120, alignment: .leading)
            HStack(alignment: .firstTextBaseline, spacing: 8) { value() }
            Spacer(minLength: 0)
        }
        .font(.mac(13))
        .padding(.vertical, 12)
    }
}

extension StorePluginRow: @retroactive Identifiable {}

/// The demo is always last and has no handle: it cannot be dragged, only dropped past.
private struct DemoStays: ViewModifier {
    let isDemo: Bool
    let id: String

    @ViewBuilder
    func body(content: Content) -> some View {
        if isDemo { content } else { content.draggable(id) }
    }
}
