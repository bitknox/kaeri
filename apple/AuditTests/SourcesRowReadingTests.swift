// In Read only, the iPhone's Settings speak of book sources: the Sources row counts them and its
// page lists them (iPhone R8I-5: the row said "Torrents off" and the page held only the shows'
// settings). The iPhone's Settings are its own, so this runs with the Audit scheme on an iPhone
// Simulator.

import XCTest

@MainActor
final class SourcesRowReadingTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    func testReadingOnlySourcesSpeakOfBooks() throws {
        guard UIDevice.current.userInterfaceIdiom == .phone else { throw XCTSkip("the iPhone's Settings") }
        let app = XCUIApplication()
        app.launchArguments = [
            "-KaeriScene", "readinghome", "-KaeriMedium", "read", "-KaeriSection", "settings", "-AppleLanguages", "(en)",
        ]
        app.launch()
        let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Sources")).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 20), "no Sources row in Settings")
        let torrents = NSPredicate(format: "label CONTAINS[c] %@ OR value CONTAINS[c] %@", "torrent", "torrent")
        let said = "\(row.label) \(String(describing: row.value ?? ""))"
        XCTAssertFalse(said.localizedCaseInsensitiveContains("torrent"), "the row speaks of torrents: \(said)")
        row.tap()
        XCTAssertTrue(app.navigationBars["Your sources"].waitForExistence(timeout: 10), "the page is not the book sources")
        XCTAssertFalse(app.descendants(matching: .any).matching(torrents).firstMatch.exists, "the page holds the torrents")
    }
}
