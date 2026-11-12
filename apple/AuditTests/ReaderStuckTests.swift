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
}
