// A folder picked again while it is already a source is scanned again and stays: Cancel and
// Change never take it out, which would remove the source with its series. A folder added just
// now still goes back out when the sheet is left without Done.

import KaeriKit
@testable import KaeriUI
import Testing

struct FolderCameInTests {
    private func report(alreadyAdded: Bool) -> FolderReport {
        FolderReport(
            id: "f1", name: "Manga", path: "~/Books/Manga", series: 3, books: 12, files: 12, matched: 3,
            unsupported: [], locked: [], listed: [], alreadyAdded: alreadyAdded)
    }

    @Test func aFolderAlreadyAddedIsNeverTakenBack() {
        #expect(!FolderCameInSheet.takesBack(report(alreadyAdded: true)))
    }

    @Test func aFolderAddedJustNowGoesBackOutOnCancel() {
        #expect(FolderCameInSheet.takesBack(report(alreadyAdded: false)))
    }
}
