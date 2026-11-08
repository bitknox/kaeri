// The Mac's main window: the sidebar, the toolbar each section dresses, the section on screen and
// a title's page over it, the palette and the notice capsule.

#if os(macOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct MacShell: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// Where a title's art flies from the element clicked to its page (TitleOpening).
    @Namespace private var titleSpace

    var body: some View {
        @Bindable var navigator = navigator
        NavigationSplitView {
            Sidebar(selection: $navigator.section)
                // 220 wide, as the boards draw it. macOS 26 takes the column width; macOS 27
                // ignores it and opens the column at 144, where "Plugin stores" is cut, or at
                // whatever it saved. A minimum on the column's content holds on both, and brings
                // a saved 144 back to 220.
                .frame(minWidth: 220)
                .navigationSplitViewColumnWidth(220)
                .toolbar(removing: .sidebarToggle)
                // The brief's navy glass. macOS draws its sidebar glass a slate of its own, grey
                // in an inactive window, and has no tint for it: `.navigationSplitView` container
                // backgrounds are iOS only, and a tint on the sidebar's NSGlassEffectView lifts it
                // to a light grey. So the column lays its tint over that glass, inside its shape.
                .background(.sidebarFill)
        } detail: {
            ZStack {
                SectionView(section: navigator.section)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .background(.ground)
                    // Under a page the section's banners are not where the player grows from.
                    .environment(\.playerZoomAside, navigator.pageCovers ? "section" : nil)
                if let id = navigator.titleOnMac {
                    TitlePage(id: id)
                        // The page crossfades over the section on standard, as it opens and on Back;
                        // art in flight stays whole above both.
                        .transition(.opacity)
                        .zIndex(1)
                        // Nor is the page's banner until the page has come in: the player never
                        // pairs the two, and the page's art never grows out of the section's.
                        .environment(\.playerZoomAside, navigator.pageSettled ? nil : "page")
                        .environment(\.titleAwaited, navigator.awaited)
                }
                if let work = navigator.seriesOnMac, navigator.titleOnMac == nil {
                    // A series' page crossfades over the section as a show's does.
                    SeriesPage(work: work)
                        .id(work)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                        .background(.ground)
                        .transition(.opacity)
                        .zIndex(1)
                }
                if let flight = navigator.flight {
                    TitleFlyer(flight: flight, space: titleSpace)
                        // Each flight its own flyer, so one started while the last is still in
                        // the air takes off too, rather than staying where it started.
                        .id(flight.trip)
                        .transition(.identity)
                        .zIndex(2)
                }
            }
            .environment(\.titleSpace, titleSpace)
            .onChange(of: reduceMotion, initial: true) { _, reduced in navigator.motionReduced = reduced }
            // Downloads says its own, with the storage meter beside the line (MacDownloads).
            .navigationTitle(macTitle)
            .navigationSubtitle(navigator.section == .downloads ? "" : subtitle)
            .toolbar {
                if let id = navigator.titleOnMac {
                    TitleToolbar(store: store, navigator: navigator, id: id)
                } else {
                    if navigator.seriesOnMac != nil {
                        ToolbarItem(placement: .navigation) {
                            Button(L.actionBack, systemImage: "chevron.left") { navigator.closeSeries() }
                                .keyboardShortcut("[", modifiers: .command)
                        }
                    }
                    if navigator.section == .providers, navigator.bookSourcePage != nil {
                        ToolbarItem(placement: .navigation) {
                            Button(L.actionBack, systemImage: "chevron.left") { navigator.bookSourcePage = nil }
                                .keyboardShortcut("[", modifiers: .command)
                        }
                    }
                    // Search opened by a Home row's View all: Back returns to Home where it was.
                    if navigator.section == .search, navigator.seriesOnMac == nil, navigator.readSearchBack != nil {
                        ToolbarItem(placement: .navigation) {
                            Button(L.actionBack, systemImage: "chevron.left") { navigator.closeReadSearch() }
                                .keyboardShortcut("[", modifiers: .command)
                        }
                    }
                    if navigator.section == .downloads, navigator.seriesOnMac == nil {
                        // The title on the toolbar itself, as a title is, not in a glass capsule.
                        ToolbarItem(placement: .navigation) { DownloadsTitle() }
                            .sharedBackgroundVisibility(.hidden)
                    }
                    if navigator.section == .together, store.party.lobbyShown {
                        ToolbarItem(placement: .navigation) { TogetherLobbyBack() }
                    }
                    // What has run past 600 ms, quietly, beside the section's own items.
                    ToolbarItem(placement: .primaryAction) { ActivityIndicator() }
                    // A book source's page has no Add a source: Back is all its toolbar adds.
                    if navigator.seriesOnMac == nil, navigator.bookSourcePage == nil {
                        SectionToolbar(section: navigator.section)
                    }
                    if store.tv.connected != nil {
                        ToolbarItem(placement: .primaryAction) { TVNowPlayingButton() }
                    }
                    ToolbarItem(placement: .primaryAction) {
                        SyncCapsule { navigator.openSettings(.accounts) }
                    }
                }
            }
            .toolbarBackgroundVisibility(navigator.titleOnMac == nil ? .automatic : .hidden, for: .windowToolbar)
            .titleSearchField(
                text: navigator.section == .settings ? $navigator.settingsQuery : $navigator.query,
                titles: navigator.section != .settings && !ReadSearchScope.active(store: store, navigator: navigator),
                placement: .toolbar,
                prompt: navigator.section == .settings ? L.shellSearchSettings
                    : ReadSearchScope.active(store: store, navigator: navigator) ? L.readSearchPlaceholder : L.searchPlaceholder
            )
            .modifier(ReadSearchTyping())
        }
        .containerBackground(Palette.ground, for: .window)
        .overlay {
            if navigator.paletteShown {
                PaletteOverlay()
            }
        }
        .overlay(alignment: .bottom) {
            NoticeCapsule().padding(.bottom, 16)
        }
        // Messages, the show's settings, More about and the source picker lose nothing to a click
        // outside, so they are cards over the window (Light dismiss).
        .infoSurface(isPresented: $navigator.messagesShown) {
            MessagesView()
        }
        .modifier(AppSheets())
        .infoSurface(item: Binding<ShowItem?>(
            get: { navigator.showSettings.flatMap { store.titles[$0] }.map(ShowItem.init) },
            set: { (item: ShowItem?) in navigator.showSettings = item?.entry.id }
        )) { item in
            ShowSettingsSheet(entry: item.entry)
        }
        .moreAboutSheet(navigator)
        .sourcePickerSheet(navigator)
        .matchPickerSheet(navigator)
        .openLinkSheet(navigator)
        .surprisePicks()
    }

    /// The toolbar's title: the section's, a series' over it, none under a show's page.
    private var macTitle: String {
        guard navigator.titleOnMac == nil else { return "" }
        if let work = navigator.seriesOnMac { return store.reading.series[work]?.titles.display ?? "" }
        if navigator.section == .providers, let page = navigator.bookSourcePage { return page.title(store) }
        return navigator.section == .downloads ? "" : navigator.section.title
    }

    /// What the toolbar says under the section's name.
    private var subtitle: String {
        guard navigator.titleOnMac == nil, navigator.seriesOnMac == nil else { return "" }
        switch navigator.section {
        case .home:
            // Offline is said once, in the sync capsule (`AppStore.homeOfflineLine`).
            return ""
        case .calendar:
            return CalendarWords.range(weekDays(offset: navigator.calendarWeek))
        case .seasonal:
            return store.seasonName ?? ""
        case .search:
            // The search as a job past 600 ms, then what is asked (Running jobs, native-ux.md).
            return store.search.searchingLine ?? (store.search.atStart ? "" : store.search.query.headline())
        case .downloads:
            return downloadsSubtitle(store)
        default:
            return ""
        }
    }
}

/// The toolbar items a section adds beside the sync capsule and search.
struct SectionToolbar: ToolbarContent {
    let section: AppSection

    var body: some ToolbarContent {
        switch section {
        case .calendar:
            ToolbarItem(placement: .primaryAction) { CalendarToolbar() }
        case .library:
            ToolbarItem(placement: .primaryAction) { LibraryToolbar() }
        case .updates:
            ToolbarItem(placement: .primaryAction) { UpdatesDownloadNew() }
        case .search:
            ToolbarItem(placement: .primaryAction) { SurpriseButton() }
        case .seasonal:
            ToolbarItem(placement: .primaryAction) { SeasonalToolbar() }
        case .providers:
            ToolbarItem(placement: .primaryAction) { SourcesToolbar() }
        case .pluginStores:
            ToolbarItem(placement: .primaryAction) { StoresToolbar() }
        case .downloads:
            ToolbarItem(placement: .primaryAction) { DownloadsToolbar() }
        default:
            ToolbarItem(placement: .primaryAction) { EmptyView() }
        }
    }
}

/// Library's toolbar: the shows' segments and menus, or none for the reading library, whose
/// lists are in the content.
struct LibraryToolbar: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        if !ReadLibraryScope.reading(store: store, navigator: navigator) {
            HStack(spacing: 8) {
                LibrarySegments()
                SurpriseButton()
                LibraryMenus()
            }
        }
    }
}

/// Your sources' toolbar: Add a source with reading on, beside the stores' check with shows.
struct SourcesToolbar: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        HStack(spacing: 8) {
            if store.medium.answer != .read { ProvidersToolbar() }
            if store.medium.answer != .watch { BookSourcesToolbar() }
        }
    }
}

/// Sources' toolbar: check the stores for plugin updates.
struct ProvidersToolbar: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        HStack(spacing: 8) {
            Button(L.settingsCheckUpdates) { store.loadStores() }
        }
    }
}

/// Plugin stores' toolbar: join another.
struct StoresToolbar: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        Button {
            navigator.invite = ""
        } label: {
            Label(L.storesJoinMenu, systemImage: "plus").labelStyle(.titleAndIcon)
        }
    }
}

/// Downloads' toolbar: pause or resume everything, and the folder.
struct DownloadsToolbar: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let running = store.downloads.rows.contains { $0.state == .active || $0.state == .queued }
        let paused = store.downloads.rows.contains { $0.state == .paused }
        HStack(spacing: 8) {
            // Episodes only: chapters stop one series at a time, from their ring.
            if !store.medium.watches || store.downloads.rows.isEmpty {
                EmptyView()
            } else if running || !paused {
                Button(L.downloadsPauseAll) { store.pauseAllDownloads() }
                    .disabled(!running)
            } else {
                Button(L.downloadsResumeAll) { store.resumeAllDownloads() }
            }
            Button {
                let path = store.settings.row(.downloadDir)?.typedText ?? ""
                if !path.isEmpty { NSWorkspace.shared.open(URL(fileURLWithPath: path)) }
            } label: {
                Label(L.shellShowInFinder, systemImage: "folder").labelStyle(.titleAndIcon)
            }
            .disabled((store.settings.row(.downloadDir)?.typedText ?? "").isEmpty)
        }
    }
}

/// The Library's lists, with how many titles each holds.
struct LibrarySegments: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        @Bindable var navigator = navigator
        if store.accounts.summary.state != .signedOut {
            Picker(L.shellLibraryListLabel, selection: $navigator.librarySegment) {
                ForEach(LibraryModel.order, id: \.self) { segment in
                    let count = store.library.segment(segment).state.ids.count
                    Text(L.shellLibrarySegment(name: segment.title, count: count)).tag(segment)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
        }
    }
}

/// The words for a sort, from the catalogue.
private extension LibrarySort {
    var shown: String {
        switch self {
        case .lastWatched: L.librarySortUpdated
        case .title: L.librarySortTitle
        case .score: L.librarySortScore
        }
    }
}

/// The Library's order, and More: Sync Now.
struct LibraryMenus: View {
    @Environment(AppStore.self) private var store
    @AppStorage("kaeri.librarySort") private var sort: LibrarySort = .lastWatched

    var body: some View {
        if store.accounts.summary.state != .signedOut {
            HStack(spacing: 8) {
                Menu {
                    Picker(L.shellLibraryOrderLabel, selection: $sort) {
                        ForEach(LibrarySort.allCases) { Text($0.shown).tag($0) }
                    }
                    .pickerStyle(.inline)
                } label: {
                    Text(sort.shown)
                }
                .fixedSize()
                Menu {
                    Button(L.accountsSyncNow) { store.send(.syncNow) }
                } label: {
                    Label(L.actionMore, systemImage: "ellipsis")
                }
            }
        }
    }
}

/// The sidebar: the sections, then Sources, then Settings alone at the foot. Its rows draw
/// their own selection, silk on the raised selected fill as the reading board decided (amber
/// stays on the one main action), which a system list would draw grey whenever the window is
/// not key.
struct Sidebar: View {
    @Binding var selection: AppSection
    @FocusState private var focused: Bool
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            // A 12 pt gap between the groups, with no headings: Watch and Read name them.
            ForEach(Array(AppSection.groups(store.medium.answer).enumerated()), id: \.offset) { index, group in
                if index > 0 { Color.clear.frame(height: 12) }
                ForEach(group) { row($0) }
            }
            Text(L.sourcesTitle)
                .font(.mac(11, weight: .semibold))
                .foregroundStyle(.haze)
                .padding(EdgeInsets(top: 18, leading: 10, bottom: 6, trailing: 10))
                .accessibilityAddTraits(.isHeader)
            ForEach(AppSection.sources) { row($0) }
            Spacer(minLength: 0)
            if store.sync.hasProfiles, let profile = store.sync.activeProfile { account(profile) }
            row(.settings)
        }
        .padding(EdgeInsets(top: 0, leading: 10, bottom: 12, trailing: 10))
        .focusable()
        .focused($focused)
        .listFocusEffect()
        .onKeyPress(.upArrow) { step(-1) }
        .onKeyPress(.downArrow) { step(1) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(L.navSectionsLabel)
    }

    /// The profile in use, with the menu that switches it: shown only with two or more.
    private func account(_ profile: ProfileView) -> some View {
        Menu {
            ForEach(store.sync.profiles, id: \.id) { other in
                Button {
                    if !other.active { store.sync.switchProfile(other.id) }
                } label: {
                    if other.active { Label(other.name, systemImage: "checkmark") } else { Text(other.name) }
                }
            }
            Divider()
            Button(L.profilesAdd + "…") {
                navigator.openSettings(.profiles)
                store.sync.sheet = .profileAdd
            }
            Button(L.profilesManage) { navigator.openSettings(.profiles) }
        } label: {
            HStack(spacing: 10) {
                ProfileAvatar(profile: profile, size: 22)
                Text(profile.name)
                    .font(.mac(13))
                    .foregroundStyle(.silk)
                Spacer(minLength: 0)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.mac(10))
                    .foregroundStyle(.haze)
            }
            .padding(.horizontal, 10)
            .frame(height: 30)
            .contentShape(.rect(cornerRadius: 8))
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .accessibilityLabel(L.profilesAvatarDesc(name: profile.name))
    }

    private func row(_ section: AppSection) -> some View {
        let selected = selection == section
        return Button {
            // M1: a section changes at once, with no animation.
            var instant = Transaction()
            instant.disablesAnimations = true
            withTransaction(instant) { selection = section }
        } label: {
            HStack(spacing: 10) {
                Image(systemName: section.symbol)
                    .font(.mac(14))
                    .foregroundStyle(selected ? .silk : .haze)
                    .frame(width: 16)
                Text(section.title)
                    .font(.mac(13, weight: selected ? .medium : .regular))
                    .foregroundStyle(.silk)
                Spacer(minLength: 0)
                // Unread chapters beside Updates, in haze: a count, not a badge.
                if section == .updates, store.reading.unread > 0 {
                    Text(store.reading.unread, format: .number)
                        .font(.mac(12))
                        .monospacedDigit()
                        .foregroundStyle(.haze)
                }
            }
            .padding(.horizontal, 10)
            .frame(height: 30)
            .background(selected ? Palette.raisedSelected : .clear, in: .rect(cornerRadius: 8))
            .contentShape(.rect(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        // Nothing moves on a section change (M1), the tint included.
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    private func step(_ delta: Int) -> KeyPress.Result {
        let order = AppSection.groups(store.medium.answer).flatMap { $0 } + AppSection.sources + [.settings]
        let index = order.firstIndex(of: selection) ?? 0
        selection = order[min(max(index + delta, 0), order.count - 1)]
        return .handled
    }
}

/// The section on screen.
struct SectionView: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let section: AppSection

    var body: some View {
        switch section {
        case .home: HomeScreen()
        case .read: ReadHomeScreen()
        case .updates: UpdatesScreen()
        case .calendar: CalendarScreen()
        case .seasonal: SeasonalScreen()
        case .search:
            if ReadSearchScope.active(store: store, navigator: navigator) { ReadSearchScreen() } else { SearchScreen() }
        case .library: LibraryScreen()
        case .settings: SettingsScreen()
        case .downloads: DownloadsScreen()
        case .together: TogetherScreen()
        case .providers: SourcesScreen()
        case .pluginStores: PluginStoresScreen()
        }
    }
}
#endif
