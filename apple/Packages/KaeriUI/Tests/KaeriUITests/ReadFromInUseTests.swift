#if os(macOS)
// The Read from sheet's row for the source in use chooses it again rather than only closing
// (Mac R8-M6: after a new source matched, the series page kept naming the old one, and the sheet
// gave no way to put it right).

import AppKit
import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import SwiftUI
import Testing

@MainActor
private final class Sent {
    var commands: [Command] = []

    /// The sources chosen, in order.
    var chosen: [String] {
        commands.compactMap { command in
            if case .reading(.chooseEdition(_, let source, _, _)) = command { return source }
            return nil
        }
    }
}

@MainActor
@Suite(.serialized)
struct ReadFromInUseTests {
    private let size = CGSize(width: 520, height: 520)
    private let work = "a:900306"

    private func store(_ sent: Sent) -> AppStore {
        let store = AppStore()
        store.reading.send = { sent.commands.append($0) }
        store.bookSources.observe(.bookSources(.forWork(
            work: WorkId(work),
            editions: [
                BookEditionRow(sourceId: "harbour-shelf", sourceName: "Harbour Shelf", chapters: 1204, inUse: true),
                BookEditionRow(sourceId: "lantern-pages", sourceName: "Lantern Pages", chapters: 1198),
            ],
            asking: false)))
        return store
    }

    private func press(_ window: TapWindow, _ name: String) throws {
        let row = try #require(window.accessibilityElements().first { element in
            element.accessibilityRole() == .button && TapWindow.text(of: element).contains { $0.contains(name) }
        }, "\(name)'s row is a button")
        #expect(row.accessibilityPerformPress())
    }

    @Test func theRowInUseChoosesItAgain() async throws {
        let sent = Sent()
        let store = store(sent)
        let window = TapWindow(ReadFromSheet(work: work).environment(store).environment(Navigator()), size: size)
        defer { window.close() }
        try await window.settle()
        try press(window, "Harbour Shelf")
        try await window.wait(0.2)
        #expect(sent.chosen == ["harbour-shelf"], "the source in use is chosen again")
    }

    @Test func anotherRowChoosesThatSource() async throws {
        let sent = Sent()
        let store = store(sent)
        let window = TapWindow(ReadFromSheet(work: work).environment(store).environment(Navigator()), size: size)
        defer { window.close() }
        try await window.settle()
        try press(window, "Lantern Pages")
        try await window.wait(0.2)
        #expect(sent.chosen == ["lantern-pages"])
    }
}
#endif
