// Search on the Mac, laid out and drawn in a window as the app's is: the filters panel pushes
// what Search shows aside, at its start as with results, and an overdue search shows its
// placeholders at once. The covers are a solid amber-brown, which nothing else on the screen is,
// so the drawn window says where covers are.

#if os(macOS)
import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import AppKit
import SwiftUI
import Testing

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct SearchLayoutTests {
    /// A window that lays out and draws as an on-screen one does, out of sight.
    private final class Window: NSWindow {
        override var canBecomeKey: Bool { true }
        override var isKeyWindow: Bool { true }
        override var isVisible: Bool { true }
    }

    private static let size = CGSize(width: 1_200, height: 800)
    /// Where the open panel's leading edge falls, in the window's points.
    private static var panelEdge: CGFloat { size.width - 340 }

    private static func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func turn(_ seconds: Double = 0.02) async throws {
        Self.spin(seconds)
        try await Task.sleep(for: .milliseconds(1))
    }

    private func settle(_ window: NSWindow, turns: Int = 40) async throws {
        for _ in 0..<turns {
            window.layoutIfNeeded()
            window.displayIfNeeded()
            try await turn()
        }
    }

    private func titles(from first: Int, count: Int) -> [Entry] {
        (0..<count).map { n in
            entry {
                $0.id = AnilistId(900_100 + first + n)
                $0.title = "Lantern Hollow \(first + n)"
                $0.coverColor = "#cc8844"
            }
        }
    }

    private func window(_ store: AppStore, _ navigator: Navigator) -> Window {
        let window = Window(
            contentRect: NSRect(origin: .zero, size: Self.size),
            styleMask: [.titled, .closable, .resizable, .fullSizeContentView],
            backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: RootView().environment(store).environment(navigator))
        window.alphaValue = 0
        window.setFrameOrigin(NSPoint(x: -20_000, y: -20_000))
        window.orderFrontRegardless()
        return window
    }

    /// The window as drawn now.
    private func drawn(_ window: NSWindow) throws -> NSBitmapImageRep {
        let view = try #require(window.contentView)
        window.layoutIfNeeded()
        window.displayIfNeeded()
        let image = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: image)
        return image
    }

    /// The points in `xs` by `ys` (window points from the top left) where a cover is drawn.
    private func covers(in image: NSBitmapImageRep, xs: StrideThrough<CGFloat>, ys: StrideThrough<CGFloat>) -> [CGPoint] {
        let scale = CGFloat(image.pixelsWide) / Self.size.width
        var found: [CGPoint] = []
        for x in xs {
            for y in ys {
                guard let colour = image.colorAt(x: Int(x * scale), y: Int(y * scale))?.usingColorSpace(.sRGB)
                else { continue }
                // Amber-brown: red well over blue. The ground, the raised plates and the panel
                // are all blue-grey.
                if colour.redComponent - colour.blueComponent > 0.12, colour.redComponent > 0.25 {
                    found.append(CGPoint(x: x, y: y))
                }
            }
        }
        return found
    }

    /// Search's start, with two rails of covers.
    private func startShowing(_ store: AppStore) {
        store.apply(.browse([
            BrowseRail(title: "Trending now", detail: nil, query: SearchQuery(sort: .trending),
                       entries: titles(from: 0, count: 12)),
            BrowseRail(title: "Top rated", detail: nil, query: SearchQuery(sort: .trending),
                       entries: titles(from: 20, count: 12)),
        ]))
    }

    @Test func theFiltersPanelPushesTheStartsCoversAsItPushesTheResults() async throws {
        _ = NSApplication.shared
        let store = AppStore()
        startShowing(store)
        let navigator = Navigator()
        navigator.section = .search
        let window = window(store, navigator)
        defer { window.close() }
        try await settle(window)
        // The rails run on to the window's edge with the panel shut.
        let edge = Self.panelEdge
        let strip = stride(from: edge - 14, through: edge - 2, by: 4)
        let rows = stride(from: CGFloat(150), through: Self.size.height - 10, by: 8)
        #expect(!covers(in: try drawn(window), xs: stride(from: edge - 14, through: Self.size.width - 40, by: 8), ys: rows).isEmpty)

        // Open, the panel sits beside the start's covers, with the margin between: none runs on
        // under it.
        navigator.searchFiltersShown = true
        try await settle(window)
        #expect(store.search.atStart)
        let start = covers(in: try drawn(window), xs: strip, ys: rows)
        #expect(start.isEmpty, "covers at the panel's edge at Search's start: \(start.prefix(4))")
        // The rails still run to that margin, cutting their last cover there: every rail has
        // a cover in the last 24 points before it, wider than the gap between two covers.
        let reach = try railEnds(in: try drawn(window))
        #expect(reach.count == 2, "the rails' rows: \(reach)")
        for end in reach {
            #expect(abs(end - (edge - 16)) <= 1.5, "a rail ends at \(end), the margin before the panel at \(edge - 16)")
        }

        // The same with results.
        let mecha = SearchQuery(genres: ["Mecha"])
        store.searchLive(mecha)
        store.apply(.content(view: .search(query: mecha), content: .entries(titles(from: 40, count: 20))))
        try await settle(window, turns: 60)
        #expect(!store.search.atStart)
        let results = covers(in: try drawn(window), xs: strip, ys: rows)
        #expect(results.isEmpty, "covers at the panel's edge with results: \(results.prefix(4))")
    }

    @Test func anOverdueSearchShowsItsPlaceholdersAtOnce() async throws {
        _ = NSApplication.shared
        let store = AppStore()
        let navigator = Navigator()
        navigator.section = .search
        let mecha = SearchQuery(genres: ["Mecha"])
        store.searchLive(mecha)
        store.apply(.content(view: .search(query: mecha), content: .entries(titles(from: 0, count: 20))))
        let window = window(store, navigator)
        defer { window.close() }
        try await settle(window)
        let content = stride(from: CGFloat(240), through: Self.size.width - 20, by: 10)
        let rows = stride(from: CGFloat(150), through: Self.size.height - 10, by: 10)
        #expect(!covers(in: try drawn(window), xs: content, ys: rows).isEmpty, "the results")

        // Another search, unanswered. Under 600 ms the results stay; once it is overdue, the
        // placeholders take their place without a second wait of their own.
        store.searchLive(SearchQuery(genres: ["Mecha", "Drama"]))
        let end = Date().addingTimeInterval(30)
        while !store.search.overdue {
            try #require(Date() < end, "waited for the search to be overdue")
            window.layoutIfNeeded()
            window.displayIfNeeded()
            try await turn()
        }
        // The placeholders' crossfade, and no more: the list's own 600 ms would still show the
        // results before them.
        let since = Date()
        while Date().timeIntervalSince(since) < 0.45 {
            window.layoutIfNeeded()
            window.displayIfNeeded()
            try await turn(0.01)
        }
        let left = covers(in: try drawn(window), xs: content, ys: rows)
        #expect(left.isEmpty, "the earlier results still showing past 600 ms: \(left.prefix(4))")
    }

    /// Where each row of covers ends, in window points: the last cover colour on a line through
    /// each rail's covers, or each row of the grid's, top to bottom. Only the column's end is
    /// looked at, clear of the sidebar's accent.
    private func railEnds(in image: NSBitmapImageRep) throws -> [CGFloat] {
        let xs = stride(from: Self.panelEdge - 300, through: Self.size.width - 1, by: 1)
        var ends: [CGFloat] = []
        var y: CGFloat = 150
        while y < Self.size.height - 10 {
            if !covers(in: image, xs: xs, ys: stride(from: y, through: y, by: 1)).isEmpty {
                // A little below the covers' tops, where the rounded corners no longer narrow them.
                let lines = stride(from: y, through: y + 40, by: 4).compactMap {
                    covers(in: image, xs: xs, ys: stride(from: $0, through: $0, by: 1)).last?.x
                }
                ends.append(try #require(lines.max()) + 1)
                // Past this row's covers and its captions, to the next.
                while y < Self.size.height - 10,
                      !covers(in: image, xs: xs, ys: stride(from: y, through: y, by: 1)).isEmpty {
                    y += 2
                }
                y += 40
            } else {
                y += 2
            }
        }
        return ends
    }

    @Test func theCoversKeepTheirSizeAsThePanelOpens() async throws {
        _ = NSApplication.shared
        let store = AppStore()
        startShowing(store)
        let navigator = Navigator()
        navigator.section = .search
        let window = window(store, navigator)
        defer { window.close() }
        try await settle(window)
        let shut = try #require(firstCoverWidth(try drawn(window)))
        // Open, the start's covers are as wide as they were: fewer of them, none larger.
        navigator.searchFiltersShown = true
        try await settle(window)
        let open = try #require(firstCoverWidth(try drawn(window)))
        #expect(abs(open - shut) <= 1, "a cover \(shut) points wide with the panel shut, \(open) open")

        // The results fill the column to the margin before the panel, their columns flexing a
        // little at most from the covers' width with it shut.
        let mecha = SearchQuery(genres: ["Mecha"])
        store.searchLive(mecha)
        store.apply(.content(view: .search(query: mecha), content: .entries(titles(from: 40, count: 20))))
        try await settle(window, turns: 60)
        let image = try drawn(window)
        let results = try #require(firstCoverWidth(image))
        #expect(abs(results / shut - 1) <= SearchGridColumns.flex + 0.01, "a result \(results) points wide, a rail's cover \(shut)")
        let end = try #require(try railEnds(in: image).first)
        #expect(abs(end - (Self.panelEdge - 16)) <= 1.5, "the results end at \(end), the margin before the panel at \(Self.panelEdge - 16)")
        navigator.searchFiltersShown = false
        try await settle(window)
        let resultsShut = try #require(firstCoverWidth(try drawn(window)))
        #expect(abs(resultsShut - shut) <= 1, "a result \(resultsShut) points wide with the panel shut")
    }

    /// How wide the first cover drawn is, in points: its widest run of cover colour, on the rows
    /// a little below its top, where the rounded corners no longer narrow it. Runs narrower than a
    /// cover could be, such as an accent mark, are passed over.
    private func firstCoverWidth(_ image: NSBitmapImageRep) -> CGFloat? {
        func firstRun(at y: CGFloat) -> CGFloat? {
            let hits = covers(in: image, xs: stride(from: 0, through: Self.size.width - 1, by: 1), ys: stride(from: y, through: y, by: 1))
            var runs: [(start: CGFloat, end: CGFloat)] = []
            for point in hits {
                if let last = runs.last, point.x == last.end + 1 {
                    runs[runs.count - 1].end = point.x
                } else {
                    runs.append((point.x, point.x))
                }
            }
            return runs.first(where: { $0.end - $0.start + 1 >= 60 }).map { $0.end - $0.start + 1 }
        }
        for y in stride(from: CGFloat(150), through: Self.size.height - 50, by: 4) where firstRun(at: y) != nil {
            return stride(from: y, through: y + 40, by: 4).compactMap(firstRun).max()
        }
        return nil
    }

    @Test func theResultsFillTheColumnNearTheCoversWidth() {
        // A width that fits whole covers keeps them exactly: the panel shut.
        let shut = SearchGridColumns.columns(cover: RailCover.width(in: 1_000), in: 1_000)
        #expect(shut == GridColumns(count: 6, width: 150, spacing: 20))
        // The count whose columns come nearest the cover: six of 135 across 910, not five of 166.
        #expect(SearchGridColumns.columns(cover: 140, in: 910) == GridColumns(count: 6, width: 135, spacing: 20))
        // Neither count within the flex: the columns keep to it and the gaps widen.
        let wide = SearchGridColumns.columns(cover: 150, in: 372)
        #expect(wide.count == 2 && wide.width == 162 && wide.spacing == 48)
        // Across any width at least a cover wide, the columns fill it edge to edge, within the
        // flex of the cover's width, the gaps never narrower than asked.
        for cover in [128.0, 141.5, 150, 170] as [CGFloat] {
            for width in stride(from: cover, through: 2_400, by: 7) {
                let columns = SearchGridColumns.columns(cover: cover, in: width)
                let filled = CGFloat(columns.count) * columns.width + CGFloat(columns.count - 1) * columns.spacing
                #expect(abs(filled - width) < 0.01 || columns.count == 1, "\(columns) across \(width)")
                #expect(abs(columns.width / cover - 1) <= SearchGridColumns.flex + 0.0001, "\(columns) for a cover of \(cover)")
                #expect(columns.spacing >= 20 - 0.0001, "\(columns) across \(width)")
            }
        }
    }

    @Test func aRailsCoversAreTheGridsColumns() {
        // As many columns of at least 128 points as fit with 20 between them, sharing the width.
        #expect(RailCover.width(in: 1_000) == 150)
        #expect(RailCover.width(in: 128) == 128)
        #expect(RailCover.width(in: 100) == 100)
        let narrow = RailCover.width(in: 604)
        #expect(narrow >= 128 && narrow * 4 + 60 == 604)
    }
}
#endif
