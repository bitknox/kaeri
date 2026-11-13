// A folder picked again while it is already a source is scanned again and stays: closing the
// sheet, Cancel and Change never take it out, which would remove the source with its series. A
// folder added just now still goes back out when the sheet is left without Add folder.

import KaeriKit
@testable import KaeriUI
import Testing

struct FolderCameInTests {
    private func report(alreadyAdded: Bool) -> FolderReport {
        FolderReport(
            id: "f1", name: "Manga", path: "~/Books/Manga", series: 3, books: 12, files: 12, matched: 3,
            unsupported: [], locked: [], listed: [], alreadyAdded: alreadyAdded)
    }

    private func removed(_ report: FolderReport, kept: Bool) -> [String] {
        var removed: [String] = []
        FolderCameInSheet.leave(report, kept: kept) { removed.append($0) }
        return removed
    }

    @Test func aFolderAlreadyAddedStaysWhenTheSheetIsLeft() {
        // Closed, cancelled, or left for another folder.
        #expect(removed(report(alreadyAdded: true), kept: false).isEmpty)
    }

    @Test func aFolderAddedJustNowGoesBackOutWithoutAddFolder() {
        #expect(removed(report(alreadyAdded: false), kept: false) == ["f1"])
        #expect(removed(report(alreadyAdded: false), kept: true).isEmpty)
    }
}
