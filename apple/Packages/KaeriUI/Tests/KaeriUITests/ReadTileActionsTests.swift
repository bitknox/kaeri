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
        // SwiftUI builds its accessibility tree only for a client; this asks for it in this
        // process, as VoiceOver's presence would.
        NSApplication.shared.accessibilitySetValue(
            true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
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
            // Read by name: the protocol's accessors are optional, and Swift can't pick between
            // their overloads on an element of an unknown class.
            func read(_ key: String) -> Any? {
                guard let node = node as? NSObject, node.responds(to: Selector(key)) else { return nil }
                return node.value(forKey: key)
            }
            guard depth < 40 else { return }
            names += (read("accessibilityCustomActions") as? [NSAccessibilityCustomAction] ?? []).map(\.name)
            for child in read("accessibilityChildren") as? [Any] ?? [] { walk(child, depth: depth + 1) }
        }
        walk(window.contentView as Any, depth: 0)
        // The tree lists them in its own order, not the menu's.
        return names.sorted()
    }

    @Test func aLibraryCoverOffersDetailsAndEveryOtherList() {
        navigator.readingSegment = .reading
        #expect(actions(.lists) == [
            "Add to Completed", "Add to Dropped", "Add to Paused", "Add to Planning", "Add to Rereading", "Details", // i18n-skip
        ])
    }

    @Test func aDiscoveryCoverOffersDetailsAndPlanning() {
        #expect(actions(.plan) == ["Add to Planning", "Details"]) // i18n-skip
    }

    @Test func aCoverOfThePersonsOwnOffersDetailsAlone() {
        #expect(actions(.details) == ["Details"]) // i18n-skip
    }
}
#endif
