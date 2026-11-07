// The reading library: one cover grid for every list, as for shows. AniList's six lists, then
// Your files for series in folders and servers on no list. With both media on, a segmented
// control with words sets Shows or Reading, first in the content; with one there is none.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

enum ReadLibraryScope {
    /// Whether the Library shows reading: always with reading alone, by the control with both.
    @MainActor
    static func reading(store: AppStore, navigator: Navigator) -> Bool {
        switch store.medium.answer {
        case .read: true
        case .both: navigator.libraryReading
        case .watch: false
        }
    }
}

/// The reading library's order. The engine sends each list most recently read first.
enum ReadLibrarySort: String, CaseIterable, Identifiable {
    case lastRead, title

    var id: Self { self }

    var shown: String {
        switch self {
        case .lastRead: L.readSortLastRead
        case .title: L.librarySortTitle
        }
    }
}

/// Shows or Reading, with words, as the board puts it first in the Library.
struct LibraryScopePicker: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        @Bindable var navigator = navigator
        Picker(L.libraryScopeLabel, selection: Binding(
            get: { navigator.libraryReading },
            set: { reading in withAnimation(.quick) { navigator.libraryReading = reading } }
        )) {
            Text(L.libraryScopeShows).tag(false)
            Text(L.libraryScopeReading).tag(true)
        }
        .pickerStyle(.segmented)
        .labelsHidden()
        #if os(macOS)
        // Its own width, so it starts on the content's edge as the covers do.
        .fixedSize()
        #else
        .frame(maxWidth: .infinity)
        #endif
    }
}

/// The reading library's list menu, its order, and the covers.
struct ReadLibraryView: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @AppStorage("kaeri.readLibrarySort") private var sort: ReadLibrarySort = .lastRead
    /// Before the list menu: Shows or Reading, with both media on.
    var scope = false

    var body: some View {
        let segment = navigator.readingSegment
        VStack(alignment: .leading, spacing: Device.isMac ? 16 : 12) {
            HStack(spacing: 12) {
                if scope, Device.isMac { LibraryScopePicker() }
                ReadListMenu()
                Spacer(minLength: 8)
                sortMenu
            }
            grid(segment)
                .id(segment)
                .transition(.opacity)
        }
        .animation(.standard, value: segment)
        .task {
            for list in ReadWords.lists where !store.reading.listsLoaded.contains(list) {
                store.reading.loadList(list)
            }
        }
    }

    private var sortMenu: some View {
        Menu {
            Picker(L.shellLibraryOrderLabel, selection: $sort) {
                ForEach(ReadLibrarySort.allCases) { Text($0.shown).tag($0) }
            }
            .pickerStyle(.inline)
        } label: {
            HStack(spacing: 6) {
                Image(systemName: "arrow.up.arrow.down").font(.ui(Device.isMac ? 11 : 13, weight: .semibold))
                    .accessibilityHidden(true)
                Text(sort.shown)
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
        .accessibilityValue(sort.shown)
    }

    @ViewBuilder
    private func grid(_ segment: LibrarySegmentRead) -> some View {
        if store.reading.lists[segment] == nil, let failure = store.reading.failures[.library] {
            PlainEmpty(title: L.libraryListLoadFailed(list: ReadWords.list(segment)), line: failure.words) {
                Button(L.actionTryAgain) { store.reading.loadList(segment) }
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                    .fixedSize()
            }
        } else if !store.reading.listsLoaded.contains(segment) {
            HomeWait(active: true, activity: "loading your reading list") { // i18n-skip
                ReadGridPlaceholder()
            }
        } else if let tiles = store.reading.lists[segment], !tiles.isEmpty {
            ReadGrid(tiles: sorted(tiles))
        } else if segment == .yourFiles {
            PlainEmpty(title: L.readLibraryFilesEmptyTitle, line: L.readLibraryFilesEmptyLine) {
                Button(L.readLibraryFilesAdd) { navigator.addBookSource = .choose }
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                    .fixedSize()
            }
        } else {
            PlainEmpty(title: L.readLibraryEmpty(list: ReadWords.list(segment)), line: L.readLibraryEmptyLine) {
                Button(L.homeSearch) { navigator.openReadingSearch() }
                    .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 44, fontSize: Device.isMac ? 13 : 17))
                    .fixedSize()
            }
        }
    }

    private func sorted(_ tiles: [WorkTile]) -> [WorkTile] {
        switch sort {
        case .lastRead: tiles
        case .title: tiles.sorted { $0.title.localizedStandardCompare($1.title) == .orderedAscending }
        }
    }
}

/// The list on screen with its count, and the other lists in its menu.
private struct ReadListMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        let segment = navigator.readingSegment
        Menu {
            Picker(L.shellLibraryListLabel, selection: Binding(
                get: { navigator.readingSegment },
                set: { navigator.readingSegment = $0 }
            )) {
                ForEach(ReadWords.lists, id: \.self) { list in
                    Text(ReadWords.list(list))
                        .badge(count(list))
                        .tag(list)
                }
            }
            .pickerStyle(.inline)
        } label: {
            HStack(spacing: 6) {
                Text(ReadWords.list(segment)).foregroundStyle(.silk)
                Text(String(count(segment))).foregroundStyle(.haze).monospacedDigit() // i18n-skip
                Image(systemName: Device.isMac ? "chevron.up.chevron.down" : "chevron.down")
                    .font(.ui(Device.isMac ? 9 : 12, weight: .semibold))
                    .foregroundStyle(.silk)
                    .accessibilityHidden(true)
            }
            .font(.plat(mac: 13, touch: 20, weight: .semibold))
            #if os(macOS)
            .padding(.horizontal, 12)
            .frame(height: 28)
            .overlay(Capsule().strokeBorder(Palette.glassRule, lineWidth: 1))
            #endif
            .contentShape(.rect)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .pointerHover()
        .accessibilityLabel(L.shellLibraryListLabel)
        .accessibilityValue(L.readLibraryListValue(list: ReadWords.list(segment), n: count(segment)))
    }

    private func count(_ list: LibrarySegmentRead) -> Int {
        store.reading.lists[list]?.count ?? 0
    }
}

/// Covers in as many columns of at least the cover's width as fit, sharing the width.
struct ReadGrid: View {
    let tiles: [WorkTile]
    @State private var width: CGFloat = 0

    var body: some View {
        let (columns, cell) = Self.columns(width)
        LazyVGrid(
            columns: Array(repeating: GridItem(.fixed(cell), spacing: Self.gap, alignment: .top), count: columns),
            alignment: .leading,
            spacing: Device.isMac ? 24 : 20
        ) {
            ForEach(tiles, id: \.work) { tile in
                ReadTileCard(tile: tile, width: cell)
                    .contextMenu { ReadTileMenu(tile: tile) }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { width = $0 }
    }

    #if os(macOS)
    static let minimum: CGFloat = 128
    static let gap: CGFloat = 16
    #else
    static let minimum: CGFloat = 104
    static let gap: CGFloat = 12
    #endif

    static func columns(_ width: CGFloat) -> (Int, CGFloat) {
        guard width > 0 else { return (1, minimum) }
        let count = max(1, Int((width + gap) / (minimum + gap)))
        let cell = ((width - gap * CGFloat(count - 1)) / CGFloat(count)).rounded(.down)
        return (count, cell)
    }
}

/// A cover's menu in the library and on Read.
struct ReadTileMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let tile: WorkTile

    var body: some View {
        Button(L.homeDetails) { navigator.openSeries(tile.work) }
        Menu(L.seriesAddToList) {
            ForEach(ReadWords.statuses, id: \.self) { list in
                Button(ReadWords.list(list)) { store.reading.setStatus(tile.work, list) }
            }
        }
    }
}

private struct ReadGridPlaceholder: View {
    var body: some View {
        let (columns, cell) = ReadGrid.columns(Device.isMac ? 992 : 360)
        LazyVGrid(columns: Array(repeating: GridItem(.fixed(cell), spacing: ReadGrid.gap), count: columns),
                  alignment: .leading, spacing: 24) {
            ForEach(0..<(columns * 2), id: \.self) { _ in
                VStack(alignment: .leading, spacing: 6) {
                    RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: cell, height: cell * 1.5)
                    RoundedRectangle(cornerRadius: 3).fill(.raised).frame(width: cell * 0.8, height: 10)
                    RoundedRectangle(cornerRadius: 3).fill(.raised).frame(width: cell * 0.5, height: 8)
                }
            }
        }
        .accessibilityHidden(true)
    }
}
