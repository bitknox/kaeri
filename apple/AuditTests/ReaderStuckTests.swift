// The reader's stuck states from the round 5 hunt, driven as a person drives them in the Fixtures
// app: the hardware keys on iPad, Go to page from the keyboard and by touch, a novel's end page,
// a strip scrolled to its top by the system rather than a finger, and a pinch cut off by the app
// going to the background. Run them with apple/scripts/audit-reader.sh, which gives the Go to page
// touch test the Simulator of its own it needs (see there).
//
// The keys here are the ones XCUITest delivers on iPadOS: arrows, Space and letters. Return,
// Home, End, Page Up, Page Down and Esc from a hardware keyboard do not reach the app through
// it, so those are checked by hand on an iPad with a keyboard. What Return submits in Go to page
// is checked by ReaderGoToPageTests in KaeriStore.
//
// The pinch test needs a hand from outside the Simulator: with TEST_RUNNER_KAERI_TEST_MARKS set
// to a folder, it writes `pinching` there as the pinch starts, and the script running the tests
// brings another app to the front while the fingers are down. Without it the test is skipped.

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

    /// Whether the novel's chapter was laid out within a while. Until its page says it is ready
    /// the line under the title counts one page, and a key pressed then turns nothing. The test's
    /// chapter has more than one page.
    private func opened(_ app: XCUIApplication) -> Bool {
        let line = app.buttons.matching(NSPredicate(format: "label CONTAINS ' · page '")).firstMatch
        let counted = NSPredicate(format: "exists == true AND NOT (label ENDSWITH ' of 1')")
        let laidOut = XCTNSPredicateExpectation(predicate: counted, object: line)
        return XCTWaiter().wait(for: [laidOut], timeout: 30) == .completed
    }

    /// Whether `field` takes the keys within a while. It asks for them as it reaches its window,
    /// a moment after the prompt shows.
    private func focused(_ field: XCUIElement) -> Bool {
        let keys = XCTNSPredicateExpectation(predicate: NSPredicate(format: "hasKeyboardFocus == true"), object: field)
        return XCTWaiter().wait(for: [keys], timeout: 5) == .completed
    }

    /// Whether `field` reads `text` within a while.
    private func reads(_ field: XCUIElement, _ text: String) -> Bool {
        let value = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", text), object: field)
        return XCTWaiter().wait(for: [value], timeout: 5) == .completed
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

    private func key(_ app: XCUIApplication, _ key: XCUIKeyboardKey, _ flags: XCUIElement.KeyModifierFlags = []) {
        app.typeKey(key, modifierFlags: flags)
    }

    private func key(_ app: XCUIApplication, _ key: String, _ flags: XCUIElement.KeyModifierFlags = []) {
        app.typeKey(key, modifierFlags: flags)
    }

    // MARK: Keys on iPad

    /// The reader's keys turn, each press once, and again after the first.
    func testEveryReaderKeyWorksRepeatedly() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "hardware keys are an iPad check")
        let app = launch("readermangartl")
        let start = try XCTUnwrap(page(app), "no page on screen")
        // Right to left: Left goes forward.
        for step in 1...3 {
            key(app, .leftArrow)
            XCTAssertEqual(page(app), start + step, "Left, press \(step)")
        }
        for step in 1...2 {
            key(app, .rightArrow)
            XCTAssertEqual(page(app), start + 3 - step, "Right, press \(step)")
        }
        key(app, .downArrow); XCTAssertEqual(page(app), start + 2, "Down")
        key(app, .downArrow); XCTAssertEqual(page(app), start + 3, "Down again")
        key(app, .upArrow); XCTAssertEqual(page(app), start + 2, "Up")
        key(app, .space); XCTAssertEqual(page(app), start + 3, "Space")
        key(app, .space); XCTAssertEqual(page(app), start + 4, "Space again")
        key(app, .space, .shift); XCTAssertEqual(page(app), start + 3, "Shift-Space")
        // A tap shows the controls and a second puts them away: the keys still go to the reader.
        _ = line(app, containing: " · page ")
        key(app, .leftArrow); XCTAssertEqual(page(app), start + 4, "Left after a tap")
        key(app, .leftArrow); XCTAssertEqual(page(app), start + 5, "Left again after a tap")
    }

    // Go to page is checked twice: from the keyboard, closed by a tap outside it, and by touch,
    // with the number pad's keys and Go. Keep key presses and taps on the keyboard's area in
    // separate tests. On the Simulator a key press in the field swaps the number pad on screen
    // for the hardware keyboard's bar, and the keyboard keeps the pad's area for a while after:
    // Go never reads as hittable and the next tap never reaches the app. Once a key press has
    // reached a Simulator, its number pad stays below the screen in later runs too, so the touch
    // test runs on a Simulator booted fresh for it and settled: the Fixtures app launched once on
    // this scene until it prints KAERI_READY. A first launch on a Simulator that has just booted
    // is slow enough under load that the reader's first page misses `page`'s wait.
    // apple/scripts/audit-reader.sh runs the class this way.

    /// G opens Go to page with its field taking the keys, the reader's own keys wait while it is
    /// up, and closing it gives the keys back.
    func testGoToPageFromTheKeyboard() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "hardware keys are an iPad check")
        let app = launch("readermangartl")
        let start = try XCTUnwrap(page(app), "no page on screen")
        key(app, "g")
        let field = app.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5), "Go to page did not open")
        XCTAssertTrue(focused(field), "the field does not have the keyboard")
        // A reader key with the prompt up goes to the field, not the book.
        key(app, "]")
        key(app, .space)
        XCTAssertTrue(field.exists, "a reader key closed the prompt")
        XCTAssertTrue(((field.value as? String) ?? "").contains("]"), "a reader key did not go to the field")
        // What those keys typed is selected and the page typed in its place.
        key(app, "a", .command)
        key(app, "5")
        XCTAssertTrue(reads(field, "5"), "the field does not read 5")
        // A tap over the book, above the keyboard's area, closes the prompt without going.
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.45)).tap()
        XCTAssertTrue(field.waitForNonExistence(timeout: 3), "a tap outside did not close Go to page")
        XCTAssertEqual(page(app), start, "closing Go to page turned the page")
        // Closing it gave the keys back: the arrows turn again.
        key(app, .leftArrow)
        XCTAssertEqual(page(app), start + 1, "Left after Go to page")
        key(app, "g")
        XCTAssertTrue(field.waitForExistence(timeout: 5), "Go to page did not open a second time")
    }

    /// Go to page by touch alone: the line under the title opens it, the number pad's keys type the
    /// page and Go goes there.
    func testGoToPageByTouch() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "the Go to page checks run on iPad")
        let app = launch("readermangartl")
        _ = try XCTUnwrap(page(app), "no page on screen")
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        let line = app.buttons.matching(NSPredicate(format: "label CONTAINS ' · page '")).firstMatch
        XCTAssertTrue(line.waitForExistence(timeout: 5), "no line under the title")
        line.tap()
        let field = app.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5), "Go to page did not open")
        XCTAssertTrue(focused(field), "the field does not have the keyboard")
        let five = app.keyboards.keys["5"]
        XCTAssertTrue(five.waitForExistence(timeout: 5), "the number pad did not show")
        five.tap()
        XCTAssertTrue(reads(field, "5"), "the field does not read 5")
        app.buttons["Go"].tap()
        XCTAssertTrue(field.waitForNonExistence(timeout: 3), "Go did not close Go to page")
        XCTAssertEqual(page(app), 5, "Go to page did not go")
    }

    /// A novel turns by its keys as the comic does, press after press.
    func testANovelTurnsByKeyRepeatedly() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "hardware keys are an iPad check")
        let app = launch("readernovel")
        XCTAssertTrue(opened(app), "the chapter did not open")
        let first = number(after: "page ", in: line(app, containing: " · page "))
        let start = try XCTUnwrap(first, "no novel line")
        for _ in 0..<3 { key(app, .downArrow); Thread.sleep(forTimeInterval: 0.8) }
        XCTAssertEqual(number(after: "page ", in: line(app, containing: " · page ")), start + 3, "Down three times")
        key(app, .space); Thread.sleep(forTimeInterval: 0.8)
        key(app, .upArrow); Thread.sleep(forTimeInterval: 0.8)
        key(app, .upArrow); Thread.sleep(forTimeInterval: 0.8)
        XCTAssertEqual(number(after: "page ", in: line(app, containing: " · page ")), start + 2, "Space, Up, Up")
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

    // MARK: A pinch cut off

    /// A pinch the app loses to the background leaves swipes turning once the page is fitted.
    func testSwipesTurnAfterAPinchCutOffByTheBackground() throws {
        let env = ProcessInfo.processInfo.environment
        let marks = try XCTUnwrap(env["KAERI_TEST_MARKS"].map { URL(filePath: $0) }, "needs the script's hand")
        let app = launch("readermangartl")
        let start = try XCTUnwrap(page(app), "no page on screen")
        try "go".write(to: marks.appending(path: "pinching"), atomically: true, encoding: .utf8)
        // Slow, so the script has time to bring another app up while the fingers are down.
        // The page on screen: the pages beside it have no place to pinch at.
        let shown = app.images.matching(NSPredicate(format: "label BEGINSWITH 'Page '")).allElementsBoundByIndex
        try XCTUnwrap(shown.first(where: \.isHittable), "no page to pinch").pinch(withScale: 3, velocity: 0.15)
        app.activate()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        Thread.sleep(forTimeInterval: 1)
        // Back to fitted: a turn by a tap at an edge.
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.06, dy: 0.5)).tap()
        let turned = try XCTUnwrap(page(app))
        XCTAssertNotEqual(turned, start, "the tap did not turn")
        app.swipeRight()
        let swiped = try XCTUnwrap(page(app))
        XCTAssertNotEqual(swiped, turned, "the swipe did not turn after the cut-off pinch")
        app.swipeLeft()
        XCTAssertEqual(page(app), turned, "the swipe back did not turn")
        // On iPad, Command-0 fits the page and a turn still goes after it.
        if UIDevice.current.userInterfaceIdiom == .pad {
            key(app, "0", .command)
            app.swipeRight()
            XCTAssertEqual(page(app), swiped, "the swipe did not turn after Command-0")
        }
    }
}
