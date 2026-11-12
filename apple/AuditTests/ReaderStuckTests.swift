// The reader's stuck states from the round 5 hunt, driven as a person drives them in the Fixtures
// app. Run with the Audit scheme on any Simulator, or an iPad Simulator for the keys.

import UIKit
import XCTest

@MainActor
final class ReaderStuckTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func launch(_ scene: String) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-KaeriScene", scene]
        // The script's settings for the app, when it passes any.
        for (name, value) in ProcessInfo.processInfo.environment where name.hasPrefix("KAERI_APP_") {
            app.launchEnvironment["KAERI_" + name.dropFirst("KAERI_APP_".count)] = value
        }
        app.launch()
        return app
    }

    // MARK: Reading the place

    /// The page on screen, from its spoken label "Page 9 of 20, …".
    private func page(_ app: XCUIApplication, settle: Double = 0.9) -> Int? {
        Thread.sleep(forTimeInterval: settle)
        let pages = app.images.matching(NSPredicate(format: "label BEGINSWITH 'Page '"))
        guard pages.firstMatch.waitForExistence(timeout: 5) else { return nil }
        let labels = pages.allElementsBoundByIndex.filter(\.isHittable).map(\.label)
        guard let label = labels.first ?? pages.allElementsBoundByIndex.first?.label else { return nil }
        return Int(label.dropFirst("Page ".count).prefix { $0.isNumber })
    }

    /// The line under the series title, read with the controls up and put away again.
    private func line(_ app: XCUIApplication, containing words: String) -> String {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        let line = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", words)).firstMatch
        guard line.waitForExistence(timeout: 5) else { return "" }
        let text = line.label
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        Thread.sleep(forTimeInterval: 0.6)
        return text
    }

    /// The number after `words` in the line: "image 5 of 8" gives 5 for "image ".
    private func number(after words: String, in line: String) -> Int? {
        guard let range = line.range(of: words) else { return nil }
        return Int(line[range.upperBound...].prefix { $0.isNumber })
    }

    // MARK: A novel's end page

    /// A novel chapter turned through to its end page counts as read there, with "After the last
    /// page", and the end page says so.
    func testANovelChapterCountsAsReadAtItsEndPage() throws {
        let app = launch("readernovel")
        Thread.sleep(forTimeInterval: 3)
        let read = app.staticTexts["Counted as read"]
        let notYet = app.staticTexts["Not marked read yet"]
        var turns = 0
        while !read.exists, !notYet.exists, turns < 80 {
            app.swipeLeft()
            turns += 1
            Thread.sleep(forTimeInterval: 0.5)
        }
        XCTAssertTrue(read.waitForExistence(timeout: 5), "the end page came after \(turns) turns and the chapter did not count as read")
    }

    // MARK: The strip's restore

    /// A resumed strip scrolled to its top by the system, with no finger on it, stays there as
    /// the images above load, and the place is the top.
    func testAStripScrolledToItsTopWithoutAFingerStaysThere() throws {
        let app = launch("readerstrip")
        Thread.sleep(forTimeInterval: 3)
        let resumed = number(after: "image ", in: line(app, containing: " · image "))
        XCTAssertNotNil(resumed)
        XCTAssertGreaterThan(resumed ?? 0, 1, "the strip did not resume down the chapter")
        // The controls up bring the status bar; a tap on it scrolls to the top, as the system does.
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        Thread.sleep(forTimeInterval: 0.8)
        let bar = XCUIApplication(bundleIdentifier: "com.apple.springboard").statusBars.firstMatch
        if bar.exists {
            bar.tap()
        } else {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0)).withOffset(CGVector(dx: 0, dy: 8)).tap()
        }
        // The images above come in as the top shows, and turning the device lays the strip out
        // again: neither may pull the strip back to where it resumed.
        Thread.sleep(forTimeInterval: 2)
        XCUIDevice.shared.orientation = .landscapeLeft
        Thread.sleep(forTimeInterval: 3)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        Thread.sleep(forTimeInterval: 0.6)
        let now = number(after: "image ", in: line(app, containing: " · image "))
        if now != 1, let marks = ProcessInfo.processInfo.environment["KAERI_TEST_MARKS"] {
            try? app.debugDescription.write(to: URL(filePath: marks).appending(path: "strip-tree.txt"), atomically: true, encoding: .utf8)
        }
        XCUIDevice.shared.orientation = .portrait
        XCTAssertEqual(now, 1, "the strip went back to the resumed place")
    }
}
