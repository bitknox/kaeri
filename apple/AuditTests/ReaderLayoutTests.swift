// The iPad layout bugs of the round 5 hunt, checked in the Fixtures app: the Chapters popover in
// landscape and in a narrow window, the opening screen's chapter, the series header at the
// largest text size, a failed strip image's words, Choose chapters' Download in a narrow window,
// Downloads and Sources keeping their place when the window is made narrow, the app running on
// through narrow and wide, and Choose chapters' picks kept through them. Run with the Audit scheme
// on an iPad Simulator.
//
// With TEST_RUNNER_KAERI_TEST_MARKS set to a folder, each test leaves a screenshot there and a
// line of the frames it measured, for before and after shots.
//
// The narrow window needs windowed apps on in the Simulator (SpringBoard's
// SBChamoisWindowingEnabled); the test drags the window's corner through SpringBoard and is
// skipped when the window does not get narrow.
//
// The iPad Simulator's hierarchy has no Window element on iOS 27: its frame reads as zero and
// XCUITest finds no hit point for a tap. The window is measured from the app's own elements, and a
// button that cannot be hit is tapped at its middle.

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

    /// The screen in the current orientation, in points.
    private func screen() -> CGRect {
        let shot = XCUIScreen.main.screenshot().image
        // The runner is on the same device, so its screen's scale is the device's.
        let scale = shot.scale > 1 ? 1 : UITraitCollection.current.displayScale
        return CGRect(x: 0, y: 0, width: shot.size.width / scale, height: shot.size.height / scale)
    }

    /// The app's window: the Window element's frame when there is one, otherwise what the app's
    /// top elements cover, and the screen when they cover nothing.
    private func windowFrame(_ app: XCUIApplication) -> CGRect {
        let window = app.windows.firstMatch
        if window.exists, !window.frame.isEmpty { return window.frame }
        let tops = app.children(matching: .any).allElementsBoundByIndex.map(\.frame).filter { !$0.isEmpty }
        if let first = tops.first {
            return tops.dropFirst().reduce(first) { $0.union($1) }.intersection(screen())
        }
        return app.frame.isEmpty ? screen() : app.frame
    }

    /// A point on the screen, for taps and drags by place.
    private func point(_ app: XCUIApplication, _ x: CGFloat, _ y: CGFloat) -> XCUICoordinate {
        app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x - app.frame.minX, dy: y - app.frame.minY))
    }

    /// Taps an element, at its middle when XCUITest cannot find a hit point for it.
    private func tap(_ element: XCUIElement, in app: XCUIApplication) {
        if element.isHittable {
            element.tap()
        } else {
            let frame = element.frame
            point(app, frame.midX, frame.midY).tap()
        }
    }

    /// Every text on screen with its frame, read from one snapshot of the app. Reading the texts
    /// one index at a time asks the app again for each, and a strip still placing its images
    /// can have fewer by the time the last index is asked for.
    /// `any` takes every element with a label, such as a button whose text is its label.
    private func texts(_ app: XCUIApplication, any: Bool = false) throws -> [(label: String, frame: CGRect, type: UInt)] {
        var found: [(String, CGRect, UInt)] = []
        func walk(_ node: XCUIElementSnapshot) {
            if node.elementType == .staticText || (any && !node.label.isEmpty) {
                found.append((node.label, node.frame, node.elementType.rawValue))
            }
            node.children.forEach(walk)
        }
        walk(try app.snapshot())
        return found
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
        tap(button, in: app)
    }

    private func closeChapters(_ app: XCUIApplication) {
        app.typeKey(XCUIKeyboardKey.escape, modifierFlags: [])
        if pagesSegment(app).exists {
            let window = windowFrame(app)
            point(app, window.minX + window.width * 0.08, window.midY).tap()
        }
        _ = pagesSegment(app).waitForNonExistence(timeout: 5)
    }

    /// The switch sits inside the window and under the status bar's strip.
    private func assertSwitchInside(_ app: XCUIApplication, _ name: String) {
        let segment = pagesSegment(app)
        XCTAssertTrue(segment.waitForExistence(timeout: 10), "\(name): the Chapters panel did not open")
        Thread.sleep(forTimeInterval: 1)
        let window = windowFrame(app)
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
        app.terminate()
        // Cmd-L with the reader's bar hidden, where the Chapters button has slid away.
        let hidden = launch(["-KaeriScene", "readermangartl"])
        XCTAssertTrue(hidden.wait(for: .runningForeground, timeout: 20))
        Thread.sleep(forTimeInterval: 6)
        hidden.typeKey("l", modifierFlags: .command)
        assertSwitchInside(hidden, "p5-landscape-cmd-l-hidden")
    }

    // MARK: P6, Chapters in a narrow window

    func testChaptersFitANarrowWindow() throws {
        try requirePad()
        let app = launch(["-KaeriScene", "readermangartl", "-KaeriReader", "chrome"])
        XCTAssertTrue(app.buttons["Chapters"].firstMatch.waitForExistence(timeout: 20))
        narrow(app, to: 375)
        let width = windowFrame(app).width
        try XCTSkipIf(width > 500, "the window stayed \(width) pt wide: windowed apps are off or the drag missed")
        // The reader's bar may have hidden itself: a tap in the middle brings it back.
        let chapters = app.buttons["Chapters"].firstMatch
        let window = windowFrame(app)
        if !(chapters.exists && window.contains(CGPoint(x: chapters.frame.midX, y: chapters.frame.midY))) {
            point(app, window.midX, window.midY).tap()
        }
        openChapters(app)
        assertSwitchInside(app, "p6-narrow")
        let narrowWindow = windowFrame(app)
        // Every page of the grid's first row is inside the window.
        let below = pagesSegment(app).frame.maxY
        // The grid's first row, the one with page 1. A page's number can be its button's label,
        // and the reader's bar under the panel has numbers of its own, such as the page count.
        let labelled = try texts(app, any: true).filter { $0.frame.minY > below }
        mark("p6-numbers", "\(labelled.prefix(40).map { "\($0.type) \($0.label) \($0.frame.origin)" })")
        let numbers = labelled.filter { Int($0.label) != nil }
        let first = try XCTUnwrap(numbers.first { $0.label == "1" }, "no page 1 in the grid")
        let thumbs = numbers.filter { abs($0.frame.minY - first.frame.minY) < 4 }
        XCTAssertGreaterThanOrEqual(thumbs.count, 3, "the grid's first row has \(thumbs.count) pages")
        for thumb in thumbs {
            XCTAssertTrue(narrowWindow.contains(CGPoint(x: thumb.frame.midX, y: thumb.frame.midY)), "page \(thumb.label) is outside the window")
        }
    }

    // MARK: A resize keeps the page and what it holds

    /// Choose chapters with every unread chapter chosen, then narrow, wide and narrow again: the
    /// series page stays open, still choosing, with the same chapters chosen.
    func testChosenChaptersSurviveNarrowAndWide() throws {
        try requirePad()
        let app = launch(["-KaeriScene", "readingseries", "-KaeriWork", "a:900301", "-KaeriSheet", "choose-chapters"])
        XCTAssertTrue(app.buttons["Mark read"].firstMatch.waitForExistence(timeout: 20), "no Choose chapters bar")
        // A windowed app opens at the size it was left at; the check starts wide.
        if windowFrame(app).width < 600 {
            widen(app)
            Thread.sleep(forTimeInterval: 2)
        }
        let count = NSPredicate(format: "label MATCHES '^[0-9]+ chapters?$'")
        let start = app.staticTexts.matching(count).firstMatch
        XCTAssertTrue(start.waitForExistence(timeout: 5), "no count of chosen chapters")
        let before = start.label
        let unread = app.buttons["All unread"].firstMatch
        XCTAssertTrue(unread.waitForExistence(timeout: 5), "no All unread")
        tap(unread, in: app)
        Thread.sleep(forTimeInterval: 1)
        let chosen = app.staticTexts.matching(count).firstMatch.label
        mark("picks-start", "window \(windowFrame(app)) \(before), after All unread \(chosen)")
        XCTAssertNotEqual(chosen, before, "All unread chose the same chapters as the start, so the check proves nothing")
        let steps: [(String, (XCUIApplication) -> Void)] = [
            ("narrow", { self.narrow($0, to: 375) }),
            ("wide", { self.widen($0) }),
            ("narrow again", { self.narrow($0, to: 375) }),
        ]
        for (name, step) in steps {
            step(app)
            Thread.sleep(forTimeInterval: 2)
            let window = windowFrame(app)
            let now = app.staticTexts.matching(count).firstMatch
            let bar = app.buttons["Mark read"].firstMatch
            mark("picks-\(name.replacingOccurrences(of: " ", with: "-"))",
                 "window \(window) state \(app.state.rawValue) count \(now.exists ? now.label : "none") bar \(bar.exists)")
            XCTAssertEqual(app.state, .runningForeground, "\(name): the app ended")
            guard app.state == .runningForeground else { return }
            XCTAssertTrue(bar.exists, "\(name): Choose chapters is gone, the page or its picks were lost")
            XCTAssertEqual(now.exists ? now.label : "none", chosen, "\(name): the chosen chapters changed")
        }
    }

    // MARK: A narrow window keeps the app running

    /// Narrowed and widened twice, on Watch, on Read and with the reader open: Read's bar has
    /// fewer tabs than its sidebar, Watch's has as many.
    func testTheAppKeepsRunningThroughNarrowAndWide() throws {
        try requirePad()
        let scenes: [(String, [String])] = [
            ("watching", ["-KaeriScene", "watching"]),
            ("reading", ["-KaeriScene", "readinghome"]),
            ("reader", ["-KaeriScene", "readermangartl", "-KaeriReader", "chrome"]),
        ]
        for (name, arguments) in scenes {
            let app = launch(arguments)
            XCTAssertTrue(app.wait(for: .runningForeground, timeout: 20), "\(name): the app did not start")
            Thread.sleep(forTimeInterval: 3)
            for turn in 1...2 {
                narrow(app, to: 375)
                let window = windowFrame(app)
                guard window.width <= 500 else {
                    app.terminate()
                    throw XCTSkip("the window did not get narrow: \(window)")
                }
                Thread.sleep(forTimeInterval: 2)
                mark("narrow-\(name)-\(turn)", "window \(window) state \(app.state.rawValue)")
                XCTAssertEqual(app.state, .runningForeground, "\(name): the app ended when the window was made narrow (turn \(turn))")
                guard app.state == .runningForeground else { break }
                widen(app)
                Thread.sleep(forTimeInterval: 2)
                XCTAssertEqual(app.state, .runningForeground, "\(name): the app ended when the window was made wide (turn \(turn))")
                guard app.state == .runningForeground else { break }
            }
            app.terminate()
        }
    }

    /// The window's corner dragged back out to the screen's edge.
    private func widen(_ app: XCUIApplication) {
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let frame = windowFrame(app)
        let screen = screen()
        let origin = springboard.coordinate(withNormalizedOffset: .zero)
        let start = origin.withOffset(CGVector(dx: frame.maxX - 6 - springboard.frame.minX, dy: frame.maxY - 6 - springboard.frame.minY))
        let end = origin.withOffset(CGVector(dx: screen.maxX - 2 - springboard.frame.minX, dy: frame.maxY - 6 - springboard.frame.minY))
        start.press(forDuration: 0.6, thenDragTo: end, withVelocity: .slow, thenHoldForDuration: 0.4)
    }

    /// Drags the app window's lower right corner left until the window is `width` wide.
    private func narrow(_ app: XCUIApplication, to width: CGFloat) {
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let frame = windowFrame(app)
        guard frame.width > width + 20 else { return }
        let origin = springboard.coordinate(withNormalizedOffset: .zero)
        let board = springboard.frame
        for inset in [CGFloat(6), 12, 2] {
            let start = origin.withOffset(CGVector(dx: frame.maxX - inset - board.minX, dy: frame.maxY - inset - board.minY))
            let end = origin.withOffset(CGVector(dx: frame.minX + width - board.minX, dy: frame.maxY - inset - board.minY))
            start.press(forDuration: 0.6, thenDragTo: end, withVelocity: .slow, thenHoldForDuration: 0.4)
            Thread.sleep(forTimeInterval: 1.5)
            if windowFrame(app).width < frame.width - 50 { break }
        }
        mark("p6-window", "before \(frame) after \(windowFrame(app))")
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
        let window = windowFrame(app)
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
        let shown = try texts(app).map(\.label)
        mark("p9-strip-failed", "texts \(shown)")
        XCTAssertTrue(shown.contains("Couldn't load"), "no Couldn't load under the image's number")
        XCTAssertFalse(shown.contains { $0.contains("Swipe") }, "a swipe hint on a strip")
        XCTAssertFalse(shown.contains { $0.hasPrefix("Couldn't load page") }, "a strip image called a page")
    }

    func testChooseChaptersDownloadFitsANarrowWindow() throws {
        let app = launch(["-KaeriScene", "readingseries", "-KaeriWork", "a:900301", "-KaeriSheet", "choose-chapters"])
        XCTAssertTrue(app.buttons["Mark read"].firstMatch.waitForExistence(timeout: 20), "no Choose chapters bar")
        if UIDevice.current.userInterfaceIdiom == .pad {
            // A windowed app opens at the size it was left at, which can be narrow already.
            let bar = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Download'")).firstMatch
            mark("p17-choose-wide", "window \(windowFrame(app)) download \(bar.exists ? "\(bar.label) \(bar.frame)" : "none")")
            narrow(app, to: 375)
        }
        let window = windowFrame(app)
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
            let window = windowFrame(app)
            guard window.width <= 500 else {
                app.terminate()
                throw XCTSkip("the window did not get narrow: \(window)")
            }
            Thread.sleep(forTimeInterval: 1.5)
            let bar = app.navigationBars[title].firstMatch
            mark("p11-narrow-\(section)", "window \(window) \(title) bar \(bar.exists ? "\(bar.frame)" : "none")")
            XCTAssertTrue(bar.exists, "\(title) is gone in a narrow window, another tab shows")
            widen(app)
            Thread.sleep(forTimeInterval: 1.5)
            let wide = app.navigationBars[title].firstMatch
            mark("p11-wide-\(section)", "window \(windowFrame(app)) \(title) bar \(wide.exists ? "\(wide.frame)" : "none")")
            XCTAssertTrue(wide.exists, "\(title) is gone when the window is made wide again")
            app.terminate()
        }
    }
}
