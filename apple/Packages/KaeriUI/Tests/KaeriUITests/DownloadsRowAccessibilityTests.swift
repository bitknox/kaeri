#if os(macOS)
// A Downloads series row is a button that opens the series, and offers Send to e-reader only once
// a chapter is kept (Mac R8-M11: it read as text with no press action, and Send did nothing with
// no chapter downloaded).

import AppKit
import KaeriKit
@testable import KaeriStore
import KaeriStrings
@testable import KaeriUI
import SwiftUI
import Testing

@MainActor
@Suite(.serialized)
struct DownloadsRowAccessibilityTests {
    private let size = CGSize(width: 600, height: 120)
    private let work = "a:900305"
    private let title = "The Glass Orchard"

    private func series() -> BookDownloadSeries {
        BookDownloadSeries(work: WorkId(work), title: title, chapters: 1, bytes: 5_300_000)
    }

    private func row(_ store: AppStore, _ navigator: Navigator) -> some View {
        BookDownloadSeriesRow(series: series())
            .frame(width: size.width, height: size.height)
            .environment(store)
            .environment(navigator)
    }

    /// The row's element: the one that says the series' title.
    private func element(_ window: TapWindow) throws -> NSObject {
        try #require(window.accessibilityElements().first { element in
            (element.accessibilityLabel() ?? "").contains(title)
        }, "the row is in the tree")
    }

    private func actions(_ element: NSObject) -> [String] {
        (element.accessibilityCustomActions() ?? []).map(\.name)
    }

    @Test func aRowIsAButtonThatOpensItsSeries() async throws {
        let store = AppStore()
        let navigator = Navigator()
        let window = TapWindow(row(store, navigator), size: size)
        defer { window.close() }
        try await window.settle()
        let element = try element(window)
        #expect(element.accessibilityRole() == .button, "the row reads as a button")
        #expect(element.accessibilityPerformPress(), "and VoiceOver can press it")
        try await window.wait(0.5)
        #expect(navigator.seriesOnMac == work, "pressing it opens the series")
    }

    @Test func sendIsOfferedOnlyOnceAChapterIsKept() async throws {
        let store = AppStore()
        let window = TapWindow(row(store, Navigator()), size: size)
        defer { window.close() }
        try await window.settle()
        #expect(!actions(try element(window)).contains(L.sendTitle), "nothing kept, nothing to send")

        let chapter = ChapterRow(id: ChapterId("c1"), label: "Chapter 1", state: .unread)
        store.reading.apply(.downloads(BookDownloadsView(
            rows: [BookDownloadRow(work: WorkId(work), title: title, chapter: chapter, state: .done)],
            meter: StorageMeter(), series: [series()])))
        try await window.wait(0.3)
        #expect(actions(try element(window)).contains(L.sendTitle), "a kept chapter can be sent")
    }
}
#endif
