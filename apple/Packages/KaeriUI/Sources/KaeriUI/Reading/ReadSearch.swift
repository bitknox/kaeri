// Reading's Search, drawn inside Search's own screen so the two scopes share one design: the
// same row of capsules with Filters and the sort at its end, the same filters panel, rails before
// anything is asked and the same grid after. Reading's filters are its own: Genres, Started,
// Origin, Format, Status, and once words are typed Your sources only. With words, From your
// sources and More from AniList follow AniList's covers, and the last row counts the matches in
// the other scope and switches to it.

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
            set: { reading in withAnimation(.standard) { navigator.searchReading = reading } }
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

extension ReadSearchQuery {
    /// Nothing asked yet: no words, no filter and no sort, so Search shows its start, as the
    /// shows' does.
    var atStart: Bool {
        text.trimmingCharacters(in: .whitespaces).isEmpty && sort == nil && !sourcesOnly && !ReadFilterWords.any(self)
    }

    /// The filters taken off, keeping the words, the sort and what is searched, as the shows'
    /// Clear all does.
    @MainActor var cleared: ReadSearchQuery {
        var cleared = ReadingModel.emptyQuery
        cleared.text = text
        cleared.medium = medium
        cleared.sort = sort
        return cleared
    }
}

// MARK: - What Search shows

/// Reading's part of Search under the filter row: the start, the browse grid, or what words found.
/// Start and results crossfade, the one in place of the other, as the shows' do.
struct ReadSearchContent: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var compact = false
    @State private var width: CGFloat = 1_000
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif

    /// The covers' fonts and lines: Home's, as the reading covers everywhere have them.
    private var layout: HomeLayout {
        #if os(iOS)
        HomeLayout(surface: .touch(regular: sizeClass != .compact, column: width))
        #else
        HomeLayout(surface: .mac)
        #endif
    }

    var body: some View {
        let search = store.reading.search
        let start = search.query.atStart
        ZStack(alignment: .topLeading) {
            if start {
                ReadSearchStart(compact: compact)
                    .transition(.opacity)
            } else {
                ReadSearchResults(compact: compact)
                    .transition(.opacity)
            }
        }
        .animation(.standard, value: start)
        .environment(\.homeLayout, layout)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
        .task {
            if let launch = navigator.launchReadSearch {
                navigator.launchReadSearch = nil
                store.reading.runSearch(launch)
            } else if let genre = navigator.readingGenre {
                navigator.readingGenre = nil
                var query = ReadingModel.emptyQuery
                query.genres = [genre]
                store.reading.runSearch(query)
            } else if !search.query.atStart, search.tiles.isEmpty, !search.loading {
                store.reading.runSearch(search.query)
            }
        }
        // Shots: the sources' part in the board's states, `sources-<state>`.
        .task {
            guard let sheet = navigator.launchSheet, sheet.hasPrefix("sources-") else { return } // i18n-skip
            try? await Task.sleep(for: .milliseconds(500))
            navigator.launchSheet = nil
            store.reading.stageSourceSearchShot(String(sheet.dropFirst("sources-".count))) // i18n-skip
        }
        // Shots: `sheet=read-filters` opens the filters, as `filters` does on the shows' side.
        .task {
            guard navigator.launchSheet == "read-filters" else { return } // i18n-skip: a shot's sheet name
            try? await Task.sleep(for: .milliseconds(500))
            navigator.searchFiltersShown = true
            try? await Task.sleep(for: .milliseconds(1_000))
            navigator.launchSheet = nil
        }
    }
}

/// Search's start on reading's side, as on the shows': the rails, here reading Home's discovery
/// rows, each with See all, then the genres.
private struct ReadSearchStart: View {
    @Environment(AppStore.self) private var store
    var compact = false

    var body: some View {
        let home = store.reading.home
        let rows = (home?.rows ?? []).filter { !$0.tiles.isEmpty || $0.pending }
        VStack(alignment: .leading, spacing: 28) {
            if store.demo.state.onlyBooks {
                Text(L.searchDemoOnlyBooks)
                    .font(compact ? .ui(15) : .mac(13))
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
            }
            // A stack, not a Group: a Group would hand the animation to each branch, and the
            // branch coming in would arrive unanimated.
            ZStack(alignment: .topLeading) {
                if !rows.isEmpty {
                    VStack(alignment: .leading, spacing: 28) {
                        ForEach(rows, id: \.id) { row in
                            ReadRailRow(row: row, compact: compact)
                        }
                    }
                    .transition(.opacity)
                } else if home == nil {
                    RailPlaceholder(compact: compact)
                        .transition(.opacity)
                }
            }
            .animation(.standard, value: rows.map(\.id))
            let genres = store.reading.filters.genres
            if !genres.isEmpty {
                SearchGenres(genres: genres, compact: compact) { genre in
                    var query = ReadingModel.emptyQuery
                    query.genres = [genre]
                    store.reading.runSearch(query)
                }
            }
        }
        .task {
            if store.reading.home == nil { store.reading.loadHome() }
        }
    }
}

/// One of the start's rails: a discovery row's heading and See all, then its covers at the
/// shows' rail width, placeholders at their size while the row is on its way.
private struct ReadRailRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let row: ReadHomeRow
    var compact = false

    var body: some View {
        let title = row.title.text
        let waiting = row.pending && row.tiles.isEmpty
        VStack(alignment: .leading, spacing: 12) {
            RailHeader(compact: compact, detail: row.side?.text, seeAll: row.viewAll.map { query in { seeAll(query) } }) {
                // Sentence case on a phone, title case on the Mac, as the shows' rails.
                SectionHeading(compact ? title : title.capitalizedSentence, compact: compact)
                    .accessibilityLabel(waiting ? L.readHomeRowLoading(row: title) : title)
            }
            ReadWaitSlot(waiting: waiting) {
                RailStrip(compact: compact) { width in
                    ForEach(row.tiles, id: \.work) { tile in
                        ReadTileCard(tile: tile, width: width, twoLines: true, planAction: true, caption: row.caption)
                            .contextMenu { ReadTileMenu(tile: tile, kind: .plan) }
                            .transition(.opacity)
                    }
                }
                .animation(.standard, value: row.tiles.map(\.work))
            } placeholder: {
                RailPlates(compact: compact)
            }
        }
    }

    /// See all: the row's list in Search's grid, its first covers read as they did in the rail.
    private func seeAll(_ query: ReadSearchQuery) {
        store.reading.runSearch(query)
        navigator.query = query.text
        navigator.readViewAllLine = row.viewAllTitle.map { (query, $0.text) }
    }
}

/// Reading's answer to a browse with filters or a sort, or to words.
private struct ReadSearchResults: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var compact = false

    var body: some View {
        let search = store.reading.search
        Group {
            if search.hasWords {
                ReadWordsResults(compact: compact)
            } else if let failure = search.failure, search.tiles.isEmpty {
                ReadSearchFailed(failure: failure)
            } else if search.tiles.isEmpty, search.loading || search.page == 0 && search.hasMore {
                HomeWait(active: true, activity: "searching reading") { // i18n-skip
                    ReadGridPlaceholder(compact: compact)
                }
            } else if search.tiles.isEmpty {
                VStack(alignment: .leading, spacing: 0) {
                    ReadNothingFound(title: L.readSearchNothingBrowse, compact: compact)
                    OtherScopeRow()
                }
            } else {
                VStack(alignment: .leading, spacing: 16) {
                    let opened = navigator.readViewAllLine.flatMap { $0.query == search.query ? $0 : nil }
                    Text(ReadFilterWords.browseLine(search.query, filters: store.reading.filters, total: search.total, line: opened?.line))
                        .font(compact ? .ui(15) : .mac(13))
                        .foregroundStyle(.haze)
                    // A row's See all keeps the row's caption, so its first covers read as in the rail.
                    ReadSearchGrid(tiles: search.tiles, compact: compact, caption: .list(search.query, viewAll: opened != nil))
                    ReadSearchMore()
                }
            }
        }
        .animation(.standard, value: search.query)
    }
}

/// Reading's covers in the shows' results grid: the same columns, gaps and rows.
private struct ReadSearchGrid: View {
    @Environment(\.searchCoverWidth) private var cover
    @Environment(\.searchSettledWidth) private var settled
    let tiles: [WorkTile]
    var compact = false
    var note: (WorkTile) -> String? = { _ in nil }
    var caption = ReadCaption.plain

    var body: some View {
        ReadGrid(tiles: tiles, twoLines: true, note: note, caption: caption) { width in
            SearchGridColumns.search(width: width, compact: compact, cover: cover, settled: settled)
        }
    }
}

/// The grid's placeholders, as the shows' results have them.
private struct ReadGridPlaceholder: View {
    @Environment(\.searchCoverWidth) private var cover
    @Environment(\.searchSettledWidth) private var settled
    var compact = false

    var body: some View {
        let spacing: CGFloat = compact ? 12 : 20
        let columns = settled.flatMap { settled in
            cover.map { SearchGridColumns.columns(cover: $0, in: settled, spacing: spacing) }
        }
        PosterGridPlaceholder(count: compact ? 12 : 14, minimum: compact ? 100 : 128, columns: columns, spacing: spacing)
            .accessibilityHidden(true)
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

/// Couldn't search, with Try again: the shows' failed state.
private struct ReadSearchFailed: View {
    @Environment(AppStore.self) private var store
    let failure: ReadingFailure

    var body: some View {
        ScreenFailed(title: L.readSearchFailed, reason: failure.words) {
            store.reading.runSearch(store.reading.search.query)
        }
        .frame(maxWidth: .infinity, minHeight: 360)
    }
}

/// Nothing found, as the shows' Search says it: the title, the line to try other words or fewer
/// filters, and with filters set Clear filters. With words and sources, the line that they found
/// nothing either follows.
private struct ReadNothingFound: View {
    @Environment(AppStore.self) private var store
    let title: String
    var sourcesToo = false
    var compact = false

    var body: some View {
        let query = store.reading.search.query
        let filtered = ReadFilterWords.any(query)
        ContentUnavailableView {
            Label(title, systemImage: "magnifyingglass")
        } description: {
            Text(filtered ? L.readSearchNothingFiltersLine : L.readSearchNothingLine)
            if sourcesToo { Text(L.readSearchSourcesNothing) }
        } actions: {
            if filtered {
                Button(L.readSearchClearFilters) {
                    var cleared = query.cleared
                    cleared.sourcesOnly = query.sourcesOnly
                    store.reading.runSearch(cleared)
                }
                .buttonStyle(SecondaryCapsuleStyle(height: compact ? 44 : 28, fontSize: compact ? 17 : 13))
            }
        }
        .frame(maxWidth: .infinity, minHeight: 240)
    }
}

// MARK: - Results for words

/// With words: AniList's covers in the results grid, then From your sources with one rail per
/// source, then the rest of AniList's results under More from AniList. Your sources only
/// leaves AniList out and the source rows lead.
private struct ReadWordsResults: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var compact = false
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
        // From your sources shows while it has a row or the line of sources that found nothing;
        // once the last row joins AniList's covers, its heading leaves with it.
        let sourcesShown = groups.contains { $0.hasRow || $0.state == .nothing }
        VStack(alignment: .leading, spacing: 0) {
            if anilistNothing, sourcesNothing, !anilistAsking {
                // Nothing anywhere: one title, the line, and the sources' line where there are some.
                ReadNothingFound(title: L.readSearchNothing(query: search.query.text), sourcesToo: !groups.isEmpty, compact: compact)
                    .transition(.opacity)
            } else {
                if !only {
                    anilist(search, asking: anilistAsking, failed: anilistFailed, sources: !groups.isEmpty)
                        .padding(.bottom, sourcesShown ? 28 : 0)
                        .transition(.opacity.animation(.fadeIn))
                }
                if sourcesShown {
                    ReadSourcesPart(groups: groups, compact: compact)
                        .transition(.asymmetric(insertion: .opacity, removal: .opacity.animation(.fadeOut)))
                }
                if !only, !groups.isEmpty, search.firstPage > 0 {
                    moreFromAnilist(search)
                }
            }
            if store.demo.state.onlyBooks {
                Text(L.searchDemoOnlyBooks)
                    .font(compact ? .ui(15) : .mac(13))
                    .foregroundStyle(.haze)
                    .padding(.top, 12)
            }
            OtherScopeRow()
        }
        .animation(reduceMotion ? .fadeIn : .standard, value: only)
        .animation(.standard, value: groups.map(\.sourceId))
        .animation(.standard, value: sourcesShown)
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
                        .buttonStyle(SecondaryCapsuleStyle(height: compact ? 44 : 28, fontSize: compact ? 17 : 13))
                        .fixedSize()
                }
                .padding(.vertical, -24)
            } else if let failure = search.failure {
                ReadSearchFailed(failure: failure)
            }
        } else if asking, search.tiles.isEmpty {
            HomeWait(active: true, activity: "searching reading") { // i18n-skip
                ReadGridPlaceholder(compact: compact)
            }
        } else if search.tiles.isEmpty {
            if sources {
                PlainEmpty(title: L.readSearchAnilistNothing, line: L.readSearchNothingLine)
                    .padding(.vertical, -24)
            } else {
                ReadNothingFound(title: L.readSearchNothing(query: search.query.text), compact: compact)
            }
        } else {
            VStack(alignment: .leading, spacing: 16) {
                // AniList's count only once it is exact; until then the line has no number.
                Text(search.total.map { L.readSearchTotal(n: Int($0)) } ?? L.readSearchTotalUnknown)
                    .font(compact ? .ui(15) : .mac(13))
                    .foregroundStyle(.haze)
                // With sources, the first page stands above them and the rest waits under More
                // from AniList; without, the results run on as browsing's do.
                let shown = sources ? Array(search.tiles.prefix(max(search.firstPage, 1))) : search.tiles
                ReadSearchGrid(tiles: shown, compact: compact, note: { tile in joinedLine(tile.work, in: search) }, caption: .formatOrigin)
                if !sources { ReadSearchMore() }
            }
        }
    }

    /// "In Home library", "In your folders", or "In 2 of your sources".
    private func joinedLine(_ work: String, in search: ReadSearchState) -> String? {
        guard let join = search.joined(work), let first = join.sourceNames.first else { return nil }
        guard join.sourceNames.count == 1 else { return L.readSearchInSources(n: join.sourceNames.count) }
        // The folders search as one source, named Folders.
        if join.sourceIds.first == "folders" { return L.readSearchInFolders } // i18n-skip: the engine's id
        return L.readSearchInSource(source: first)
    }

    @ViewBuilder
    private func moreFromAnilist(_ search: ReadSearchState) -> some View {
        let first = max(search.firstPage, 1)
        let rest = Array(search.tiles.dropFirst(first))
        if search.hasMore || !rest.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    if moreShown {
                        SectionHeading(L.readSearchMoreAnilist, compact: compact)
                    } else {
                        Button {
                            withAnimation(.standard) { moreShown = true }
                            if rest.isEmpty { store.reading.searchMore() }
                        } label: {
                            // The heading as a disclosure, its chevron as Trending's heading has it.
                            let chevron = Text(Image(systemName: "chevron.down"))
                                .font(.ui(compact ? 12 : 10, weight: .semibold))
                                .foregroundStyle(.haze)
                                .baselineOffset(compact ? 2 : 1)
                            Text("\(L.readSearchMoreAnilist)\u{00A0}\(chevron)")
                                .font(compact ? .ui(20, weight: .semibold) : .mac(15, weight: .semibold))
                                .foregroundStyle(.silk)
                                .frame(minHeight: compact ? 44 : 28)
                                .contentShape(.rect)
                                .accessibilityLabel(L.readSearchMoreAnilist)
                        }
                        .buttonStyle(.plain)
                        .pointerHover()
                    }
                    let shown = moreShown ? search.tiles.count : first
                    Text(search.total.map { L.readSearchShown(shown: shown, total: Int($0)) } ?? L.readSearchShownSome(shown: shown))
                        .font(compact ? .ui(15) : .mac(13))
                        .foregroundStyle(.haze)
                        .contentTransition(.numericText())
                }
                if moreShown {
                    ReadSearchGrid(tiles: rest, compact: compact, note: { tile in joinedLine(tile.work, in: search) }, caption: .formatOrigin)
                        .transition(.opacity)
                    ReadSearchMore()
                }
            }
            .padding(.top, 28)
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

extension ReadSourceGroup {
    /// Whether the source has a row under From your sources: one whose every hit joined an
    /// AniList result says so there and has none, and one that found nothing is in the line.
    var hasRow: Bool {
        state != .nothing && !(state == .hits && tiles.isEmpty)
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

/// From your sources: a rail per source with what it found, waiting or what went wrong, and one
/// line at the end for every source that found nothing.
private struct ReadSourcesPart: View {
    let groups: [ReadSourceGroup]
    var compact = false

    var body: some View {
        let rows = groups.filter(\.hasRow)
        let nothing = groups.filter { $0.state == .nothing }.map(\.sourceName)
        VStack(alignment: .leading, spacing: 0) {
            SectionHeading(L.readSearchFromSources, compact: compact)
                .frame(minHeight: compact ? 44 : 28, alignment: .leading)
                .accessibilityHeading(.h2)
            VStack(alignment: .leading, spacing: 28) {
                ForEach(rows, id: \.sourceId) { group in
                    ReadSourceRow(group: group, compact: compact)
                        .transition(.opacity)
                }
            }
            .padding(.top, rows.isEmpty ? 0 : 12)
            if !nothing.isEmpty {
                VStack(alignment: .leading, spacing: 16) {
                    Rectangle().fill(Palette.faintRule).frame(height: 1).accessibilityHidden(true)
                    Text(L.readSearchSourceNothing(sources: nothing.formatted(.list(type: .and).locale(AirTime.locale))))
                        .font(compact ? .ui(15) : .mac(13))
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

/// One source's rail: its name and kind, then its covers sideways, placeholders while it is
/// asked, or that it didn't answer with Try again for it alone.
private struct ReadSourceRow: View {
    @Environment(AppStore.self) private var store
    let group: ReadSourceGroup
    var compact = false
    /// Whether the hits past the first five are in.
    @State private var expanded = false

    /// A row shows five hits, and Show N more brings in the rest.
    private static let first = 5

    var body: some View {
        let asking = group.state == .asking
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(group.sourceName)
                    .font(compact ? .ui(17, weight: .semibold) : .mac(13, weight: .semibold))
                    .foregroundStyle(.silk)
                if let kind {
                    Text(kind)
                        .font(compact ? .ui(15) : .mac(13))
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
                RailPlates(compact: compact)
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
                RailStrip(compact: compact) { width in
                    ForEach(group.tiles.prefix(group.tiles.count - hidden), id: \.work) { tile in
                        ReadTileCard(tile: tile, width: width, twoLines: true, caption: .amount)
                            .contextMenu { ReadTileMenu(tile: tile, kind: .details) }
                            .transition(.opacity)
                    }
                }
                .animation(.standard, value: expanded)
                if hidden > 0 {
                    Button(L.readSearchSourceMore(n: hidden)) { expanded = true }
                        .buttonStyle(.plain)
                        .font(compact ? .ui(15, weight: .semibold) : .mac(13, weight: .semibold))
                        .foregroundStyle(.silk)
                        .frame(minHeight: compact ? 44 : 24)
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
                .font(compact ? .ui(15) : .mac(12))
                .foregroundStyle(.vermilion)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(L.readSearchSourceFailed(source: group.sourceName))
                        .font(compact ? .ui(17) : .mac(13))
                        .foregroundStyle(.silk)
                    Text(reason)
                        .font(compact ? .ui(15) : .mac(12))
                        .foregroundStyle(.haze)
                }
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityElement(children: .combine)
                Button(L.actionTryAgain) { store.reading.retrySourceSearch(group.sourceId) }
                    .buttonStyle(SecondaryCapsuleStyle(height: compact ? 44 : 28, fontSize: compact ? 17 : 13))
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
                    withAnimation(.standard) { navigator.searchReading = false }
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
        chips(query, filters: filters).map(\.label).joined(separator: ", ") // i18n-skip
    }

    /// The words if there are any, and otherwise the filters set or the order, in one line as the
    /// shows' search says it under the Mac title: "Fantasy · Japan · Manga".
    static func headline(_ query: ReadSearchQuery, filters: ReadFilters) -> String {
        let words = query.text.trimmingCharacters(in: .whitespaces)
        guard words.isEmpty else { return words }
        let set = chips(query, filters: filters).map(\.label)
        return set.isEmpty ? query.sort.map(sort) ?? "" : set.joined(separator: " \u{b7} ") // i18n-skip
    }

    /// Each filter set, keyed by what it filters, worded as its removable chip says it.
    static func chips(_ query: ReadSearchQuery, filters: ReadFilters) -> [FilterChipItem] {
        var chips = [FilterChipItem]()
        chips += query.genres.map { FilterChipItem(key: "genre:" + $0, label: $0) } // i18n-skip: a key
        chips += query.excludeGenres.map { FilterChipItem(key: "not:" + $0, label: L.readFilterNotGenre(genre: $0)) } // i18n-skip
        if let started = started(query), let label = labels([started], in: filters.started).first {
            chips.append(FilterChipItem(key: "started", label: label)) // i18n-skip
        }
        chips += query.origins.compactMap { key in labels([key], in: filters.origins).first.map { FilterChipItem(key: "origin:" + key, label: $0) } } // i18n-skip
        chips += query.formats.compactMap { key in labels([key], in: filters.formats).first.map { FilterChipItem(key: "format:" + key, label: $0) } } // i18n-skip
        if let status = query.status, let label = labels([status], in: filters.statuses).first {
            chips.append(FilterChipItem(key: "status", label: label)) // i18n-skip
        }
        if query.sourcesOnly { chips.append(FilterChipItem(key: "sources", label: L.readSearchSourcesOnly)) } // i18n-skip
        return chips
    }

    /// The query with one chip's filter taken off.
    static func removing(_ key: String, from query: ReadSearchQuery) -> ReadSearchQuery {
        var query = query
        let (kind, value) = key.split(separator: ":", maxSplits: 1).map(String.init).splitPair
        switch kind {
        case "genre": query.genres.removeAll { $0 == value } // i18n-skip
        case "not": query.excludeGenres.removeAll { $0 == value } // i18n-skip
        case "started": query.started = nil // i18n-skip
        case "origin": query.origins.removeAll { $0 == value } // i18n-skip
        case "format": query.formats.removeAll { $0 == value } // i18n-skip
        case "status": query.status = nil // i18n-skip
        case "sources": query.sourcesOnly = false // i18n-skip
        default: break
        }
        return query
    }

    static func labels(_ keys: [String], in choices: [ReadFilterChoice]) -> [String] {
        keys.compactMap { key in choices.first { $0.key == key }?.label.text }
    }

    /// What a filter's capsule says once set: its value, or the first and how many more.
    static func value(_ keys: [String], in choices: [ReadFilterChoice]) -> String? {
        let words = labels(keys, in: choices)
        guard let first = words.first else { return nil }
        return words.count == 1 ? first : L.readFilterMany(first: first, more: words.count - 1)
    }

    /// The line over browsing: what is shown, and where the list comes from.
    /// "Popular · from AniList", or once every result is listed, "Popular · 214 series from AniList".
    /// `line` is a See all's own line ("Manga, trending"), which says it better than the filters.
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
        default: L.sortPopularity
        }
    }
}

private extension Array where Element == String {
    /// The first and second of a split key, the second empty when there is none.
    var splitPair: (String, String) { (first ?? "", count > 1 ? self[1] : "") }
}

enum ReadFilterGenres {
    static func set(_ query: inout ReadSearchQuery, _ genre: String, _ pick: Pick) {
        query.genres.removeAll { $0 == genre }
        query.excludeGenres.removeAll { $0 == genre }
        switch pick {
        case .off: break
        case .include: query.genres.append(genre)
        case .exclude: query.excludeGenres.append(genre)
        }
    }

    static func pick(_ query: ReadSearchQuery, _ genre: String) -> Pick {
        if query.genres.contains(genre) { return .include }
        if query.excludeGenres.contains(genre) { return .exclude }
        return .off
    }
}

/// Reading's capsules in the shows' row, in the shows' order and slots: Genres, Started (the
/// shows' Year), Origin (Season), Format, Status, and with words Your sources only (Available).
/// Each is the shows' capsule menu; a set one says its value.
struct ReadFilterCapsules: View {
    @Environment(AppStore.self) private var store
    var compact = false
    /// The iPhone's pinned row: a set capsule in the neutral fill.
    var pinned = false

    var body: some View {
        let filters = store.reading.filters
        let query = store.reading.search.query
        // Your sources only leaves AniList out, and these filter AniList's results.
        Group {
            genres(filters, query).id(PinnedFilterState.Pill.genres)
            started(filters, query).id(PinnedFilterState.Pill.year)
            menu(L.readFilterOrigin, value: ReadFilterWords.value(query.origins, in: filters.origins)) {
                choices(filters.origins, selected: query.origins) { key in toggle(\.origins, key) }
                if !query.origins.isEmpty {
                    Divider()
                    Button(L.readFilterAnyOrigin) { change { $0.origins = [] } }
                }
            }
            .id(PinnedFilterState.Pill.season)
            menu(L.readFilterFormat, value: ReadFilterWords.value(query.formats, in: filters.formats)) {
                choices(filters.formats, selected: query.formats) { key in toggle(\.formats, key) }
                if !query.formats.isEmpty {
                    Divider()
                    Button(L.filterAnyFormat) { change { $0.formats = [] } }
                }
            }
            .id(PinnedFilterState.Pill.formats)
            menu(L.readFilterStatus, value: query.status.flatMap { ReadFilterWords.value([$0], in: filters.statuses) }) {
                ForEach(filters.statuses, id: \.key) { choice in
                    Toggle(choice.label.text, isOn: Binding(
                        get: { store.reading.search.query.status == choice.key },
                        set: { on in change { $0.status = on ? choice.key : nil } }
                    ))
                }
                if query.status != nil {
                    Divider()
                    Button(L.filterAnyStatus) { change { $0.status = nil } }
                }
            }
            .id(PinnedFilterState.Pill.statuses)
        }
        .opacity(query.sourcesOnly ? 0.45 : 1)
        if store.reading.search.hasWords {
            sourcesOnly(query)
                .id(PinnedFilterState.Pill.available)
                .transition(.opacity)
        }
    }

    /// The shows' genres menu: each genre a toggle, Leave out for the ones to leave out, and Any
    /// genre once one is set.
    private func genres(_ filters: ReadFilters, _ query: ReadSearchQuery) -> some View {
        let count = query.genres.count + query.excludeGenres.count
        let value: String? = count == 0 ? nil
            : count == 1 ? (query.genres.first ?? L.readFilterNotGenre(genre: query.excludeGenres[0]))
            : L.readFilterGenresCount(n: count)
        return menu(L.readFilterGenres, value: value) {
            ForEach(filters.genres, id: \.self) { genre in
                Toggle(genre, isOn: pick(genre, .include))
            }
            if !filters.genres.isEmpty {
                Divider()
                Menu(L.searchLeaveOut) {
                    ForEach(filters.genres, id: \.self) { genre in
                        Toggle(genre, isOn: pick(genre, .exclude))
                    }
                }
            }
            if count > 0 {
                Divider()
                Button(L.filterAnyGenre) { change { $0.genres = []; $0.excludeGenres = [] } }
            }
        }
    }

    private func pick(_ genre: String, _ pick: Pick) -> Binding<Bool> {
        Binding(
            get: { ReadFilterGenres.pick(store.reading.search.query, genre) == pick },
            set: { on in change { ReadFilterGenres.set(&$0, genre, on ? pick : .off) } }
        )
    }

    private func started(_ filters: ReadFilters, _ query: ReadSearchQuery) -> some View {
        menu(L.readSearchStarted, value: ReadFilterWords.started(query).flatMap { ReadFilterWords.value([$0], in: filters.started) }) {
            ForEach(filters.started, id: \.key) { choice in
                Toggle(choice.label.text, isOn: Binding(
                    get: { (ReadFilterWords.started(store.reading.search.query) ?? "any") == choice.key }, // i18n-skip
                    set: { on in change { $0.started = on && choice.key != "any" ? choice.key : nil } } // i18n-skip
                ))
            }
        }
    }

    /// Your sources only, in Available's place: on, the shows' set capsule. Announced once as it
    /// turns on.
    private func sourcesOnly(_ query: ReadSearchQuery) -> some View {
        Button {
            let on = !query.sourcesOnly
            change { $0.sourcesOnly = on }
            if on { AccessibilityNotification.Announcement(L.readSearchSourcesOnlyLine).post() }
        } label: {
            FilterCapsule(title: L.readSearchSourcesOnly, active: query.sourcesOnly, compact: compact,
                          symbol: query.sourcesOnly ? "checkmark" : nil, chevron: false, pinned: pinned)
        }
        .buttonStyle(.plain)
        .fixedSize()
        .pointerHover()
        .accessibilityRepresentation {
            Toggle(L.readSearchSourcesOnly, isOn: Binding(
                get: { store.reading.search.query.sourcesOnly },
                set: { on in change { $0.sourcesOnly = on } }
            ))
        }
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
            FilterCapsule(title: value ?? name, active: value != nil, compact: compact, pinned: pinned)
        }
        .filterMenu()
        .accessibilityLabel(name)
        .accessibilityValue(value ?? L.readFilterAny)
    }
}

/// What decides the iPhone's pinned row's width and order on reading's side: Clear all, what the
/// capsules say and the sort. The words typed change nothing.
struct ReadRowLayout: Hashable {
    let clearAll: Bool
    let words: [String]
    let sort: String?

    init(_ query: ReadSearchQuery, filters: ReadFilters) {
        clearAll = ReadFilterWords.any(query) || query.sourcesOnly
        words = ReadFilterWords.chips(query, filters: filters).map(\.label)
        sort = query.sort
    }
}

/// Reading's sort in the shows' sort capsule: AniList's orders, and with words Relevance first,
/// the order AniList gives words when none is chosen.
struct ReadSortMenu: View {
    @Environment(AppStore.self) private var store
    var compact = false
    var pinned = false
    /// The glyph alone, where the Mac's row is short of room.
    var glyph = false

    var body: some View {
        let label = Self.label(store)
        Menu {
            Picker(L.searchSortBy, selection: Binding<String?>(
                get: { Self.inEffect(store) },
                set: { key in
                    var query = store.reading.search.query
                    query.sort = key
                    store.reading.runSearch(query)
                }
            )) {
                if store.reading.search.hasWords {
                    Text(L.sortRelevance).tag(String?.none)
                }
                ForEach(store.reading.filters.sorts, id: \.self) { key in
                    Text(ReadFilterWords.sort(key)).tag(String?.some(key))
                }
            }
            .pickerStyle(.inline)
        } label: {
            Self.capsule(store, compact: compact, pinned: pinned, glyph: glyph)
        }
        .filterMenu()
        .accessibilityLabel(L.sortLabel(sort: label))
        .tooltip(glyph ? L.sortLabel(sort: label) : "")
    }

    /// The sort in effect: the one chosen, else relevance with words and AniList's first without.
    static func inEffect(_ store: AppStore) -> String? {
        let search = store.reading.search
        return search.query.sort ?? (search.hasWords ? nil : store.reading.filters.sorts.first)
    }

    static func label(_ store: AppStore) -> String {
        inEffect(store).map(ReadFilterWords.sort) ?? L.sortRelevance
    }

    static func capsule(_ store: AppStore, compact: Bool, pinned: Bool = false, glyph: Bool) -> FilterCapsule {
        FilterCapsule(
            title: glyph ? "" : label(store), active: store.reading.search.query.sort != nil,
            compact: compact, symbol: "arrow.up.arrow.down", pinned: pinned)
    }
}

/// Every reading filter, in the shows' panel: beside the results on the Mac, the iPad's
/// inspector, the iPhone's sheet. With words, Your sources only leads. Each change searches.
struct ReadFiltersPanel: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var compact = false
    /// How the panel closes, where it is not Search's own (View all's sheet on the iPhone).
    var close: (() -> Void)?
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif

    /// The iPhone's sheet picks in the neutral fill, as the shows' sheet has them.
    private var neutral: Bool {
        #if os(iOS)
        compact && sizeClass == .compact
        #else
        false
        #endif
    }

    var body: some View {
        let query = store.reading.search.query
        let filters = store.reading.filters
        VStack(spacing: 0) {
            FiltersPanelHeader(
                compact: compact, canReset: ReadFilterWords.any(query) || query.sourcesOnly,
                reset: {
                    var cleared = query.cleared
                    cleared.sort = query.sort
                    store.reading.runSearch(cleared)
                },
                close: dismiss)
                .padding(.horizontal, compact ? 16 : 14)
                .padding(.top, compact ? 16 : 14)
                .padding(.bottom, compact ? 8 : 6)
            ScrollView {
                VStack(alignment: .leading, spacing: compact ? 28 : 24) {
                    if store.reading.search.hasWords {
                        PanelSection(compact: compact, L.readSearchFromSources) {
                            PanelSwitch(L.readSearchSourcesOnly, isOn: Binding(
                                get: { store.reading.search.query.sourcesOnly },
                                set: { on in
                                    change { $0.sourcesOnly = on }
                                    if on { AccessibilityNotification.Announcement(L.readSearchSourcesOnlyLine).post() }
                                }
                            ))
                            if query.sourcesOnly {
                                Text(L.readSearchSourcesOnlyLine)
                                    .font(compact ? .ui(13) : .mac(11))
                                    .foregroundStyle(.haze)
                                    .fixedSize(horizontal: false, vertical: true)
                                    .transition(.opacity)
                            }
                        }
                    }
                    Group {
                        PanelSection(compact: compact, L.readFilterGenres) {
                            FlowLayout(spacing: 6, lineSpacing: 6) {
                                ForEach(filters.genres, id: \.self) { genre in
                                    PickChip(name: genre, pick: ReadFilterGenres.pick(query, genre), compact: compact, neutral: neutral) { pick in
                                        change { ReadFilterGenres.set(&$0, genre, pick) }
                                    }
                                }
                            }
                            .padding(.vertical, 4)
                        }
                        PanelSection(compact: compact, L.readSearchStarted) {
                            chips(filters.started, on: { (ReadFilterWords.started(query) ?? "any") == $0 }) { key in // i18n-skip
                                change { $0.started = key == "any" ? nil : key } // i18n-skip
                            }
                        }
                        PanelSection(compact: compact, L.readFilterOrigin) {
                            chips(filters.origins, on: { query.origins.contains($0) }) { key in toggle(\.origins, key) }
                        }
                        PanelSection(compact: compact, L.readFilterFormat) {
                            chips(filters.formats, on: { query.formats.contains($0) }) { key in toggle(\.formats, key) }
                        }
                        PanelSection(compact: compact, L.readFilterStatus) {
                            chips(filters.statuses, on: { query.status == $0 }) { key in
                                change { $0.status = $0.status == key ? nil : key }
                            }
                        }
                    }
                    .opacity(query.sourcesOnly ? 0.45 : 1)
                    Text(L.filterLiveNote)
                        .font(compact ? .ui(13) : .mac(11))
                        .foregroundStyle(.haze)
                }
                .padding(compact ? 16 : 14)
                .animation(.fadeIn, value: query.sourcesOnly)
            }
            .scrollIndicators(.never)
        }
        .background(Palette.ground)
        .tint(.amber)
        .toggleStyle(.switch)
        .closesOnEscape { dismiss() }
    }

    private func dismiss() {
        if let close { close() } else { navigator.searchFiltersShown = false }
    }

    private func chips(_ choices: [ReadFilterChoice], on: @escaping (String) -> Bool, tap: @escaping (String) -> Void) -> some View {
        FlowLayout(spacing: 6, lineSpacing: 6) {
            ForEach(choices, id: \.key) { choice in
                ToggleChip(name: choice.label.text, on: on(choice.key), compact: compact, neutral: neutral) {
                    tap(choice.key)
                }
            }
        }
        .padding(.vertical, 4)
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
}
