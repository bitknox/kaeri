// A series row on Read's Downloads is a button for VoiceOver, opening its series as a tap does
// (Mac R8-M11: VoiceOver read the row as text with no way to open it). The row is shared by every
// platform, and a Simulator's app gives the accessibility tree a test process can't. Run with the
// Audit scheme on an iPhone or iPad Simulator.

import XCTest

@MainActor
final class DownloadsRowButtonTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    func testASeriesRowIsAButton() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-KaeriScene", "readingdownloads", "-KaeriMedium", "read", "-KaeriSection", "downloads"]
        app.launch()
        let title = NSPredicate(format: "label CONTAINS %@", "The Lantern Ferry")
        let any = app.descendants(matching: .any).matching(title).firstMatch
        XCTAssertTrue(any.waitForExistence(timeout: 20), "the series is not on Downloads")
        let button = app.buttons.matching(title).firstMatch
        XCTAssertTrue(button.exists, "the series row is not a button: \(any.elementType.rawValue)")
    }
}
