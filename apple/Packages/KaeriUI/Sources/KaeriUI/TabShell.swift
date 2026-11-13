// iPhone and iPad: the tabs, Search on its own, a stack per tab with a title's page pushed onto it
// by a zoom from its cover (T1).

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct TabShell: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        // The sidebar on iPad, which turns into the bar in a narrow window; on iPhone the system's
        // own tab bar, with Search apart. On iPad the style goes by the device, never the width:
        // one tab view through every resize, so the pages in it keep what they hold (picks, a
        // scroll). No customization: the bar's set is fixed, and the rest live in the sidebar
        // only, so the sidebar has no Edit button.
        if Self.pad || sizeClass == .regular {
            tabs.tabViewStyle(.sidebarAdaptable)
        } else {
            tabs
        }
    }

    private var tabs: some View {
        @Bindable var navigator = navigator
        // The Calendar tab tapped again on its own screen goes back to today; with a page on
        // top, the system's pop to the screen comes first.
        let section = Binding(
            get: { navigator.section },
            set: { picked in
                if picked == .calendar, navigator.section == .calendar, navigator.paths[.calendar]?.isEmpty ?? true {
                    navigator.calendarToToday += 1
                }
                navigator.section = picked
            }
        )
        return TabView(selection: section) {
            // The places for what Kaeri is for, in the sidebar's order. On iPhone and in a narrow
            // window the bar keeps three or four of them; the rest are pushed from Home and Read.
            // Home and Read are named for the answer (Watch, Read, or Home alone), so their
            // identity carries it and the sidebar draws the new name when the answer changes.
            ForEach((sizeClass == .regular ? Self.places(store.medium.answer) : Self.barTabs(store.medium.answer))
                .map { NamedPlace(section: $0, answer: store.medium.answer) }) { place in
                Tab(place.section.title, systemImage: place.section.symbol, value: place.section) {
                    TabStack(section: place.section, place: { AnyView(screen($0)) }) { screen(place.section) }
                }
                // On iPad the bar keeps the iPhone's set, so it never runs past its width; the
                // sidebar has every place.
                .tabPlacement(Self.barTabs(store.medium.answer).contains(place.section) ? .automatic : .sidebarOnly)
            }
            if sizeClass == .regular {
                // In the sidebar's main group, as the Mac has it; Sources holds what plays from where.
                Tab(AppSection.downloads.title, systemImage: AppSection.downloads.symbol, value: AppSection.downloads) {
                    TabStack(section: .downloads) { screen(.downloads) }
                }
                .tabPlacement(.sidebarOnly)
                if store.medium.answer != .read {
                    Tab(AppSection.together.title, systemImage: AppSection.together.symbol, value: AppSection.together) {
                        TabStack(section: .together) { screen(.together) }
                    }
                    .tabPlacement(.sidebarOnly)
                }
                TabSection(L.sourcesTitle) {
                    ForEach([AppSection.providers, .pluginStores]) { section in
                        Tab(section.title, systemImage: section.symbol, value: section) {
                            TabStack(section: section) { screen(section) }
                        }
                        .tabPlacement(.sidebarOnly)
                    }
                }
                .tabPlacement(.sidebarOnly)
                Tab(AppSection.settings.title, systemImage: AppSection.settings.symbol, value: AppSection.settings) {
                    TabStack(section: .settings) { SettingsScreen() }
                }
                .tabPlacement(.sidebarOnly)
            }
            Tab(value: AppSection.search, role: .search) {
                TabStack(section: .search) { PhoneSearch() }
                    .titleSearchField(
                        text: $navigator.query,
                        titles: !ReadSearchScope.active(store: store, navigator: navigator),
                        // The iPhone's field holds only what was typed; the pinned row names the
                        // genres and tags.
                        tokensShown: sizeClass != .compact,
                        // On the iPhone the field always shows under the large title, with the
                        // pinned filter row beneath it, instead of waiting for a pull.
                        placement: sizeClass == .compact ? .navigationBarDrawer(displayMode: .always) : .automatic,
                        prompt: ReadSearchScope.active(store: store, navigator: navigator) ? L.readSearchPlaceholder : L.searchPlaceholder)
                    .modifier(ReadSearchTyping())
            }
        }
        .tabBarMinimizeBehavior(.onScrollDown)
        // A place with no tab here goes onto the tab that leads to it, as the headings on each
        // Home do.
        .onChange(of: navigator.section, initial: true) { _, _ in narrowed() }
        // A window made narrow keeps the place the person was on, pushed onto its tab; made wide
        // again, that place is its own section again.
        .onChange(of: sizeClass) { _, now in
            if now == .compact { narrowed() } else { widened() }
        }
        // Watch together has no tab on iPhone and in a narrow window: Home's people button
        // pushes it there.
        .onChange(of: navigator.section == .together && sizeClass == .compact, initial: true) { _, narrow in
            guard narrow else { return }
            navigator.section = .home
            store.party.placeShown = true
        }
        .modifier(TVAccessory())
        // The iPad's sidebar as the brief draws it (PadSidebarStyle). The bar is empty; it is
        // where the style reaches the sidebar from, and unlike a header it moves no row.
        .tabViewSidebarBottomBar { PadSidebarStyle(selection: navigator.section) }
        // At the foot, above the tab bar on iPhone and in a narrow window (Notices).
        .overlay(alignment: .bottom) {
            NoticeCapsule(here: navigator.phoneSettings == nil)
                .padding(.horizontal, 16)
                .padding(.bottom, sizeClass == .compact ? 64 : 16)
        }
        .sheet(isPresented: $navigator.messagesShown) {
            MessagesView()
        }
        .sheet(item: $navigator.phoneSettings) { page in
            PhoneSettingsSheet(start: page)
        }
        .sheet(item: Binding<ShowItem?>(
            get: { navigator.showSettings.flatMap { store.titles[$0] }.map(ShowItem.init) },
            set: { (item: ShowItem?) in navigator.showSettings = item?.entry.id }
        )) { item in
            PhoneShowSettingsSheet(entry: item.entry)
                .presentationDetents([.medium, .large])
        }
        .moreAboutSheet(navigator)
        .sourcePickerSheet(navigator)
        .matchPickerSheet(navigator)
        .openLinkSheet(navigator)
        .surprisePicks()
        .modifier(AppSheets())
    }

    /// The sections with a tab, in the sidebar's order: Search has its own, and Downloads and
    /// Watch together come after.
    static func places(_ answer: KaeriIsFor) -> [AppSection] {
        AppSection.groups(answer).flatMap { $0 }.filter { ![.search, .downloads, .together].contains($0) }
    }

    /// A place with no tab of its own in a narrow window goes onto the tab that leads to it.
    private func narrowed() {
        guard sizeClass == .compact, let to = NarrowTabs.narrow(navigator.section, store.medium.answer) else { return }
        switch to.pushed {
        case .downloads: navigator.downloadsPushed = true
        case .place(let place): navigator.placePushed = place
        }
        navigator.section = to.tab
    }

    private static var pad: Bool { UIDevice.current.userInterfaceIdiom == .pad }

    /// The place pushed for a narrow window is its own section again in a wide iPad window.
    private func widened() {
        guard Self.pad, let to = NarrowTabs.widen(navigator.section, downloads: navigator.downloadsPushed,
                                  place: navigator.placePushed, store.medium.answer) else { return }
        navigator.downloadsPushed = false
        navigator.placePushed = nil
        navigator.section = to
    }

    /// A place as a tab: Home's and Read's identity carries the answer they are named for.
    struct NamedPlace: Identifiable {
        let section: AppSection
        let answer: KaeriIsFor
        var id: String { [.home, .read].contains(section) ? "\(section.rawValue).\(answer)" : section.rawValue }
    }

    static func barTabs(_ answer: KaeriIsFor) -> [AppSection] { NarrowTabs.barTabs(answer) }

    @ViewBuilder
    private func screen(_ section: AppSection) -> some View {
        let wide = sizeClass == .regular
        switch section {
        case .home: HomeScreen()
        case .read: ReadHomeScreen()
        case .updates: UpdatesScreen()
        case .calendar:
            if wide {
                // The week's paging and My list or All airing, as on the Mac (PadCalendar).
                CalendarScreen().toolbar { ToolbarItem(placement: .topBarTrailing) { CalendarToolbar() } }
            } else {
                PhoneCalendar()
            }
        case .seasonal: SeasonalScreen(minimum: wide ? 128 : 100).toolbar { ToolbarItem { SeasonalToolbar() } }
        case .library: PhoneLibrary()
        case .downloads: DownloadsScreen().toolbar { IPadDownloadsToolbar() }
        case .providers: SourcesScreen().toolbar { IPadProvidersToolbar() }
        case .pluginStores: PluginStoresScreen().toolbar { IPadStoresToolbar() }
        case .settings: SettingsScreen()
        case .together: TogetherScreen()
        case .search: EmptyView()
        }
    }
}

/// A title page's toolbar on iPad: the list it is on, and More.
struct PadTitleToolbar: ToolbarContent {
    @Environment(AppStore.self) private var store
    let id: AnilistId

    var body: some ToolbarContent {
        // The list's menu is in the page's header, beside Resume (PadTitleMeta): a narrow
        // window's bar, with the tabs in it, folded it away.
        ToolbarItem(placement: .topBarTrailing) {
            AnotherButton(id: id)
        }
        ToolbarItem(placement: .topBarTrailing) {
            Menu {
                if let entry = store.titles[id] { TitleMoreItems(entry: entry) }
            } label: {
                Label(L.actionMore, systemImage: "ellipsis")
            }
        }
    }
}

/// Sources' toolbar on iPad: check the stores for plugin updates.
struct IPadProvidersToolbar: ToolbarContent {
    @Environment(AppStore.self) private var store

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Button(L.settingsCheckUpdates) { store.loadStores() }
        }
    }
}

/// Plugin stores' toolbar on iPad: join another.
struct IPadStoresToolbar: ToolbarContent {
    @Environment(Navigator.self) private var navigator

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Button(L.storesJoinMenu, systemImage: "plus") { navigator.invite = "" }
        }
    }
}

/// Downloads' toolbar on iPad: pause or resume everything.
struct IPadDownloadsToolbar: ToolbarContent {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some ToolbarContent {
        // Episodes only: chapters stop one series at a time, from their ring. With no episode
        // in the list, Pause all would only ever sit there disabled.
        if store.medium.watches {
            ToolbarItem(placement: .topBarTrailing) {
                Button(L.openedOpen, systemImage: "plus") { navigator.openLinkShown = true }
            }
        }
        ToolbarItem(placement: .topBarTrailing) {
            let paused = store.downloads.rows.contains { $0.state == .paused }
            let running = store.downloads.rows.contains { $0.state == .active || $0.state == .queued }
            if !store.medium.watches || store.downloads.rows.isEmpty {
                EmptyView()
            } else if running || !paused {
                Button(L.downloadsPauseAll) { store.pauseAllDownloads() }.disabled(!running)
            } else {
                Button(L.downloadsResumeAll) { store.resumeAllDownloads() }
            }
        }
    }
}

/// A tab's stack: its screen, with a title's page pushed onto it by a zoom from the element that
/// opened it (TitleOpening).
struct TabStack<Screen: View>: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.dynamicTypeSize) private var typeSize
    @Namespace private var zoom
    let section: AppSection
    /// The screen of a place pushed onto this tab (Navigator.placePushed), where it can be.
    var place: ((AppSection) -> AnyView)?
    @ViewBuilder let screen: () -> Screen

    var body: some View {
        NavigationStack(path: Binding(
            get: { navigator.paths[section] ?? [] },
            set: {
                // Back, however it came, ends a pick's visit and what Another put in its place.
                if $0.count < (navigator.paths[section] ?? []).count {
                    navigator.pickVisit = nil
                    navigator.shownInPlace = [:]
                }
                navigator.paths[section] = $0
            }
        )) {
            screen()
                .toolbar {
                    if store.tv.connected != nil {
                        ToolbarItem(placement: .topBarLeading) { TVNowPlayingButton() }
                    }
                }
                .navigationTitle(section.title)
                .boardTitle(section.title)
                // Library's Downloads row, on iPhone and in a narrow window: no tab of its own.
                .navigationDestination(isPresented: section == .library ? Binding(
                    get: { navigator.downloadsPushed },
                    set: { navigator.downloadsPushed = $0 }
                ) : .constant(false)) {
                    PhoneDownloadsPage()
                }
                .navigationDestination(isPresented: Binding(
                    get: { place != nil && navigator.section == section && navigator.placePushed != nil },
                    set: { if !$0 { navigator.placePushed = nil } }
                )) {
                    if let pushed = navigator.placePushed, let place {
                        place(pushed)
                            .navigationTitle(pushed.title)
                            .background(.ground)
                    }
                }
                .background(.ground)
                // The ground under the whole stack, edge to edge, however narrow the screen in
                // it: never the window's black at its sides.
                .containerBackground(.ground, for: .navigation)
                .navigationDestination(for: TitleRoute.self) { route in
                    if let work = route.work {
                        SeriesPage(work: work)
                    } else {
                        title(route)
                    }
                }
        }
        .environment(\.zoomNamespace, zoom)
    }

    @ViewBuilder
    private func title(_ route: TitleRoute) -> some View {
        let id = navigator.shownInPlace[route.id] ?? route.id
        // A wide iPad has the Mac's page (PadTitleMeta); an iPhone, a narrow window
        // and the accessibility text sizes, the iPhone's, whose one column wraps
        // what the Mac's side-by-side header would cut.
        // Another swaps the title in place: a crossfade with both sides drawn.
        ZStack {
            Group {
                if sizeClass == .regular, !typeSize.isAccessibilitySize {
                    TitlePage(id: id).toolbar { PadTitleToolbar(id: id) }
                } else {
                    PhoneTitle(id: id)
                }
            }
            .id(id)
            .transition(.opacity)
        }
        .animation(.standard, value: id)
        // Out of the element clicked, and back into it.
        .navigationTransition(.zoom(sourceID: route.zoomID, in: zoom))
    }
}

/// The iPad's sidebar as the brief draws it: the brief's navy glass, and its rows with the
/// selection silk on the raised selected fill (amber stays on the one main action), every other
/// row silk with a haze icon, and the Sources header in haze.
/// UIKit draws the sidebar itself, grey glass with a solid capsule in the
/// tint under white text, and gives no style for either; this view, in the sidebar's bottom bar,
/// reaches both from there. It draws nothing, and takes no room.
///
/// - The glass: it finds the glass around it and tints it, whenever the sidebar is put on screen
///   or laid out again.
/// - The rows: it styles each row as UIKit asks the sidebar's delegate for it (`SidebarRows`,
///   which hands everything else on to SwiftUI's own delegate), and has them asked again when the
///   section changes, since UIKit moves the selection without asking.
/// - A pressed row: UIKit tells the delegate nothing of a press, so `RowPress` watches the list's
///   touches and lays a faint layer under the row while the finger is down.
struct PadSidebarStyle: UIViewRepresentable {
    let selection: AppSection

    final class Hook: UIView {
        private static let navy = UIColor(Palette.glassOverGround)

        override func didMoveToWindow() {
            super.didMoveToWindow()
            apply()
        }

        override func layoutSubviews() {
            super.layoutSubviews()
            apply()
        }

        private func apply() {
            guard window != nil else { return }
            if let glass {
                tint(glass)
                if let list = list(in: glass.contentView) { RowPress.watch(list) }
            }
            guard let controller = tabBarController else { return }
            if SidebarRows.install(on: controller) { restyle() }
        }

        /// The glass the sidebar sits in.
        private var glass: UIVisualEffectView? {
            var view = superview
            while let current = view {
                if let glass = current as? UIVisualEffectView, glass.effect is UIGlassEffect { return glass }
                view = current.superview
            }
            return nil
        }

        private func tint(_ glass: UIVisualEffectView) {
            guard let effect = glass.effect as? UIGlassEffect, effect.tintColor != Self.navy,
                  let tinted = effect.copy() as? UIGlassEffect else { return }
            tinted.tintColor = Self.navy
            glass.effect = tinted
        }

        /// The sidebar's list of rows.
        private func list(in view: UIView) -> UICollectionView? {
            if let list = view as? UICollectionView { return list }
            for subview in view.subviews {
                if let list = list(in: subview) { return list }
            }
            return nil
        }

        private var tabBarController: UITabBarController? {
            var responder: UIResponder? = self
            while let current = responder {
                if let controller = current as? UITabBarController { return controller }
                responder = current.next
            }
            return nil
        }

        /// Has UIKit ask for every row again, so each is styled as it now stands.
        func restyle() {
            guard let controller = tabBarController else { return }
            func all(_ tabs: [UITab]) -> [UITab] {
                tabs.flatMap { [$0] + (($0 as? UITabGroup).map { all($0.children) } ?? []) }
            }
            for tab in all(controller.tabs) { controller.sidebar.reconfigureItem(for: tab) }
        }
    }

    func makeUIView(context: Context) -> Hook { Hook() }

    func updateUIView(_ uiView: Hook, context: Context) {
        // After this update, once the tab bar controller has the section selected. When the
        // section changes twice in one turn, as Open at does once the medium is known, UIKit
        // can select the tab after the first pass, so a second pass follows.
        Task { @MainActor in
            uiView.restyle()
            try? await Task.sleep(for: .milliseconds(250))
            uiView.restyle()
        }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: Hook, context: Context) -> CGSize? {
        .zero
    }
}

/// The sidebar's delegate: styles each row UIKit asks for, and hands every other call on to the
/// delegate it replaced, SwiftUI's own.
@MainActor
final class SidebarRows: NSObject, UITabBarController.Sidebar.Delegate {
    private static var key: UInt8 = 0
    /// SwiftUI's delegate, which SwiftUI keeps.
    nonisolated(unsafe) private weak var original: (any UITabBarController.Sidebar.Delegate)?

    /// Puts the styling delegate on `controller`'s sidebar, kept with the controller, in front of
    /// whatever delegate is there. True when it was not there already.
    static func install(on controller: UITabBarController) -> Bool {
        let rows: SidebarRows
        if let kept = objc_getAssociatedObject(controller, &key) as? SidebarRows {
            rows = kept
        } else {
            rows = SidebarRows()
            objc_setAssociatedObject(controller, &key, rows, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        }
        let sidebar = controller.sidebar
        guard sidebar.delegate !== rows else { return false }
        rows.original = sidebar.delegate
        sidebar.delegate = rows
        return true
    }

    override nonisolated func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (original?.responds(to: selector) ?? false)
    }

    override nonisolated func forwardingTarget(for selector: Selector!) -> Any? {
        if let original, original.responds(to: selector) { return original }
        return super.forwardingTarget(for: selector)
    }

    func tabBarController(
        _ tabBarController: UITabBarController, sidebar: UITabBarController.Sidebar,
        itemFor request: UITabSidebarItem.Request
    ) -> UITabSidebarItem {
        let item = original?.tabBarController?(tabBarController, sidebar: sidebar, itemFor: request)
            ?? UITabSidebarItem(request: request)
        style(item, in: tabBarController)
        return item
    }

    func tabBarController(
        _ tabBarController: UITabBarController, sidebar: UITabBarController.Sidebar,
        update item: UITabSidebarItem
    ) {
        original?.tabBarController?(tabBarController, sidebar: sidebar, update: item)
        style(item, in: tabBarController)
    }

    private func style(_ item: UITabSidebarItem, in controller: UITabBarController) {
        guard case .tab(let tab) = item.content,
              var content = item.contentConfiguration as? UIListContentConfiguration else { return }
        content.textProperties.colorTransformer = nil
        if tab is UITabGroup {
            // A section's header, Sources.
            content.textProperties.color = UIColor(Palette.haze)
            item.contentConfiguration = content
            return
        }
        // The controller's selected tab, which is current before the row's own state is.
        let selected = controller.selectedTab.map { $0.identifier == tab.identifier }
            ?? item.configurationState.isSelected
        content.textProperties.color = UIColor(Palette.silk)
        content.imageProperties.tintColorTransformer = nil
        content.imageProperties.tintColor = UIColor(selected ? Palette.silk : Palette.haze)
        if selected {
            let font = content.textProperties.font
            let medium = font.fontDescriptor.addingAttributes([.traits: [UIFontDescriptor.TraitKey.weight: UIFont.Weight.medium]])
            content.textProperties.font = UIFont(descriptor: medium, size: 0)
        }
        item.contentConfiguration = content
        var background = item.defaultBackgroundConfiguration()
        background.backgroundColorTransformer = nil
        background.backgroundColor = selected ? UIColor(Palette.raisedSelected) : .clear
        background.cornerRadius = 12
        item.backgroundConfiguration = background
    }
}

/// A finger on one of the sidebar's rows. UIKit tells neither the sidebar's delegate nor the rows'
/// configurations that a row is pressed, and its own press was drawn on the colours `SidebarRows`
/// replaces, so this watches the list's touches, alongside the list's own gestures, and lays a
/// layer under the pressed row: a faint silk, over the selected row's fill too. It goes when the
/// finger lifts or moves away.
final class RowPress: UILongPressGestureRecognizer, UIGestureRecognizerDelegate {
    private static let underSelection = UIColor(Palette.silk.opacity(0.06))

    private var pressLayer: UIView?
    private var start: CGPoint = .zero

    /// Watches `list`'s rows, once.
    static func watch(_ list: UICollectionView) {
        guard !(list.gestureRecognizers ?? []).contains(where: { $0 is RowPress }) else { return }
        list.addGestureRecognizer(RowPress())
    }

    private init() {
        super.init(target: nil, action: nil)
        addTarget(self, action: #selector(changed))
        minimumPressDuration = 0
        cancelsTouchesInView = false
        delaysTouchesBegan = false
        delaysTouchesEnded = false
        delegate = self
    }

    @objc private func changed() {
        guard let list = view as? UICollectionView else { return }
        switch state {
        case .began:
            start = location(in: list)
            guard let path = list.indexPathForItem(at: start), let row = list.cellForItem(at: path) else { return }
            let under = UIView(frame: row.bounds)
            under.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            under.isUserInteractionEnabled = false
            under.layer.cornerRadius = 12
            under.layer.cornerCurve = .continuous
            under.backgroundColor = row.isSelected ? Self.underSelection : UIColor(Palette.rowPressed)
            row.insertSubview(under, belowSubview: row.contentView)
            pressLayer = under
        case .changed:
            let point = location(in: list)
            if hypot(point.x - start.x, point.y - start.y) > 10 { lift() }
        default:
            lift()
        }
    }

    private func lift() {
        pressLayer?.removeFromSuperview()
        pressLayer = nil
    }

    func gestureRecognizer(
        _ gestureRecognizer: UIGestureRecognizer,
        shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer
    ) -> Bool {
        true
    }
}
#endif
