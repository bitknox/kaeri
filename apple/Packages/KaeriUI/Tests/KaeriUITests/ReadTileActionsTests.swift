// A reading cover's context menu reaches VoiceOver as the cover's custom actions, as Watch's
// covers do. Actions have no submenus, so Add to list's lists come as an action each.

#if os(macOS)
import KaeriKit
import KaeriStore
@testable import KaeriUI
import AppKit
import SwiftUI
import Testing

@MainActor
struct ReadTileActionsTests {
    let store = AppStore()
    let navigator = Navigator()

    let tile = WorkTile(
        work: "a:900302", title: "Lantern Ferry", medium: .manga, cover: nil, // i18n-skip
        formatLabel: "manga", progress: nil, next: nil, newChapter: false, adult: false, // i18n-skip
        facts: [], demo: false, notInSources: false, origin: nil, amount: nil, started: nil)

    /// The names of the custom actions in the accessibility tree of `view`, hosted in a window.
    func actions(_ kind: ReadTileMenu.Kind) -> [String] {
        _ = NSApplication.shared
        let view = Text(tile.title)
            .accessibilityElement(children: .combine)
            .readTileMenu(tile, kind: kind)
            .frame(width: 200, height: 100)
            .environment(store)
            .environment(navigator)
        let window = NSWindow(
            contentRect: NSRect(x: -20_000, y: -20_000, width: 200, height: 100), styleMask: [.borderless],
            backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: view)
        window.alphaValue = 0
        window.orderFrontRegardless()
        defer { window.close() }
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        var names: [String] = []
        func walk(_ node: Any, depth: Int) {
            guard depth < 40, let node = node as? NSObject else { return }
            names += (node.accessibilityCustomActions() ?? []).map(\.name)
            for child in node.accessibilityChildren() ?? [] { walk(child, depth: depth + 1) }
        }
        walk(window.contentView as Any, depth: 0)
        return names
    }

    @Test func aLibraryCoverOffersDetailsAndEveryOtherList() {
        navigator.readingSegment = .reading
        #expect(actions(.lists) == [
            "Details", "Add to Planning", "Add to Completed", "Add to Paused", "Add to Dropped", "Add to Rereading", // i18n-skip
        ])
    }

    @Test func aDiscoveryCoverOffersDetailsAndPlanning() {
        #expect(actions(.plan) == ["Details", "Add to Planning"]) // i18n-skip
    }

    @Test func aCoverOfThePersonsOwnOffersDetailsAlone() {
        #expect(actions(.details) == ["Details"]) // i18n-skip
    }
}
#endif
