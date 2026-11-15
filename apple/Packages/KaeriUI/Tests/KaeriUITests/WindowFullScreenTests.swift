// View › Enter Full Screen (⌃⌘F) is on for a window that can fill the screen, browsing, a book or
// the player, and off for one that cannot, such as the trailer's.

#if os(macOS)
import AppKit
@testable import KaeriUI
import Testing

@MainActor
struct WindowFullScreenTests {
    private func window(_ style: NSWindow.StyleMask) -> NSWindow {
        NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: style, backing: .buffered, defer: true)
    }

    @Test func aResizableWindowCanFillTheScreen() {
        #expect(WindowFullScreen.canFill(window([.titled, .closable, .resizable])))
    }

    @Test func aWindowKeptOutOfFullScreenCannot() {
        let trailer = window([.titled, .closable, .resizable])
        trailer.collectionBehavior.insert(.fullScreenNone)
        #expect(!WindowFullScreen.canFill(trailer))
    }

    @Test func aFixedSizeWindowCannot() {
        #expect(!WindowFullScreen.canFill(window([.titled, .closable])))
    }
}
#endif
