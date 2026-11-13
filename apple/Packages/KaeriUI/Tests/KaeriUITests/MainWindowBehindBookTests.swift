// A book open in the main window covers it, and closing the book returns to exactly where it was
// opened. The menu items that change the window under the book are dimmed while it is open.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct MainWindowBehindBookTests {
    @Test func theWindowsOwnItemsAreDimmedWhileABookIsOpen() {
        let store = AppStore()
        store.reader.send = { _ in }
        #expect(!BehindBook.covered(store))
        store.reader.open(work: "a:900301", chapter: "12")
        #expect(BehindBook.covered(store))
        store.reader.close()
        #expect(!BehindBook.covered(store))
    }
}
