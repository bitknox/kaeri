// The reader's stuck states from the round 5 hunt, driven as a person drives them in the Fixtures
// app: the hardware keys on iPad, Go to page from the keyboard, a novel's end page, a strip
// scrolled to its top by the system rather than a finger, and a pinch cut off by the app going
// to the background. Run with the Audit scheme on an iPad Simulator (the keys) or any Simulator
// (the rest).
//
// The keys here are the ones XCUITest delivers on iPadOS: arrows, Space, letters and Return.
// Home, End, Page Up, Page Down and Esc from a hardware keyboard do not come through it, so those
// are checked with a keyboard attached to the Simulator.
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

    private func launch(_ scene: String, _ extra: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-KaeriScene", scene] + extra
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

    /// G opens Go to page with its field taking the keys, the reader's own keys wait while it is
    /// up, and closing it gives the keys back. XCUITest types into the number pad on screen, so
    /// Esc and Return from a keyboard are checked with real key presses on the Simulator instead.
    func testGoToPageFromTheKeyboard() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "hardware keys are an iPad check")
        let app = launch("readermangartl")
        _ = try XCTUnwrap(page(app), "no page on screen")
        // The chapter, from the line under the title: what goes before its " · ".
        let chapter = { (line: String) in line.components(separatedBy: " · ").first ?? line }
        let before = chapter(line(app, containing: " · "))
        XCTAssertFalse(before.isEmpty, "no line under the title")
        key(app, "g")
        let field = app.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5), "Go to page did not open")
        Thread.sleep(forTimeInterval: 0.8)
        XCTAssertEqual(field.value(forKey: "hasKeyboardFocus") as? Bool, true, "the field does not have the keyboard")
        // A reader key with the prompt up goes to the field, not the book.
        key(app, "]")
        key(app, .space)
        XCTAssertTrue(field.exists, "a reader key closed the prompt")
        // What those keys typed goes, and the page goes in its place.
        let typed = (field.value as? String) ?? ""
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: typed.count) + "5")
        app.buttons["Go"].tap()
        XCTAssertTrue(field.waitForNonExistence(timeout: 3), "Go did not close Go to page")
        XCTAssertEqual(page(app), 5, "Go to page did not go")
        // `]` with the prompt up went to the field: the chapter is the one it was.
        XCTAssertEqual(chapter(line(app, containing: " · ")), before, "a reader key changed the chapter under the prompt")
        // Closing it gave the keys back: the arrows turn again.
        key(app, .leftArrow)
        XCTAssertEqual(page(app), 6, "Left after Go to page")
        key(app, "g")
        XCTAssertTrue(field.waitForExistence(timeout: 5), "Go to page did not open a second time")
    }

    /// A novel turns by its keys as the comic does, press after press.
    func testANovelTurnsByKeyRepeatedly() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "hardware keys are an iPad check")
        let app = launch("readernovel")
        Thread.sleep(forTimeInterval: 3)
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

    /// The strip's twin: a pinch the app loses to the background leaves the strip at the scale the
    /// fingers left, and Command-0 fits it back to its column.
    func testCommandZeroFitsAStripAfterAPinchCutOffByTheBackground() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "Command-0 is an iPad check")
        let env = ProcessInfo.processInfo.environment
        let marks = try XCTUnwrap(env["KAERI_TEST_MARKS"].map { URL(filePath: $0) }, "needs the script's hand")
        let app = launch("readerstrip")
        Thread.sleep(forTimeInterval: 3)
        // The Fixtures app says the strip's zoom on an element of its own: the images are none.
        let probe = app.descendants(matching: .any)["strip-zoom"]
        XCTAssertTrue(probe.waitForExistence(timeout: 10), "no strip zoom from the Fixtures app")
        let zoom = { Double(probe.value as? String ?? "") ?? 0 }
        XCTAssertEqual(zoom(), 1, accuracy: 0.01, "the strip did not open fitted")
        try "go".write(to: marks.appending(path: "pinching"), atomically: true, encoding: .utf8)
        app.pinch(withScale: 2.5, velocity: 0.15)
        app.activate()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10))
        Thread.sleep(forTimeInterval: 1)
        let left = zoom()
        try? "\(left)".write(to: marks.appending(path: "strip-zoom-left.txt"), atomically: true, encoding: .utf8)
        XCTAssertGreaterThan(left, 1.05, "the cut-off pinch left no zoom to fit")
        key(app, "0", .command)
        Thread.sleep(forTimeInterval: 1)
        XCTAssertEqual(zoom(), 1, accuracy: 0.01, "Command-0 left the strip magnified")
    }

    /// Zoom In, Zoom Out and Fit to Window zoom a page from the keyboard: a pinched page is fitted
    /// by Command-0, and Command-equals and Command-minus step it about the middle.
    func testTheZoomKeysZoomAPage() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "the keys are an iPad check")
        let app = launch("readermangartl")
        let start = try XCTUnwrap(page(app), "no page on screen")
        let probe = app.descendants(matching: .any)["page-zoom"]
        XCTAssertTrue(probe.waitForExistence(timeout: 10), "no page zoom from the Fixtures app")
        let zoom = { () -> Double in
            Thread.sleep(forTimeInterval: 0.8)
            return Double(probe.value as? String ?? "") ?? 0
        }
        XCTAssertEqual(zoom(), 1, accuracy: 0.01, "the page did not open fitted")
        let shown = app.images.matching(NSPredicate(format: "label BEGINSWITH 'Page '")).allElementsBoundByIndex
        try XCTUnwrap(shown.first(where: \.isHittable), "no page to pinch").pinch(withScale: 2.5, velocity: 1)
        XCTAssertGreaterThan(zoom(), 1.05, "the pinch did not zoom")
        key(app, "0", .command)
        XCTAssertEqual(zoom(), 1, accuracy: 0.01, "Command-0 left the page zoomed")
        key(app, "=", .command)
        XCTAssertEqual(zoom(), 1.25, accuracy: 0.01, "Command-equals did not zoom in")
        key(app, "=", .command)
        XCTAssertEqual(zoom(), 1.56, accuracy: 0.01, "a second Command-equals did not zoom in again")
        key(app, "-", .command)
        XCTAssertEqual(zoom(), 1.25, accuracy: 0.01, "Command-minus did not zoom out")
        key(app, "0", .command)
        XCTAssertEqual(zoom(), 1, accuracy: 0.01, "Command-0 did not fit the page")
        XCTAssertEqual(page(app), start, "zooming moved the place")
        app.swipeRight()
        XCTAssertNotEqual(page(app), start, "the swipe did not turn after the zoom keys")
    }

    /// Command-0 fits a finished pinch in every way of reading: one page, a spread, pages one
    /// under another and the long strip. One test each, so a mode that fails names itself and
    /// the others still run.
    func testCommandZeroFitsAPinchedPage() throws {
        try fitsAfterPinch("readermangartl", probe: "page-zoom")
    }

    func testCommandZeroFitsAPinchedSpread() throws {
        try fitsAfterPinch("readermangartl", probe: "page-zoom", wide: true)
    }

    func testCommandZeroFitsPinchedPagesOneUnderAnother() throws {
        try fitsAfterPinch("readermangartl", ["-KaeriReaderSet", "mode=vertical"], probe: "page-zoom")
    }

    func testCommandZeroFitsAPinchedStrip() throws {
        try fitsAfterPinch("readerstrip", probe: "strip-zoom")
    }

    private func fitsAfterPinch(_ scene: String, _ extra: [String] = [], probe name: String, wide: Bool = false) throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "Command-0 is an iPad check")
        XCUIDevice.shared.orientation = wide ? .landscapeLeft : .portrait
        defer { XCUIDevice.shared.orientation = .portrait }
        let app = launch(scene, extra)
        Thread.sleep(forTimeInterval: 3)
        let probe = app.descendants(matching: .any)[name]
        XCTAssertTrue(probe.waitForExistence(timeout: 10), "no zoom from the Fixtures app")
        let zoom = { () -> Double in
            Thread.sleep(forTimeInterval: 0.8)
            return Double(probe.value as? String ?? "") ?? 0
        }
        XCTAssertEqual(zoom(), 1, accuracy: 0.01, "did not open fitted")
        app.pinch(withScale: 2.5, velocity: 1)
        XCTAssertGreaterThan(zoom(), 1.05, "the pinch did not zoom")
        key(app, "0", .command)
        XCTAssertEqual(zoom(), 1, accuracy: 0.01, "Command-0 left the pinch")
    }

    /// File › Open (Command-O) over a book on iPad: a sheet with a field of its own takes the
    /// arrows and Command-L, and the book behind it does not turn. If no sheet comes up over the
    /// reader's cover, the book keeps its keys.
    func testCommandOOverABookTakesTheArrowsWhenItShows() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "an iPad check")
        let app = launch("readermangartl")
        let start = try XCTUnwrap(page(app), "no page on screen")
        key(app, "o", .command)
        Thread.sleep(forTimeInterval: 1.5)
        let field = app.textFields.firstMatch
        let shown = field.exists && field.isHittable
        print("audit: Command-O over a book shows its sheet: \(shown)")
        if shown {
            field.tap()
            field.typeText("abc")
            key(app, .rightArrow)
            key(app, .rightArrow)
            key(app, "l", .command)
            Thread.sleep(forTimeInterval: 1)
            XCTAssertTrue(field.exists, "Command-L took the sheet away")
            // The book's pages leave the accessibility tree while a sheet is over them, so the
            // page is read once the sheet is cancelled.
            // The reader keeps a go-to-page field of its own, so the sheet's going is told by its
            // own button, not by a text field.
            let choose = app.buttons["Choose a torrent file"].firstMatch
            XCTAssertTrue(choose.exists, "the sheet has no Choose a torrent file")
            app.buttons["Cancel"].firstMatch.tap()
            XCTAssertTrue(choose.waitForNonExistence(timeout: 5), "Cancel left the sheet up")
            // Command-L under the sheet must not have asked for the chapters, to come up once it goes.
            let chapters = app.buttons["Chapters"].firstMatch
            XCTAssertFalse(chapters.waitForExistence(timeout: 2), "Command-L opened the chapters behind the sheet")
            let after = page(app)
            if after == nil {
                print("audit: no page after Cancel; images \(app.images.count), buttons \(app.buttons.allElementsBoundByIndex.prefix(12).map(\.label))")
            }
            XCTAssertEqual(after, start, "the arrows turned the book behind the sheet")
        } else {
            key(app, .leftArrow)
            XCTAssertNotEqual(page(app), start, "with nothing over it the book did not turn")
        }
    }
}
