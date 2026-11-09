// Reading's Search: browsing with no query, as for shows, and results as rows once words are
// typed. Four filters: format, origin, status and genres. With both media on, Shows or Reading
// comes first, and the last row counts matches in the other scope and switches to it.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

enum ReadSearchScope {
    /// Whether Search looks for reading: always with reading alone, by the control with both.
    @MainActor
    static func active(store: AppStore, navigator: Navigator) -> Bool {
        switch store.medium.answer {
        case .read: true
        case .both: navigator.searchReading
        case .watch: false
        }
    }
}

/// Shows or Reading in Search.
struct SearchScopePicker: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        Picker(L.libraryScopeLabel, selection: Binding(
            get: { navigator.searchReading },
            set: { reading in withAnimation(.quick) { navigator.searchReading = reading } }
        )) {
            Text(L.libraryScopeShows).tag(false)
            Text(L.libraryScopeReading).tag(true)
        }
        .pickerStyle(.segmented)
        .labelsHidden()
        // Its own width on the Mac, so it starts on the content's edge as the covers do.
        .fixedSize(horizontal: Device.isMac, vertical: false)
    }
}

/// Typing anywhere searches reading while Search's scope is reading; the shows' search field
/// modifier stands aside then (`titles: false`).
struct ReadSearchTyping: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    func body(content: Content) -> some View {
        content
            .task(id: ReadSearchScope.active(store: store, navigator: navigator) ? navigator.query : nil) {
                guard ReadSearchScope.active(store: store, navigator: navigator) else { return }
                let words = navigator.query
                if !words.isEmpty, navigator.section != .search { navigator.section = .search }
                guard words.trimmingCharacters(in: .whitespaces) != store.reading.search.query.text else { return }
                // Asked as the typing pauses, not at every key.
                try? await Task.sleep(for: .milliseconds(250))
                guard !Task.isCancelled else { return }
                var query = store.reading.search.query
                query.text = words.trimmingCharacters(in: .whitespaces)
                store.reading.runSearch(query)
            }
    }
}

struct ReadSearchScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var filtersShown = false
    @State private var width: CGFloat = 1_000
    @State private var position = ScrollPosition()
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    private var compact: Bool { sizeClass == .compact }
    #else
    private let compact = false
    #endif

    private var layout: HomeLayout {
        #if os(iOS)
        HomeLayout(surface: .touch(regular: !compact, column: width - 48))
        #else
        HomeLayout(surface: .mac)
        #endif
    }

    var body: some View {
        let search = store.reading.search
        let browsing = search.query.text.isEmpty
        ScrollView {
            VStack(alignment: .leading, spacing: compact ? 12 : 16) {
                if compact {
                    if store.medium.both { SearchScopePicker() }
                    ScrollView(.horizontal) {
                        HStack(spacing: 8) { ReadFilterMenus(compact: true) { filtersShown = true } }
                            .padding(.horizontal, 16)
                    }
                    .scrollIndicators(.never)
                    .padding(.horizontal, -16)
                } else {
                    HStack(spacing: 12) {
                        if store.medium.both { SearchScopePicker() }
                        ReadFilterMenus(compact: false) { filtersShown = true }
                        // Last, after a rule, so nothing moves when it comes with the first word.
                        if search.hasWords {
                            Rectangle().fill(Palette.glassRule).frame(width: 1, height: 16).accessibilityHidden(true)
                            ReadSourcesOnlyToggle()
                        }
                        Spacer(minLength: 8)
                        if browsing { ReadSortMenu() }
                    }
                    .animation(.fadeIn, value: search.hasWords)
                }
                ReadSearchBody(browsing: browsing)
                    .animation(.standard, value: search.query)
            }
            .padding(EdgeInsets(top: 8, leading: 16, bottom: 24, trailing: 16))
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .scrollIndicators(.never)
        .scrollPosition($position)
        .background(.ground)
        .environment(\.homeLayout, layout)
        .onOfferedWidth { width = $0 }
        .task {
            if let genre = navigator.readingGenre {
                navigator.readingGenre = nil
                var query = ReadingModel.emptyQuery
                query.genres = [genre]
                store.reading.runSearch(query)
            } else if search.tiles.isEmpty, !search.loading {
                store.reading.runSearch(search.query)
            }
        }
        // Shots: the sources' part in the board's states, `sources-<state>`.
        .task {
            guard let sheet = navigator.launchSheet, sheet.hasPrefix("sources-") else { return } // i18n-skip
            try? await Task.sleep(for: .milliseconds(500))
            navigator.launchSheet = nil
            store.reading.stageSourceSearchShot(String(sheet.dropFirst("sources-".count))) // i18n-skip
            // A shot's `scroll=<points>`: the sources' rows brought up once they are drawn.
            guard navigator.section == .search, let points = navigator.launchReadScroll else { return }
            navigator.launchReadScroll = nil
            try? await Task.sleep(for: .milliseconds(1_000))
            position.scrollTo(y: CGFloat(points))
        }
        #if os(iOS)
        // Shots: `sheet=filters` opens the Filters sheet.
        .task {
            guard navigator.launchSheet == "filters" else { return } // i18n-skip: a shot's sheet name
            try? await Task.sleep(for: .milliseconds(500))
            navigator.launchSheet = nil
            filtersShown = true
        }
        .toolbar {
            if compact {
                ToolbarItem(placement: .topBarTrailing) {
                    ReadFilterButton { filtersShown = true }
                }
            }
        }
        .sheet(isPresented: $filtersShown) { ReadFiltersSheet() }
        #endif
    }
}

/// Your sources only, the filter bar's last chip: a switch to a screen reader, and on, a check.
/// Announced once as it turns on.
private struct ReadSourcesOnlyToggle: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let on = store.reading.search.query.sourcesOnly
        Toggle(L.readSearchSourcesOnly, isOn: Binding(
            get: { on },
            set: { value in
                var query = store.reading.search.query
                query.sourcesOnly = value
                store.reading.runSearch(query)
                if value { AccessibilityNotification.Announcement(L.readSearchSourcesOnlyLine).post() }
            }
        ))
        .toggleStyle(ReadChipToggleStyle())
        .pointerHover()
    }
}

/// A chip that is on or off: an outline either way, the check and silk words while on.
private struct ReadChipToggleStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        Button {
            configuration.isOn.toggle()
        } label: {
            HStack(spacing: 6) {
                if configuration.isOn {
                    Image(systemName: "checkmark")
                        .font(.ui(Device.isMac ? 10 : 12, weight: .bold))
                        .accessibilityHidden(true)
                        .transition(.opacity)
                }
                configuration.label
                    .fontWeight(configuration.isOn ? .semibold : .medium)
            }
            .font(.plat(mac: 13, touch: 15, weight: .medium))
            .foregroundStyle(configuration.isOn ? .silk : .haze)
            .padding(.horizontal, 12)
            .frame(height: Device.isMac ? 28 : 36)
            // On, it takes the neutral fill as a set filter does; amber stays with the actions.
            .background(configuration.isOn ? Palette.raisedSelected : .clear, in: .capsule)
            .overlay { Capsule().strokeBorder(configuration.isOn ? .clear : Palette.glassRule, lineWidth: 1) }
            .contentShape(.capsule)
            .animation(.quick, value: configuration.isOn)
        }
        .buttonStyle(.plain)
        .fixedSize()
        .accessibilityRepresentation {
            Toggle(isOn: configuration.$isOn) { configuration.label }
        }
    }
}

private struct ReadSearchBody: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let browsing: Bool

    var body: some View {
        let search = store.reading.search
        if !browsing {
            ReadWordsResults()
        } else if let failure = search.failure, search.tiles.isEmpty {
            ReadSearchFailed(failure: failure)
        } else if search.tiles.isEmpty, search.loading || search.page == 0 && search.hasMore {
            HomeWait(active: true, activity: "searching reading") { // i18n-skip
                ReadSearchPlaceholder()
            }
        } else if search.tiles.isEmpty {
            VStack(alignment: .leading, spacing: 0) {
                ReadNothingFound(title: L.readSearchNothingBrowse)
                OtherScopeRow()
            }
        } else {
            VStack(alignment: .leading, spacing: 12) {
                let viewAll = navigator.readViewAllLine.flatMap { $0.query == search.query ? $0.line : nil }
                Text(ReadFilterWords.browseLine(search.query, filters: store.reading.filters, total: search.total, line: viewAll))
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(.haze)
                ReadGrid(tiles: search.tiles, twoLines: true, caption: .formatOrigin)
                ReadSearchMore()
            }
        }
    }
}

/// Loads AniList's next page as the end of its results comes into view.
private struct ReadSearchMore: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let search = store.reading.search
        if search.hasMore {
            HStack {
                if search.loading { ProgressView().controlSize(.small) }
            }
            .frame(maxWidth: .infinity, minHeight: 32)
            .onAppear { store.reading.searchMore() }
        }
    }
}

/// Couldn't search, with Try again.
private struct ReadSearchFailed: View {
    @Environment(AppStore.self) private var store
    let failure: ReadingFailure

    var body: some View {
        PlainEmpty(title: L.readSearchFailed, line: failure.words) {
            Button(L.actionTryAgain) { store.reading.runSearch(store.reading.search.query) }
                .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                .fixedSize()
        }
    }
}

/// Nothing found: the title, the line to try other words or fewer filters, and with filters set
/// Clear filters. With words and sources, the line that they found nothing either follows.
private struct ReadNothingFound: View {
    @Environment(AppStore.self) private var store
    let title: String
    var sourcesToo = false

    var body: some View {
        let query = store.reading.search.query
        let filtered = ReadFilterWords.any(query)
        PlainEmpty(title: title, line: filtered ? L.readSearchNothingFiltersLine : L.readSearchNothingLine) {
            VStack(alignment: .leading, spacing: 16) {
                if sourcesToo {
                    Text(L.readSearchSourcesNothing)
                        .font(.plat(mac: 13, touch: 15))
                        .foregroundStyle(.haze)
                        .padding(.top, -6)
                }
                if filtered {
                    Button(L.readSearchClearFilters) {
                        var cleared = ReadingModel.emptyQuery
                        cleared.text = query.text
                        cleared.sourcesOnly = query.sourcesOnly
                        store.reading.runSearch(cleared)
                    }
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                    .fixedSize()
                }
            }
        }
    }
}

// MARK: - Results for words

/// With words: AniList's covers in the browse grid, then From your sources with one sideways row
/// per source, then the rest of AniList's results under More from AniList. Your sources only
/// leaves AniList out and the source rows lead.
private struct ReadWordsResults: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var moreShown = false

    var body: some View {
        let search = store.reading.search
        let sources = search.sources
        let groups = sources?.groups ?? []
        let only = search.query.sourcesOnly
        let anilistAsking = search.loading || sources?.anilist == .asking
            || search.tiles.isEmpty && search.page == 0 && search.hasMore
        let anilistFailed = sources?.anilist == .failed || search.failure != nil && search.tiles.isEmpty
        let anilistNothing = only || search.tiles.isEmpty && !anilistAsking && !anilistFailed
        let sourcesNothing = sources.map { $0.done && $0.groups.allSatisfy { $0.state == .nothing } } ?? true
        VStack(alignment: .leading, spacing: 0) {
            if anilistNothing, sourcesNothing, !anilistAsking {
                // Nothing anywhere: one title, the line, and the sources' line where there are some.
                ReadNothingFound(title: L.readSearchNothing(query: search.query.text), sourcesToo: !groups.isEmpty)
                    .transition(.opacity)
            } else {
                if !only {
                    anilist(search, asking: anilistAsking, failed: anilistFailed, sources: !groups.isEmpty)
                        .padding(.bottom, groups.isEmpty ? 0 : 32)
                        .transition(.opacity.animation(.fadeIn))
                }
                if !groups.isEmpty {
                    ReadSourcesPart(groups: groups)
                }
                if !only, !groups.isEmpty, search.firstPage > 0 {
                    moreFromAnilist(search)
                }
            }
            if store.demo.state.onlyBooks {
                Text(L.searchDemoOnlyBooks)
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(.haze)
                    .padding(.top, 12)
            }
            OtherScopeRow()
        }
        .animation(reduceMotion ? .fadeIn : .standard, value: only)
        .animation(.standard, value: groups.map(\.sourceId))
        .onChange(of: search.query) { moreShown = false }
        .onChange(of: sources?.done == true) { _, done in
            if done { announce(search) }
        }
    }

    @ViewBuilder
    private func anilist(_ search: ReadSearchState, asking: Bool, failed: Bool, sources: Bool) -> some View {
        if failed, search.tiles.isEmpty {
            if sources {
                PlainEmpty(title: L.readSearchAnilistFailed, line: L.readSearchAnilistFailedLine) {
                    Button(L.actionTryAgain) { store.reading.runSearch(search.query) }
                        .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                        .fixedSize()
                }
                .padding(.vertical, -24)
            } else if let failure = search.failure {
                ReadSearchFailed(failure: failure)
            }
        } else if asking, search.tiles.isEmpty {
            HomeWait(active: true, activity: "searching reading") { // i18n-skip
                ReadSearchPlaceholder()
            }
        } else if search.tiles.isEmpty {
            if sources {
                PlainEmpty(title: L.readSearchAnilistNothing, line: L.readSearchNothingLine)
                    .padding(.vertical, -24)
            } else {
                ReadNothingFound(title: L.readSearchNothing(query: search.query.text))
            }
        } else {
            VStack(alignment: .leading, spacing: 12) {
                // AniList's count only once it is exact; until then the line has no number.
                Text(search.total.map { L.readSearchTotal(n: Int($0)) } ?? L.readSearchTotalUnknown)
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(.haze)
                // With sources, the first page stands above them and the rest waits under More
                // from AniList; without, the results run on as browsing's do.
                let shown = sources ? Array(search.tiles.prefix(max(search.firstPage, 1))) : search.tiles
                ReadGrid(tiles: shown, twoLines: true, note: { tile in joinedLine(tile.work, in: search) }, caption: .formatOrigin)
                if !sources { ReadSearchMore() }
            }
        }
    }

    /// "In Home library", or "In 2 of your sources".
    private func joinedLine(_ work: String, in search: ReadSearchState) -> String? {
        guard let join = search.joined(work), let first = join.sourceNames.first else { return nil }
        return join.sourceNames.count == 1 ? L.readSearchInSource(source: first) : L.readSearchInSources(n: join.sourceNames.count)
    }

    @ViewBuilder
    private func moreFromAnilist(_ search: ReadSearchState) -> some View {
        let first = max(search.firstPage, 1)
        let rest = Array(search.tiles.dropFirst(first))
        if search.hasMore || !rest.isEmpty {
            VStack(alignment: .leading, spacing: 16) {
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    if moreShown {
                        Text(L.readSearchMoreAnilist)
                            .font(.plat(mac: 13, touch: 16, weight: .semibold))
                            .foregroundStyle(.silk)
                            .accessibilityAddTraits(.isHeader)
                    } else {
                        Button {
                            withAnimation(.standard) { moreShown = true }
                            if rest.isEmpty { store.reading.searchMore() }
                        } label: {
                            HStack(spacing: 4) {
                                Text(L.readSearchMoreAnilist)
                                Image(systemName: "chevron.down")
                                    .font(.ui(Device.isMac ? 10 : 12, weight: .semibold))
                                    .accessibilityHidden(true)
                            }
                            .font(.plat(mac: 13, touch: 16, weight: .semibold))
                            .foregroundStyle(.silk)
                            .frame(minHeight: Device.isMac ? 24 : 44)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                        .pointerHover()
                    }
                    let shown = moreShown ? search.tiles.count : first
                    Text(search.total.map { L.readSearchShown(shown: shown, total: Int($0)) } ?? L.readSearchShownSome(shown: shown))
                        .font(.plat(mac: 12, touch: 13))
                        .foregroundStyle(.haze)
                        .contentTransition(.numericText())
                }
                if moreShown {
                    ReadGrid(tiles: rest, twoLines: true, note: { tile in joinedLine(tile.work, in: search) }, caption: .formatOrigin)
                        .transition(.opacity)
                    ReadSearchMore()
                }
            }
            .padding(.top, 32)
        }
    }

    /// One announcement once AniList and every source have settled: how many from each, then
    /// each source that didn't answer.
    private func announce(_ search: ReadSearchState) {
        guard let sources = search.sources else { return }
        let fromSources = sources.groups.reduce(0) { $0 + $1.tiles.count + Int($1.joined) }
        var words = [L.readSearchDone(anilist: search.query.sourcesOnly ? 0 : Int(search.total ?? UInt32(search.tiles.count)), sources: fromSources)]
        for group in sources.groups where group.state.didNotAnswer {
            words.append(L.readSearchSourceFailed(source: group.sourceName))
        }
        AccessibilityNotification.Announcement(words.joined(separator: " ")).post() // i18n-skip
    }
}

extension ReadSourceState {
    var didNotAnswer: Bool {
        switch self {
        case .failed, .timedOut: true
        case .asking, .hits, .nothing: false
        }
    }
}

// MARK: - From your sources

/// From your sources: a row per source with what it found, waiting or what went wrong, and one
/// line at the end for every source that found nothing.
private struct ReadSourcesPart: View {
    @Environment(AppStore.self) private var store
    @Environment(\.homeLayout) private var layout
    let groups: [ReadSourceGroup]

    var body: some View {
        // A source whose every hit joined an AniList result says so there, and has no row.
        let rows = groups.filter { $0.state != .nothing && !($0.state == .hits && $0.tiles.isEmpty) }
        let nothing = groups.filter { $0.state == .nothing }.map(\.sourceName)
        VStack(alignment: .leading, spacing: 0) {
            Text(L.readSearchFromSources)
                .font(.ui(layout.heading.size, weight: layout.heading.weight))
                .fixedLineHeight(layout.heading.line, relativeTo: .title3)
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
                .accessibilityHeading(.h2)
            VStack(alignment: .leading, spacing: 24) {
                ForEach(rows, id: \.sourceId) { group in
                    ReadSourceRow(group: group)
                        .transition(.opacity)
                }
            }
            .padding(.top, rows.isEmpty ? 0 : 12)
            if !nothing.isEmpty {
                VStack(alignment: .leading, spacing: 16) {
                    Rectangle().fill(Palette.faintRule).frame(height: 1).accessibilityHidden(true)
                    Text(L.readSearchSourceNothing(sources: nothing.formatted(.list(type: .and).locale(AirTime.locale))))
                        .font(.plat(mac: 13, touch: 16))
                        .foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.top, rows.isEmpty ? 16 : 28)
                .transition(.opacity)
            }
        }
        .animation(.standard, value: rows.map(\.sourceId))
        .animation(.fadeIn, value: nothing)
    }
}

/// One source's row: its name and kind, then its covers sideways, placeholders while it is asked,
/// or that it didn't answer with Try again for it alone.
private struct ReadSourceRow: View {
    @Environment(AppStore.self) private var store
    @Environment(\.homeLayout) private var layout
    let group: ReadSourceGroup
    /// Whether the hits past the first five are in.
    @State private var expanded = false

    /// A row shows five hits, and Show N more brings in the rest.
    private static let first = 5

    var body: some View {
        let asking = group.state == .asking
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(group.sourceName)
                    .font(.plat(mac: 13, touch: 16, weight: .semibold))
                    .foregroundStyle(.silk)
                if let kind {
                    Text(kind)
                        .font(.plat(mac: 13, touch: 16))
                        .foregroundStyle(.haze)
                }
                if asking {
                    ProgressView()
                        .controlSize(.mini)
                        .alignmentGuide(.firstTextBaseline) { $0[VerticalAlignment.center] + 4 }
                        .accessibilityHidden(true)
                        .transition(.opacity)
                }
            }
            .lineLimit(1)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Text([group.sourceName, kind?.lowercased(with: AirTime.locale)].compactMap { $0 }.joined(separator: ", "))) // i18n-skip
            .accessibilityAddTraits(.isHeader)
            .accessibilityHeading(.h3)
            ReadWaitSlot(waiting: asking) {
                answer
            } placeholder: {
                ReadCoverRowPlaceholder(count: 8)
            }
            .overlay {
                // The placeholders are hidden; a screen reader hears that the source is asked.
                if asking {
                    Color.clear
                        .accessibilityElement()
                        .accessibilityLabel(L.readSearchAsking(source: group.sourceName))
                }
            }
            .animation(.standard, value: group.state)
        }
    }

    /// Server, Folder or Plugin beside the name; the demo shows its name alone.
    private var kind: String? {
        switch group.kind {
        case .server: L.readSearchSourceKindServer
        case .folder: L.readSearchSourceKindFolder
        case .plugin: L.readSearchSourceKindPlugin
        case nil: nil
        }
    }

    @ViewBuilder
    private var answer: some View {
        switch group.state {
        case .failed(let failure):
            failed(reason: failure.words)
        case .timedOut(let seconds):
            failed(reason: L.readSearchSourceTimeout(seconds: Int(seconds)))
        case .hits, .asking, .nothing:
            let hidden = expanded ? 0 : max(0, group.tiles.count - Self.first)
            VStack(alignment: .leading, spacing: 8) {
                HomeShelf(spacing: layout.cardGap) {
                    ForEach(group.tiles.prefix(group.tiles.count - hidden), id: \.work) { tile in
                        ReadTileCard(tile: tile, twoLines: true, caption: .amount)
                            .contextMenu { ReadTileMenu(tile: tile, kind: .details) }
                            .transition(.opacity)
                    }
                }
                .animation(.standard, value: expanded)
                if hidden > 0 {
                    Button(L.readSearchSourceMore(n: hidden)) { expanded = true }
                        .buttonStyle(.plain)
                        .font(.plat(mac: 13, touch: 15, weight: .medium))
                        .foregroundStyle(.silk)
                        .frame(minHeight: Device.isMac ? 24 : 44)
                        .contentShape(.rect)
                        .pointerHover()
                        .transition(.opacity)
                }
            }
            .onChange(of: group.tiles.map(\.work)) { expanded = false }
        }
    }

    /// The vermilion glyph, the source didn't answer, the reason and Try again.
    private func failed(reason: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.plat(mac: 12, touch: 15))
                .foregroundStyle(.vermilion)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L.readSearchSourceFailed(source: group.sourceName))
                        .font(.plat(mac: 13, touch: 16))
                        .foregroundStyle(.silk)
                    Text(reason)
                        .font(.plat(mac: 12, touch: 14))
                        .foregroundStyle(.haze)
                }
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityElement(children: .combine)
                Button(L.actionTryAgain) { store.reading.retrySourceSearch(group.sourceId) }
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 15))
                    .fixedSize()
                    .pointerHover()
                    .padding(.top, 10)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// "2 matches in Shows", and Show them.
private struct OtherScopeRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        if store.medium.both, let n = store.reading.search.otherScope, n > 0 {
            HStack {
                Text(L.searchOtherScope(n: Int(n), scope: L.libraryScopeShows))
                    .foregroundStyle(.silk)
                Spacer(minLength: 8)
                Button {
                    withAnimation(.quick) { navigator.searchReading = false }
                } label: {
                    HStack(spacing: 4) {
                        Text(L.searchShowThem)
                        Image(systemName: "chevron.right").font(.ui(10, weight: .semibold)).accessibilityHidden(true)
                    }
                    .font(.plat(mac: 13, touch: 15, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(minHeight: Device.isMac ? 24 : 44)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .pointerHover()
            }
            .font(.plat(mac: 13, touch: 15))
            .padding(.vertical, 10)
        }
    }
}

// MARK: - Filters

enum ReadFilterWords {
    static func any(_ query: ReadSearchQuery) -> Bool {
        !query.formats.isEmpty || !query.origins.isEmpty || query.status != nil || !query.genres.isEmpty
            || !query.excludeGenres.isEmpty || started(query) != nil
    }

    /// The Started choice set, if any other than Any time.
    static func started(_ query: ReadSearchQuery) -> String? {
        guard let started = query.started, started != "any" else { return nil } // i18n-skip
        return started
    }

    /// What the filters set come to, for a screen reader on the filter button.
    static func summary(_ query: ReadSearchQuery, filters: ReadFilters) -> String {
        var parts = labels(query.formats, in: filters.formats) + labels(query.origins, in: filters.origins)
        if let status = query.status { parts += labels([status], in: filters.statuses) }
        parts += query.genres + query.excludeGenres.map { L.readFilterNotGenre(genre: $0) }
        if let started = started(query) { parts += labels([started], in: filters.started) }
        if query.sourcesOnly { parts.append(L.readSearchSourcesOnly) }
        return parts.joined(separator: ", ") // i18n-skip
    }

    static func labels(_ keys: [String], in choices: [ReadFilterChoice]) -> [String] {
        keys.compactMap { key in choices.first { $0.key == key }?.label.text }
    }

    /// What a filter's button says: its name, or its name and value once set.
    static func value(_ keys: [String], in choices: [ReadFilterChoice]) -> String? {
        let words = labels(keys, in: choices)
        guard let first = words.first else { return nil }
        return words.count == 1 ? first : L.readFilterMany(first: first, more: words.count - 1)
    }

    /// The line over browsing: what is shown, and where the list comes from.
    /// "Popular · from AniList", or once every result is listed, "Popular · 214 series from AniList".
    /// `line` is a View all's own line ("Manga, trending"), which says it better than the filters.
    static func browseLine(_ query: ReadSearchQuery, filters: ReadFilters, total: UInt32? = nil, line: String? = nil) -> String {
        var parts = labels(query.formats, in: filters.formats) + labels(query.origins, in: filters.origins)
        if let status = query.status { parts += labels([status], in: filters.statuses) }
        parts += query.genres
        let what = line ?? (parts.isEmpty ? sort(query.sort ?? filters.sorts.first ?? "") : parts.joined(separator: ", ")) // i18n-skip
        guard let total else { return line ?? L.readSearchBrowseLine(what: what) }
        return [what, L.readSearchTotal(n: Int(total))].joined(separator: " · ") // i18n-skip
    }

    static func sort(_ key: String) -> String {
        switch key {
        case "TRENDING_DESC": L.readSortTrending // i18n-skip
        case "SCORE_DESC": L.readSortScore // i18n-skip
        case "START_DATE_DESC": L.readSortNewest // i18n-skip
        case "TITLE_ROMAJI": L.librarySortTitle // i18n-skip
        default: L.readSortPopular
        }
    }
}

/// Format, Origin, Status and Genres as menu buttons: a set one shows its value, an unset one is
/// words only. On iPhone, one Filters button opens the sheet instead of the genres' menu.
private struct ReadFilterMenus: View {
    @Environment(AppStore.self) private var store
    let compact: Bool
    let openSheet: () -> Void

    var body: some View {
        let filters = store.reading.filters
        let query = store.reading.search.query
        menu(L.readFilterFormat, value: ReadFilterWords.value(query.formats, in: filters.formats)) {
            choices(filters.formats, selected: query.formats) { key in toggle(\.formats, key) }
        }
        menu(L.readFilterOrigin, value: ReadFilterWords.value(query.origins, in: filters.origins)) {
            choices(filters.origins, selected: query.origins) { key in toggle(\.origins, key) }
        }
        menu(L.readFilterStatus, value: query.status.flatMap { ReadFilterWords.value([$0], in: filters.statuses) }) {
            ForEach(filters.statuses, id: \.key) { choice in
                Toggle(choice.label.text, isOn: Binding(
                    get: { query.status == choice.key },
                    set: { on in change { $0.status = on ? choice.key : nil } }
                ))
            }
        }
        if compact {
            button(L.readFilterGenres, value: genresValue(query)) { openSheet() }
            started(filters, query)
        } else {
            menu(L.readFilterGenres, value: genresValue(query)) {
                ForEach(filters.genres, id: \.self) { genre in
                    Menu(genre) {
                        Toggle(L.readFilterIncluded, isOn: Binding(
                            get: { query.genres.contains(genre) },
                            set: { on in change { ReadFilterGenres.set(&$0, genre, on ? .included : .any) } }
                        ))
                        Toggle(L.readFilterLeftOut, isOn: Binding(
                            get: { query.excludeGenres.contains(genre) },
                            set: { on in change { ReadFilterGenres.set(&$0, genre, on ? .leftOut : .any) } }
                        ))
                    }
                }
            }
            started(filters, query)
        }
    }

    private func started(_ filters: ReadFilters, _ query: ReadSearchQuery) -> some View {
        menu(L.readSearchStarted, value: ReadFilterWords.started(query).flatMap { ReadFilterWords.value([$0], in: filters.started) }) {
            ForEach(filters.started, id: \.key) { choice in
                Toggle(choice.label.text, isOn: Binding(
                    get: { (ReadFilterWords.started(query) ?? "any") == choice.key }, // i18n-skip
                    set: { on in change { $0.started = on && choice.key != "any" ? choice.key : nil } } // i18n-skip
                ))
            }
        }
    }

    private func genresValue(_ query: ReadSearchQuery) -> String? {
        let count = query.genres.count + query.excludeGenres.count
        guard count > 0 else { return nil }
        if count == 1 { return query.genres.first ?? L.readFilterNotGenre(genre: query.excludeGenres[0]) }
        return L.readFilterGenresCount(n: count)
    }

    private func choices(_ list: [ReadFilterChoice], selected: [String], toggle: @escaping (String) -> Void) -> some View {
        ForEach(list, id: \.key) { choice in
            Toggle(choice.label.text, isOn: Binding(
                get: { selected.contains(choice.key) },
                set: { _ in toggle(choice.key) }
            ))
        }
    }

    private func toggle(_ path: WritableKeyPath<ReadSearchQuery, [String]>, _ key: String) {
        change { query in
            if let index = query[keyPath: path].firstIndex(of: key) {
                query[keyPath: path].remove(at: index)
            } else {
                query[keyPath: path].append(key)
            }
        }
    }

    private func change(_ edit: (inout ReadSearchQuery) -> Void) {
        var query = store.reading.search.query
        edit(&query)
        store.reading.runSearch(query)
    }

    private func menu<Items: View>(_ name: String, value: String?, @ViewBuilder items: () -> Items) -> some View {
        Menu {
            items()
        } label: {
            ReadFilterLabel(name: name, value: value)
                // Your sources only leaves AniList out, and these filter AniList's results.
                .opacity(store.reading.search.query.sourcesOnly ? 0.45 : 1)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .pointerHover()
        .accessibilityLabel(name)
        .accessibilityValue(value ?? L.readFilterAny)
    }

    private func button(_ name: String, value: String?, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            ReadFilterLabel(name: name, value: value)
                .opacity(store.reading.search.query.sourcesOnly ? 0.45 : 1)
        }
            .buttonStyle(.plain)
            .fixedSize()
            .accessibilityLabel(name)
            .accessibilityValue(value ?? L.readFilterAny)
    }
}

/// A filter's button: "Format" when unset, "Origin: Korea" on the neutral fill when set.
private struct ReadFilterLabel: View {
    let name: String
    let value: String?

    var body: some View {
        HStack(spacing: 6) {
            if let value {
                Text(L.readFilterSet(name: name)).foregroundStyle(.silk)
                Text(value).foregroundStyle(.silk).fontWeight(.semibold)
            } else {
                Text(name).foregroundStyle(.haze)
            }
            Image(systemName: "chevron.down")
                .font(.ui(Device.isMac ? 9 : 11, weight: .semibold))
                .foregroundStyle(value == nil ? .haze : .silk)
                .accessibilityHidden(true)
        }
        .font(.plat(mac: 13, touch: 15, weight: .medium))
        .padding(.horizontal, 12)
        .frame(height: Device.isMac ? 28 : 36)
        // A set filter takes the neutral fill, as the shows' set pills do; amber stays with the actions.
        .background(value == nil ? .clear : Palette.raisedSelected, in: .capsule)
        .animation(.quick, value: value)
        .contentShape(.capsule)
    }
}

enum ReadFilterGenres {
    enum State { case any, included, leftOut }

    static func set(_ query: inout ReadSearchQuery, _ genre: String, _ state: State) {
        query.genres.removeAll { $0 == genre }
        query.excludeGenres.removeAll { $0 == genre }
        switch state {
        case .any: break
        case .included: query.genres.append(genre)
        case .leftOut: query.excludeGenres.append(genre)
        }
    }

    static func state(_ query: ReadSearchQuery, _ genre: String) -> State {
        if query.genres.contains(genre) { return .included }
        if query.excludeGenres.contains(genre) { return .leftOut }
        return .any
    }
}

/// Browsing's order: AniList's sorts, worded as the shows' search words them.
private struct ReadSortMenu: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let query = store.reading.search.query
        let current = query.sort ?? store.reading.filters.sorts.first ?? ""
        Menu {
            Picker(L.shellLibraryOrderLabel, selection: Binding(
                get: { current },
                set: { key in
                    var query = store.reading.search.query
                    query.sort = key
                    store.reading.runSearch(query)
                }
            )) {
                ForEach(store.reading.filters.sorts, id: \.self) { key in
                    Text(ReadFilterWords.sort(key)).tag(key)
                }
            }
            .pickerStyle(.inline)
        } label: {
            HStack(spacing: 6) {
                Image(systemName: "arrow.up.arrow.down").font(.ui(11, weight: .semibold)).accessibilityHidden(true)
                Text(ReadFilterWords.sort(current))
            }
            .font(.plat(mac: 13, touch: 15, weight: .semibold))
            .foregroundStyle(.silk)
            .contentShape(.rect)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .pointerHover()
        .accessibilityLabel(L.shellLibraryOrderLabel)
    }
}

#if os(iOS)
/// The iPhone's filters: with words, Your sources only first as a switch; then format and origin
/// as checked rows, status and started as one choice each, and genres cycling Any, Included and
/// Left out. AniList's filters dim while Your sources only is on.
struct ReadFiltersSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var query = ReadingModel.emptyQuery

    var body: some View {
        let filters = store.reading.filters
        NavigationStack {
            List {
                if !query.text.trimmingCharacters(in: .whitespaces).isEmpty {
                    Section {
                        Toggle(L.readSearchSourcesOnly, isOn: Binding(
                            get: { query.sourcesOnly },
                            set: { on in
                                withAnimation(.fadeIn) { query.sourcesOnly = on }
                                if on { AccessibilityNotification.Announcement(L.readSearchSourcesOnlyLine).post() }
                            }
                        ))
                        .foregroundStyle(.silk)
                    } footer: {
                        Text(L.readSearchSourcesOnlyLine).foregroundStyle(.haze)
                    }
                }
                Group {
                    Section(L.readFilterFormat) { checks(filters.formats, \.formats) }
                    Section(L.readFilterOrigin) { checks(filters.origins, \.origins) }
                    Section(L.readFilterStatus) {
                        ForEach(filters.statuses, id: \.key) { choice in
                            row(choice.label.text, checked: query.status == choice.key) {
                                query.status = query.status == choice.key ? nil : choice.key
                            }
                        }
                    }
                    Section(L.readSearchStarted) {
                        ForEach(filters.started, id: \.key) { choice in
                            row(choice.label.text, checked: (ReadFilterWords.started(query) ?? "any") == choice.key) { // i18n-skip
                                query.started = choice.key == "any" ? nil : choice.key // i18n-skip
                            }
                        }
                    }
                    genres(filters)
                }
                .opacity(query.sourcesOnly ? 0.45 : 1)
            }
            .scrollContentBackground(.hidden)
            .background(.raised)
            .navigationTitle(L.readFilterTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.readFilterReset) {
                        let text = query.text
                        let only = query.sourcesOnly
                        query = ReadingModel.emptyQuery
                        query.text = text
                        query.sourcesOnly = only
                    }
                    .disabled(!ReadFilterWords.any(query))
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.actionDone) {
                        store.reading.runSearch(query)
                        dismiss()
                    }
                }
            }
        }
        .onAppear { query = store.reading.search.query }
        .presentationBackground(.raised)
    }

    private func genres(_ filters: ReadFilters) -> some View {
        Section(L.readFilterGenres) {
            ForEach(filters.genres, id: \.self) { genre in
                let state = ReadFilterGenres.state(query, genre)
                Button {
                    let next: ReadFilterGenres.State = switch state {
                    case .any: .included
                    case .included: .leftOut
                    case .leftOut: .any
                    }
                    ReadFilterGenres.set(&query, genre, next)
                } label: {
                    HStack {
                        Text(genre).foregroundStyle(.silk)
                        Spacer()
                        switch state {
                        case .any: EmptyView()
                        case .included: Text(L.readFilterIncluded).foregroundStyle(.silk)
                        case .leftOut: Text(L.readFilterLeftOut).foregroundStyle(.haze)
                        }
                    }
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func checks(_ choices: [ReadFilterChoice], _ path: WritableKeyPath<ReadSearchQuery, [String]>) -> some View {
        ForEach(choices, id: \.key) { choice in
            row(choice.label.text, checked: query[keyPath: path].contains(choice.key)) {
                if let index = query[keyPath: path].firstIndex(of: choice.key) {
                    query[keyPath: path].remove(at: index)
                } else {
                    query[keyPath: path].append(choice.key)
                }
            }
        }
    }

    private func row(_ title: String, checked: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack {
                Text(title).foregroundStyle(.silk)
                Spacer()
                if checked {
                    Image(systemName: "checkmark").foregroundStyle(.silk).accessibilityHidden(true)
                }
            }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(checked ? .isSelected : [])
    }
}
#endif

/// AniList's grid as it will stand: the line over it and a row of covers with their captions.
private struct ReadSearchPlaceholder: View {
    @State private var width: CGFloat = Device.isMac ? 992 : 360

    var body: some View {
        let (columns, cell) = ReadGrid.columns(width)
        VStack(alignment: .leading, spacing: 12) {
            RoundedRectangle(cornerRadius: 3).fill(.raised).frame(width: 160, height: 10)
            ReadCoverRowPlaceholder(width: cell, count: columns, spacing: ReadGrid.gap)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
        .accessibilityHidden(true)
    }
}
