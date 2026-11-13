// The iPad layout bugs of the round 5 hunt, checked in the Fixtures app: the Chapters popover in
// landscape and in a narrow window, the opening screen's chapter, the series header at the
// largest text size, a failed strip image's words, Choose chapters' Download in a narrow window,
// and Downloads and Sources keeping their place when the window is made narrow. Run with the
// Audit scheme on an iPad Simulator.
//
// With TEST_RUNNER_KAERI_TEST_MARKS set to a folder, each test leaves a screenshot there and a
// line of the frames it measured, for before and after shots.
//
// The narrow window needs windowed apps on in the Simulator (SpringBoard's
// SBChamoisWindowingEnabled); the test drags the window's corner through SpringBoard and is
// skipped when the window does not get narrow.

import UIKit
import XCTest

@MainActor
final class ReaderLayoutTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = true
        XCUIDevice.shared.orientation = .portrait
    }

    override func tearDown() {
        XCUIDevice.shared.orientation = .portrait
    }

    private var marks: URL? {
        ProcessInfo.processInfo.environment["KAERI_TEST_MARKS"].map { URL(filePath: $0) }
    }

    /// A screenshot and a line in `frames.txt`, when the script asks for them.
    private func mark(_ name: String, _ line: String) {
        guard let marks else { return }
        try? XCUIScreen.main.screenshot().pngRepresentation.write(to: marks.appending(path: "\(name).png"))
        let file = marks.appending(path: "frames.txt")
        let text = "\(name): \(line)\n"
        if let handle = try? FileHandle(forWritingTo: file) {
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: Data(text.utf8))
            try? handle.close()
        } else {
            try? text.write(to: file, atomically: true, encoding: .utf8)
        }
    }

    private func launch(_ arguments: [String]) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = arguments
        app.launch()
        return app
    }

    private func requirePad() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "an iPad check")
    }

    /// The Pages and Chapters switch of the open Chapters panel.
    private func pagesSegment(_ app: XCUIApplication) -> XCUIElement {
        app.buttons["Pages"].firstMatch
    }

    /// Opens Chapters with the reader's button, whose label is the same as the switch's second
    /// half: the button is the one outside the panel, so it is the only one before it opens.
    private func openChapters(_ app: XCUIApplication) {
        let button = app.buttons["Chapters"].firstMatch
        XCTAssertTrue(button.waitForExistence(timeout: 20), "no Chapters button")
        button.tap()
    }

    private func closeChapters(_ app: XCUIApplication) {
        app.typeKey(XCUIKeyboardKey.escape, modifierFlags: [])
        if pagesSegment(app).exists {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.08, dy: 0.5)).tap()
        }
        _ = pagesSegment(app).waitForNonExistence(timeout: 5)
    }

    /// The switch sits inside the window and under the status bar's strip.
    private func assertSwitchInside(_ app: XCUIApplication, _ name: String) {
        let segment = pagesSegment(app)
        XCTAssertTrue(segment.waitForExistence(timeout: 10), "\(name): the Chapters panel did not open")
        Thread.sleep(forTimeInterval: 1)
        let window = app.windows.firstMatch.frame
        let pages = segment.frame
        let chapters = app.buttons.matching(NSPredicate(format: "label == 'Chapters'")).allElementsBoundByIndex
            .map(\.frame).filter { abs($0.midY - pages.midY) < 4 }.first ?? .null
        mark(name, "window \(window) pages \(pages) chapters \(chapters)")
        // The status bar is 24 pt on iPad; nothing of the panel goes under it.
        XCTAssertGreaterThanOrEqual(pages.minY, window.minY + 24, "\(name): the switch runs past the top, \(pages)")
        XCTAssertGreaterThanOrEqual(pages.minX, window.minX, "\(name): the switch is cut on the left, \(pages)")
        if !chapters.isNull {
            XCTAssertLessThanOrEqual(chapters.maxX, window.maxX, "\(name): the switch is cut on the right, \(chapters)")
        }
    }

    // MARK: P5, Chapters in landscape

    func testChaptersFitTheScreenInLandscape() throws {
        try requirePad()
        XCUIDevice.shared.orientation = .landscapeLeft
        let app = launch(["-KaeriScene", "readermangartl", "-KaeriReader", "chrome"])
        openChapters(app)
        assertSwitchInside(app, "p5-landscape-button")
        closeChapters(app)
        // The keyboard's way in, as the hunt opened it.
        app.typeKey("l", modifierFlags: .command)
        assertSwitchInside(app, "p5-landscape-cmd-l")
    }

    // MARK: P6, Chapters in a narrow window

    func testChaptersFitANarrowWindow() throws {
        try requirePad()
        let app = launch(["-KaeriScene", "readermangartl", "-KaeriReader", "chrome"])
        XCTAssertTrue(app.buttons["Chapters"].firstMatch.waitForExistence(timeout: 20))
        narrow(app, to: 375)
        let width = app.windows.firstMatch.frame.width
        try XCTSkipIf(width > 500, "the window stayed \(width) pt wide: windowed apps are off or the drag missed")
        if !app.buttons["Chapters"].firstMatch.isHittable {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        }
        openChapters(app)
        assertSwitchInside(app, "p6-narrow")
        let window = app.windows.firstMatch.frame
        // Every page of the grid's first row is inside the window.
        let thumbs = app.staticTexts.allElementsBoundByIndex.filter { Int($0.label) != nil && $0.frame.minY > pagesSegment(app).frame.maxY }
        for thumb in thumbs.prefix(5) {
            XCTAssertTrue(window.contains(CGPoint(x: thumb.frame.midX, y: thumb.frame.midY)), "page \(thumb.label) is outside the window")
        }
    }

    /// Drags the app window's lower right corner left until the window is `width` wide.
    private func narrow(_ app: XCUIApplication, to width: CGFloat) {
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let frame = app.windows.firstMatch.frame
        guard frame.width > width + 20 else { return }
        let origin = springboard.coordinate(withNormalizedOffset: .zero)
        for inset in [CGFloat(6), 12, 2] {
            let start = origin.withOffset(CGVector(dx: frame.maxX - inset, dy: frame.maxY - inset))
            let end = origin.withOffset(CGVector(dx: frame.minX + width, dy: frame.maxY - inset))
            start.press(forDuration: 0.6, thenDragTo: end, withVelocity: .slow, thenHoldForDuration: 0.4)
            Thread.sleep(forTimeInterval: 1.5)
            if app.windows.firstMatch.frame.width < frame.width - 50 { break }
        }
        mark("p6-window", "before \(frame) after \(app.windows.firstMatch.frame)")
    }

    // MARK: P8, the opening screen's chapter

    func testTheOpeningScreenNamesTheChapterBeingOpened() throws {
        // The chapter on screen, from the reader's line under the title.
        let reading = launch(["-KaeriScene", "readermangartl", "-KaeriReader", "chrome"])
        let line = reading.buttons.matching(NSPredicate(format: "label CONTAINS 'Chapter '")).firstMatch
        XCTAssertTrue(line.waitForExistence(timeout: 20), "no reader line")
        let here = chapter(in: line.label)
        reading.terminate()

        let app = launch(["-KaeriScene", "readermangartl", "-KaeriReader", "openingnext"])
        let opening = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'Opening chapter '")).firstMatch
        XCTAssertTrue(opening.waitForExistence(timeout: 20), "no opening line")
        let named = chapter(in: opening.label)
        mark("p8-opening", "on screen \(here ?? "?"), opening line \"\(opening.label)\"")
        XCTAssertNotNil(here)
        XCTAssertNotEqual(named, here, "the opening line names the chapter being left: \(opening.label)")
    }

    /// The chapter number after "Chapter " or "chapter ".
    private func chapter(in label: String) -> String? {
        guard let range = label.range(of: "hapter ") else { return nil }
        let number = label[range.upperBound...].prefix { $0.isNumber || $0 == "." }
        return number.isEmpty ? nil : String(number)
    }

    // MARK: P12, the series header at AX XXXL

    func testTheSeriesHeaderFitsAtTheLargestTextSize() throws {
        try requirePad()
        let app = launch([
            "-KaeriScene", "readingseries", "-KaeriWork", "a:900301",
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL",
        ])
        let download = app.buttons["Download"].firstMatch
        XCTAssertTrue(download.waitForExistence(timeout: 20), "no Download menu")
        Thread.sleep(forTimeInterval: 1.5)
        let window = app.windows.firstMatch.frame
        let more = app.buttons.matching(NSPredicate(format: "label == 'More'")).allElementsBoundByIndex.map(\.frame)
        let main = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Read ' OR label BEGINSWITH 'Continue '"))
            .firstMatch
        let synopsisMore = app.buttons["Show the whole synopsis"].firstMatch
        let synopsis = synopsisMore.exists ? synopsisMore.frame : .null
        mark("p12-series-axxxl", "window \(window) main \(main.exists ? main.frame : .null) download \(download.frame) more \(more) synopsis more \(synopsis)")
        XCTAssertLessThanOrEqual(download.frame.maxX, window.maxX, "Download runs off the page")
        for frame in more {
            XCTAssertLessThanOrEqual(frame.maxX, window.maxX, "More runs off the page, \(frame)")
        }
        if main.exists {
            XCTAssertLessThanOrEqual(main.frame.maxX, window.maxX, "the main action runs off the page")
        }
    }

    func testAFailedStripImageNamesTheImage() throws {
        let app = launch(["-KaeriScene", "readerstrip", "-KaeriReader", "pagefailed"])
        let retry = app.buttons["Try again"].firstMatch
        XCTAssertTrue(retry.waitForExistence(timeout: 20), "no failed image")
        let texts = app.staticTexts.allElementsBoundByIndex.map(\.label)
        mark("p9-strip-failed", "texts \(texts)")
        XCTAssertTrue(texts.contains("Couldn't load"), "no Couldn't load under the image's number")
        XCTAssertFalse(texts.contains { $0.contains("Swipe") }, "a swipe hint on a strip")
        XCTAssertFalse(texts.contains { $0.hasPrefix("Couldn't load page") }, "a strip image called a page")
    }

    func testChooseChaptersDownloadFitsANarrowWindow() throws {
        let app = launch(["-KaeriScene", "readingseries", "-KaeriWork", "a:900301", "-KaeriSheet", "choose-chapters"])
        XCTAssertTrue(app.buttons["Mark read"].firstMatch.waitForExistence(timeout: 20), "no Choose chapters bar")
        if UIDevice.current.userInterfaceIdiom == .pad {
            let bar = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Download '")).firstMatch
            mark("p17-choose-wide", "window \(app.windows.firstMatch.frame) download \(bar.label) \(bar.frame)")
            narrow(app, to: 375)
        }
        let window = app.windows.firstMatch.frame
        try XCTSkipIf(window.width > 430, "the window did not get narrow: \(window)")
        Thread.sleep(forTimeInterval: 1)
        let download = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Download'"))
            .allElementsBoundByIndex.filter { $0.frame.minY > window.midY }.first
        mark("p17-choose-narrow", "window \(window) download \(download.map { "\($0.label) \($0.frame)" } ?? "none")")
        let button = try XCTUnwrap(download, "no Download in the bar")
        // "Download 12 chapters" needs about 180 pt at 17 pt; the bar leaves its amber button
        // about 130 at this width, so the label drops the count rather than being cut.
        XCTAssertEqual(button.label, "Download", "the count does not fit and is cut: \(button.label)")
        XCTAssertLessThanOrEqual(button.frame.maxX, window.maxX)
    }

    func testSidebarOnlySectionsKeepTheirPlaceInANarrowWindow() throws {
        try requirePad()
        for (section, title) in [("downloads", "Downloads"), ("providers", "Your sources")] {
            let app = launch(["-KaeriScene", "watching", "-KaeriSection", section])
            XCTAssertTrue(app.navigationBars[title].firstMatch.waitForExistence(timeout: 20), "no \(title) to start from")
            narrow(app, to: 375)
            let window = app.windows.firstMatch.frame
            guard window.width <= 500 else {
                app.terminate()
                throw XCTSkip("the window did not get narrow: \(window)")
            }
            Thread.sleep(forTimeInterval: 1.5)
            let bar = app.navigationBars[title].firstMatch
            mark("p11-narrow-\(section)", "window \(window) \(title) bar \(bar.exists ? "\(bar.frame)" : "none")")
            XCTAssertTrue(bar.exists, "\(title) is gone in a narrow window, another tab shows")
            app.terminate()
        }
    }
}
