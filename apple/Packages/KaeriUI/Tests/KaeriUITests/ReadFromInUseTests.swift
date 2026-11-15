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

extension WindowEventTests {
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

        /// Clicks down (or up) the middle of the sheet until a click chooses a source, the
        /// first row met in that direction.
        private func clickFirstRow(_ window: TapWindow, _ sent: Sent, fromBottom: Bool) async throws {
            let ys = stride(from: 4.0, to: size.height, by: 6.0).map { fromBottom ? size.height - $0 : $0 }
            for y in ys where sent.chosen.isEmpty {
                try await window.click(CGPoint(x: size.width / 2, y: y))
            }
        }

        @Test func theRowInUseChoosesItAgain() async throws {
            let sent = Sent()
            let store = store(sent)
            let window = TapWindow(ReadFromSheet(work: work).environment(store).environment(Navigator()), size: size)
            defer { window.close() }
            try await window.settle()
            // The row in use is first. Without its choice, the click goes on to the next row.
            try await clickFirstRow(window, sent, fromBottom: false)
            #expect(sent.chosen == ["harbour-shelf"], "the source in use is chosen again")
        }

        @Test func anotherRowChoosesThatSource() async throws {
            let sent = Sent()
            let store = store(sent)
            let window = TapWindow(ReadFromSheet(work: work).environment(store).environment(Navigator()), size: size)
            defer { window.close() }
            try await window.settle()
            try await clickFirstRow(window, sent, fromBottom: true)
            #expect(sent.chosen == ["lantern-pages"])
        }
    }
}
#endif
