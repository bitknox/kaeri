// A book open in the main window covers it, and closing the book returns to exactly where it was
// opened. The menu items that change the window under the book are dimmed while it is open, and
// the menu bar takes a change of the reader's keys without waiting for a key press.

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

    #if os(macOS)
    @MainActor final class Count {
        var value = 0
    }

    /// A Coordinator that refreshes at once and counts how often.
    private func coordinator() -> (MenuKeys.Coordinator, () -> Int) {
        let coordinator = MenuKeys.Coordinator()
        let count = Count()
        coordinator.refresh = { count.value += 1 }
        coordinator.later = { work in work() }
        return (coordinator, { count.value })
    }

    @Test func aClosedBookDimsTheGoMenuWithNoKeyPressed() {
        let (coordinator, refreshed) = coordinator()
        let reading = MenuKeys.Keys(reading: true, reader: nil)
        let closed = MenuKeys.Keys(reading: false, reader: nil)
        coordinator.follow(reading)
        #expect(refreshed() == 1)
        coordinator.follow(closed)
        #expect(refreshed() == 2, "the menus follow the book closing without a key")
        coordinator.follow(closed)
        coordinator.catchUp()
        #expect(refreshed() == 2, "keys the menus already have bring nothing")
    }
    #endif
}
