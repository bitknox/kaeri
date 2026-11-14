// The strip runs on into the next chapter under a finger's drag, as it does under turns: the
// marker crosses the middle of the screen and the next chapter opens in the same scroll. Run
// with the Audit scheme, as the accessibility audit is.

import XCTest

@MainActor
final class StripCrossTests: XCTestCase {
    /// The line under the series title, read with the controls up and put away again.
    private func line(_ app: XCUIApplication) -> String {
        app.tap()
        let line = app.buttons.matching(NSPredicate(format: "label CONTAINS ' · image '")).firstMatch
        guard line.waitForExistence(timeout: 5) else { return "" }
        let words = line.label
        app.tap()
        return words
    }

    func testADragRunsOnIntoTheNextChapter() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-KaeriScene", "readerstrip"]
        app.launch()
        let first = line(app)
        XCTAssertTrue(first.contains("Chapter 2"), "the strip opened at \(first)")
        var now = first
        for _ in 0..<12 where !now.contains("Chapter 3") {
            for _ in 0..<3 { app.swipeUp(velocity: .fast) }
            now = line(app)
        }
        XCTAssertTrue(now.contains("Chapter 3"), "the strip stopped at \(now)")
    }
}
