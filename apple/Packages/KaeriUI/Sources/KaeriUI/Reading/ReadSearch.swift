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
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    private var compact: Bool { sizeClass == .compact }
    #else
    private let compact = false
    #endif

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
                        Spacer(minLength: 8)
                        if browsing { ReadSortMenu() }
                    }
                }
                ReadSearchBody(browsing: browsing)
                    .animation(.standard, value: search.query)
            }
            .padding(EdgeInsets(top: 8, leading: 16, bottom: 24, trailing: 16))
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .scrollIndicators(.never)
        .background(.ground)
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
        #if os(iOS)
        .sheet(isPresented: $filtersShown) { ReadFiltersSheet() }
        #endif
    }
}

private struct ReadSearchBody: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let browsing: Bool

    var body: some View {
        let search = store.reading.search
        if let failure = search.failure, search.tiles.isEmpty {
            PlainEmpty(title: L.readSearchFailed, line: failure.words) {
                Button(L.actionTryAgain) { store.reading.runSearch(search.query) }
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                    .fixedSize()
            }
        } else if search.tiles.isEmpty, search.loading || search.page == 0 && search.hasMore {
            HomeWait(active: true, activity: "searching reading") { // i18n-skip
                ReadSearchPlaceholder(browsing: browsing)
            }
        } else if search.tiles.isEmpty {
            VStack(alignment: .leading, spacing: 0) {
                PlainEmpty(title: browsing ? L.readSearchNothingBrowse : L.readSearchNothing(query: search.query.text),
                           line: ReadFilterWords.any(search.query) ? L.readSearchNothingFiltersLine : L.readSearchNothingLine) {
                    if ReadFilterWords.any(search.query) {
                        Button(L.readSearchClearFilters) {
                            var query = ReadingModel.emptyQuery
                            query.text = search.query.text
                            store.reading.runSearch(query)
                        }
                        .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                        .fixedSize()
                    }
                }
                OtherScopeRow()
            }
        } else if browsing {
            VStack(alignment: .leading, spacing: 12) {
                Text(ReadFilterWords.browseLine(search.query, filters: store.reading.filters, total: search.total))
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(.haze)
                ReadGrid(tiles: search.tiles)
                more
            }
        } else {
            VStack(alignment: .leading, spacing: 0) {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(search.tiles, id: \.work) { tile in
                        Rectangle().fill(Palette.faintRule).frame(height: 1).accessibilityHidden(true)
                        ReadResultRow(tile: tile)
                    }
                    Rectangle().fill(Palette.faintRule).frame(height: 1).accessibilityHidden(true)
                }
                more
                if store.demo.state.onlyBooks {
                    Text(L.searchDemoOnlyBooks)
                        .font(.plat(mac: 12, touch: 13))
                        .foregroundStyle(.haze)
                        .padding(.top, 12)
                }
                OtherScopeRow()
            }
            .frame(maxWidth: Device.isMac ? 620 : .infinity, alignment: .leading)
        }
    }

    @ViewBuilder
    private var more: some View {
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

/// A result: the cover, the title and its facts, and your list on the right.
private struct ReadResultRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let tile: WorkTile

    var body: some View {
        Button {
            navigator.openSeries(tile.work)
        } label: {
            HStack(spacing: 12) {
                BookCover(url: tile.cover, width: Device.isMac ? 40 : 44, radius: 4)
                VStack(alignment: .leading, spacing: 2) {
                    Text(tile.title)
                        .font(.plat(mac: 13, touch: 17, weight: .semibold))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    Text(store.reading.series[tile.work]?.demo != nil ? L.searchDemoReads : tile.facts.map(\.text).joined(separator: " · ")) // i18n-skip
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                }
                Spacer(minLength: 8)
                if let next = tile.next {
                    Text(next.text)
                        .font(.plat(mac: 12, touch: 15))
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                }
            }
            .padding(.vertical, 6)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .contextMenu { ReadTileMenu(tile: tile) }
        .accessibilityElement(children: .combine)
        // The demo's own titles are the only ones asked about: the series says whether it is one.
        .task { if store.demo.reading.works.contains(where: { $0.series == tile.title }) { store.reading.loadSeries(tile.work) } }
    }
}

// MARK: - Filters

enum ReadFilterWords {
    static func any(_ query: ReadSearchQuery) -> Bool {
        !query.formats.isEmpty || !query.origins.isEmpty || query.status != nil || !query.genres.isEmpty
            || !query.excludeGenres.isEmpty
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
    static func browseLine(_ query: ReadSearchQuery, filters: ReadFilters, total: UInt32? = nil) -> String {
        var parts = labels(query.formats, in: filters.formats) + labels(query.origins, in: filters.origins)
        if let status = query.status { parts += labels([status], in: filters.statuses) }
        parts += query.genres
        let what = parts.isEmpty ? sort(query.sort ?? filters.sorts.first ?? "") : parts.joined(separator: ", ") // i18n-skip
        guard let total else { return L.readSearchBrowseLine(what: what) }
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
        Button(action: action) { ReadFilterLabel(name: name, value: value) }
            .buttonStyle(.plain)
            .fixedSize()
            .accessibilityLabel(name)
            .accessibilityValue(value ?? L.readFilterAny)
    }
}

/// A filter's button: "Format" when unset, "Origin: Korea" in an outline when set.
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
        .overlay {
            if value != nil { Capsule().strokeBorder(Palette.glassRule, lineWidth: 1) }
        }
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
/// The iPhone's filters: format and origin as checked rows, status as one choice, and genres
/// cycling Any, Included and Left out.
private struct ReadFiltersSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var query = ReadingModel.emptyQuery

    var body: some View {
        let filters = store.reading.filters
        NavigationStack {
            List {
                Section(L.readFilterFormat) { checks(filters.formats, \.formats) }
                Section(L.readFilterOrigin) { checks(filters.origins, \.origins) }
                Section(L.readFilterStatus) {
                    ForEach(filters.statuses, id: \.key) { choice in
                        row(choice.label.text, checked: query.status == choice.key) {
                            query.status = query.status == choice.key ? nil : choice.key
                        }
                    }
                }
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
            .scrollContentBackground(.hidden)
            .background(.raised)
            .navigationTitle(L.readFilterTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.readFilterReset) {
                        let text = query.text
                        query = ReadingModel.emptyQuery
                        query.text = text
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

private struct ReadSearchPlaceholder: View {
    let browsing: Bool

    var body: some View {
        if browsing {
            VStack(alignment: .leading, spacing: 12) {
                RoundedRectangle(cornerRadius: 3).fill(.raised).frame(width: 260, height: 10)
                let (columns, cell) = ReadGrid.columns(Device.isMac ? 992 : 360)
                LazyVGrid(columns: Array(repeating: GridItem(.fixed(cell), spacing: ReadGrid.gap), count: columns),
                          alignment: .leading, spacing: 24) {
                    ForEach(0..<columns, id: \.self) { _ in
                        RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: cell, height: cell * 1.5)
                    }
                }
            }
            .accessibilityHidden(true)
        } else {
            VStack(alignment: .leading, spacing: 12) {
                ForEach(0..<4, id: \.self) { _ in
                    HStack(spacing: 12) {
                        RoundedRectangle(cornerRadius: 4).fill(.raised).frame(width: 40, height: 60)
                        VStack(alignment: .leading, spacing: 6) {
                            RoundedRectangle(cornerRadius: 3).fill(.raised).frame(width: 180, height: 10)
                            RoundedRectangle(cornerRadius: 3).fill(.raised).frame(width: 120, height: 8)
                        }
                    }
                }
            }
            .accessibilityHidden(true)
        }
    }
}
