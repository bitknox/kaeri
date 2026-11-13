// The reader's Go menu and its keys step aside for anything of the app's in front of the book.
// A sheet or popover over the book is the app's unless the reader opened it: File › Open (⌘O),
// Join Plugin Store and Open Invite Link each come up as a sheet with a field of its own, which
// takes the arrows and ⌘L. The command palette (⌘K), Settings (⌘,) and Messages (⌥⌘L) are dimmed
// while a book is open, so they never come up over it (`MainWindowBehindBookTests`).

#if os(macOS)
import AppKit
@testable import KaeriUI
import Testing

@MainActor
struct ReaderKeysStepAsideTests {
    private func window() -> NSWindow {
        NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                 styleMask: [.titled, .resizable], backing: .buffered, defer: true)
    }

    /// The things of the app's that come up over a book, each a window on the reader's.
    static let appSheets = ["Open link or file", "Join Plugin Store", "Open Invite Link"]

    @Test(arguments: appSheets)
    func anAppSheetOverTheBookTakesTheKeys(_ sheet: String) {
        let reader = window()
        let over = window()
        reader.addChildWindow(over, ordered: .above)
        defer { reader.removeChildWindow(over) }
        #expect(!WindowFront.inFront(key: over, reader: reader, readerOwns: false), "\(sheet)")
    }

    @Test func theReadersOwnSheetKeepsItsModifiedKeys() {
        let reader = window()
        let settings = window()
        reader.addChildWindow(settings, ordered: .above)
        defer { reader.removeChildWindow(settings) }
        #expect(WindowFront.inFront(key: settings, reader: reader, readerOwns: true))
    }

    @Test func theReadersWindowItselfHasTheKeys() {
        let reader = window()
        #expect(WindowFront.inFront(key: reader, reader: reader, readerOwns: false))
        #expect(WindowFront.inFront(key: nil, reader: reader, readerOwns: false))
    }

    @Test func anotherWindowHasThemNot() {
        #expect(!WindowFront.inFront(key: window(), reader: window(), readerOwns: false))
    }
}
#endif
