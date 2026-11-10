// Reading's discovery: Home's rows from AniList with View all, the wait every row and source row
// shares, and the page a row's View all pushes on the iPhone.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

// MARK: - The wait

/// A slot that waits by the States rules: its placeholders hold the slot's place unseen for
/// 600 ms, then show, and are held at least 0.3 s. The content fades in over them in 200 ms and
/// they fade out under it once it is whole, so the slot never shows empty between the two. Where
/// the content is smaller, as a failure is, the slot then closes to it on the standard spring.
struct ReadWaitSlot<Content: View, Placeholder: View>: View {
    let waiting: Bool
    @ViewBuilder let content: () -> Content
    @ViewBuilder let placeholder: () -> Placeholder

    /// Whether the content is in: false while waiting and through the placeholders' hold.
    @State private var landed: Bool
    @State private var placeholders = 0.0
    @State private var shownAt: ContinuousClock.Instant?

    init(waiting: Bool, @ViewBuilder content: @escaping () -> Content,
         @ViewBuilder placeholder: @escaping () -> Placeholder) {
        self.waiting = waiting
        self.content = content
        self.placeholder = placeholder
        _landed = State(initialValue: !waiting)
    }

    private static var hold: Duration { .milliseconds(300) }

    var body: some View {
        ZStack(alignment: .topLeading) {
            if !landed || placeholders > 0 {
                placeholder()
                    .redacted(reason: .placeholder)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
                    .opacity(placeholders)
            }
            if landed {
                content().transition(.opacity)
            }
        }
        .task(id: waiting) {
            if waiting {
                landed = false
                try? await Task.sleep(for: JobsModel.delay)
                guard !Task.isCancelled else { return }
                shownAt = .now
                withAnimation(.fadeIn) { placeholders = 1 }
                return
            }
            guard !landed else { return }
            if placeholders > 0, let shownAt {
                let left = Self.hold - (ContinuousClock.now - shownAt)
                if left > .zero { try? await Task.sleep(for: left) }
                guard !Task.isCancelled else { return }
            }
            withAnimation(.fadeIn) { landed = true }
            guard placeholders > 0 else { return }
            try? await Task.sleep(for: .milliseconds(200))
            withAnimation(.standard) { placeholders = 0 }
        }
    }
}

/// A row of covers as it will stand: the covers at their size and the title's two lines and the
/// line under them, redacted by the slot.
struct ReadCoverRowPlaceholder: View {
    @Environment(\.homeLayout) private var layout
    var width: CGFloat?
    var count = 8
    var spacing: CGFloat?

    var body: some View {
        let width = width ?? layout.poster.width
        // Held in a scroll view that doesn't scroll, so a row longer than the column is cut at
        // its edge instead of widening the page.
        ScrollView(.horizontal) {
            HStack(alignment: .top, spacing: spacing ?? layout.cardGap) {
                ForEach(0..<count, id: \.self) { _ in
                    VStack(alignment: .leading, spacing: 0) {
                        RoundedRectangle(cornerRadius: ArtShape.coverRadius(width: width))
                            .fill(.raised)
                            .frame(width: width, height: (width * 3 / 2).rounded())
                        Text("Lantern Street Notes") // i18n-skip
                            .font(.showTitle(size: layout.posterTitle.size, relativeTo: .body))
                            .fixedLineHeight(layout.posterTitle.line)
                            .lineLimit(2, reservesSpace: true)
                            .padding(.top, 8)
                        Text("Japan · Releasing") // i18n-skip
                            .font(.ui(layout.posterLine.size))
                            .fixedLineHeight(layout.posterLine.line, relativeTo: .footnote)
                            .lineLimit(1)
                            .padding(.top, 2)
                    }
                    .frame(width: width, alignment: .leading)
                }
            }
        }
        .scrollDisabled(true)
        .scrollIndicators(.never)
        // To the screen's edges on a phone, as the shelf it stands in for runs.
        .contentMargins(.horizontal, layout.wide ? 0 : layout.margin, for: .scrollContent)
        .padding(.horizontal, layout.wide ? 0 : -layout.margin)
        .foregroundStyle(.haze)
        .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
    }
}

// MARK: - Home's rows

/// One of Home's discovery rows: its heading with the side line and View all, then its covers,
/// or placeholders at their size while its part is on its way.
struct ReadHomeRowView: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeLayout) private var layout
    let row: ReadHomeRow

    private var caption: ReadCaption { row.caption }

    var body: some View {
        let title = row.title.text
        let waiting = row.pending && row.tiles.isEmpty
        VStack(alignment: .leading, spacing: 12) {
            ReadShelfHeader(
                title: title, side: row.side?.text, link: row.viewAll == nil ? nil : L.homeViewAll,
                spoken: waiting ? L.readHomeRowLoading(row: title) : nil
            ) {
                guard let query = row.viewAll else { return }
                navigator.openReadViewAll(query, title: title, line: row.viewAllTitle?.text, wide: layout.wide, store: store)
            }
            ReadWaitSlot(waiting: waiting) {
                HomeShelf(spacing: layout.cardGap) {
                    ForEach(row.tiles, id: \.work) { tile in
                        ReadTileCard(tile: tile, twoLines: true, planAction: true, caption: caption)
                            .contextMenu { ReadTileMenu(tile: tile, kind: .plan) }
                            .transition(.opacity)
                    }
                }
                .animation(.standard, value: row.tiles.map(\.work))
            } placeholder: {
                ReadCoverRowPlaceholder()
            }
        }
    }
}

extension ReadHomeRow {
    /// A row of one format says the origin then the status, a mixed row the format then the
    /// status, and a row that fixes the status (Finished) the count in its place. Home's rows and
    /// Search's rails both caption their covers so.
    var caption: ReadCaption {
        let fixed = kind == .finished || viewAll?.status != nil
        return format == nil ? .mixedRow(statusFixed: fixed) : .formatRow(statusFixed: fixed)
    }
}

// MARK: - The largest text

/// A cover as a row, for a grid at the largest text sizes: the small cover, then the title and
/// its lines at full width, wrapping as far as they need.
struct ReadTileListRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let tile: WorkTile
    var note: String?
    var planAction = false
    var caption = ReadCaption.plain

    var body: some View {
        let line = ReadWords.line(tile, caption)
        Button {
            navigator.openSeries(tile.work)
        } label: {
            HStack(alignment: .top, spacing: 12) {
                BookCover(url: tile.cover, width: 56, radius: 4, title: tile.title)
                VStack(alignment: .leading, spacing: 4) {
                    Text(tile.title)
                        .font(.showTitle(size: 17, relativeTo: .body))
                        .foregroundStyle(.silk)
                    if let line {
                        Text(line).font(.ui(15)).foregroundStyle(.haze)
                    }
                    if let note {
                        Text(note).font(.ui(15, weight: .semibold)).foregroundStyle(.silk)
                    }
                }
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(.vertical, 10)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(tile.title))
        .accessibilityValue(Text([ReadWords.spoken(tile, caption), note].compactMap { $0 }.joined(separator: ", "))) // i18n-skip
        .accessibilityAddTraits(.isButton)
        .accessibilityActions {
            if planAction {
                Button(L.libraryAddPlanning) { store.reading.setStatus(tile.work, .planning) }
            }
        }
    }
}

// MARK: - View all on the iPhone

/// A row's View all on the iPhone: its results pushed onto Home under the row's name, three
/// across, with the filters behind the filter button.
struct ReadViewAllPage: View {
    @Environment(AppStore.self) private var store
    let page: ReadViewAll
    @State private var filtersShown = false

    var body: some View {
        let search = store.reading.search
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                if let failure = search.failure, search.tiles.isEmpty {
                    PlainEmpty(title: L.readSearchFailed, line: failure.words) {
                        Button(L.actionTryAgain) { store.reading.runSearch(search.query) }
                            .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 17))
                            .fixedSize()
                    }
                } else if search.tiles.isEmpty, search.loading {
                    HomeWait(active: true, activity: "loading view all") { // i18n-skip
                        ReadCoverRowPlaceholder(width: ReadGrid.columns(360).1, count: 3, spacing: ReadGrid.gap)
                    }
                } else if search.tiles.isEmpty {
                    PlainEmpty(title: L.readSearchNothingBrowse, line: L.readSearchNothingFiltersLine)
                } else {
                    Text(ReadFilterWords.browseLine(
                        search.query, filters: store.reading.filters, total: search.total,
                        line: search.query == page.query ? page.line : nil))
                        .font(.ui(13))
                        .foregroundStyle(.haze)
                    // The row's own caption, so the first covers read as they did on Home.
                    ReadGrid(tiles: search.tiles, twoLines: true,
                             caption: .list(search.query, viewAll: search.query == page.query))
                    if search.hasMore {
                        HStack { if search.loading { ProgressView().controlSize(.small) } }
                            .frame(maxWidth: .infinity, minHeight: 32)
                            .onAppear { store.reading.searchMore() }
                    }
                }
            }
            .padding(EdgeInsets(top: 8, leading: 16, bottom: 24, trailing: 16))
            .frame(maxWidth: .infinity, alignment: .leading)
            .animation(.standard, value: search.tiles.isEmpty)
        }
        .scrollIndicators(.never)
        .background(.ground)
        .navigationTitle(page.title)
        #if os(iOS)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                ReadFilterButton { filtersShown = true }
            }
        }
        .sheet(isPresented: $filtersShown) {
            ReadFiltersPanel(compact: true) { filtersShown = false }
                .presentationDetents([.medium, .large])
                .presentationDragIndicator(.visible)
        }
        #endif
    }
}

/// The filter button in View all's bar on the iPhone, with a dot while a filter is on.
struct ReadFilterButton: View {
    @Environment(AppStore.self) private var store
    let action: () -> Void

    var body: some View {
        let query = store.reading.search.query
        let on = ReadFilterWords.any(query) || query.sourcesOnly
        Button(action: action) {
            Image(systemName: "line.3.horizontal.decrease")
                .overlay(alignment: .topTrailing) {
                    if on {
                        Circle().fill(.silk).frame(width: 7, height: 7).offset(x: 4, y: -3)
                            .accessibilityHidden(true)
                    }
                }
        }
        .accessibilityLabel(L.readFilterTitle)
        .accessibilityValue(on ? ReadFilterWords.summary(query, filters: store.reading.filters) : L.readFilterAny)
    }
}
