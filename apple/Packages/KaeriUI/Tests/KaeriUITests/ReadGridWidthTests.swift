// The Mac's reading grid in a window that narrows, as a drag on its edge narrows it: the grid
// takes the width it is given and lays out fewer columns, rather than keeping its old columns
// and pushing the sidebar out of the way.

#if os(macOS)
import KaeriKit
import KaeriStore
@testable import KaeriUI
import AppKit
import SwiftUI
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct ReadGridWidthTests {
    private final class Window: NSWindow {
        override var canBecomeKey: Bool { true }
        override var isKeyWindow: Bool { true }
        override var isVisible: Bool { true }
    }

    @MainActor final class Seen { var width: CGFloat = 0 }

    private func tile(_ n: Int) -> WorkTile {
        WorkTile(
            work: WorkId("a:9003\(10 + n)"), title: "Lantern Street Notes \(n)", medium: .manga, cover: nil, // i18n-skip
            formatLabel: "manga", progress: nil, next: nil, newChapter: false, adult: false, // i18n-skip
            facts: [.reading(.readingKindManga)],
            demo: false, notInSources: false, origin: nil, amount: nil, started: nil)
    }

    private static func turn() { RunLoop.main.run(until: Date().addingTimeInterval(0.01)) }

    private func settle(_ window: NSWindow) async throws {
        for _ in 0..<30 {
            window.layoutIfNeeded()
            window.displayIfNeeded()
            Self.turn()
            try await Task.sleep(for: .milliseconds(1))
        }
    }

    @Test func theGridNarrowsWithTheWindow() async throws {
        let seen = Seen()
        let grid = ReadGrid(tiles: (0..<14).map(tile))
            .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { seen.width = $0 }
            .environment(AppStore())
            .environment(Navigator())
        let window = Window(
            contentRect: NSRect(x: 0, y: 0, width: 1_200, height: 700),
            styleMask: [.titled, .closable, .resizable, .fullSizeContentView],
            backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: ScrollView { grid })
        window.alphaValue = 0
        window.setFrameOrigin(NSPoint(x: -20_000, y: -20_000))
        window.orderFrontRegardless()
        defer { window.close() }
        try await settle(window)
        #expect(seen.width > 1_100)

        // In 16 pt steps down to 700, as a drag goes.
        var width: CGFloat = 1_200
        while width > 700 {
            width = max(700, width - 16)
            window.setContentSize(NSSize(width: width, height: 700))
            try await settle(window)
        }
        #expect(seen.width <= 700, "the grid kept \(seen.width) pt in a 700 pt window")
        #expect(ReadGrid.columns(seen.width).0 <= 4)
    }
}
#endif
