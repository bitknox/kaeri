// Read Home and Library at the largest accessibility text size stay as wide as the screen: no
// heading, link or menu pushes the column wider, so no row loses its leading edge. Run with the
// Audit scheme on an iPhone Simulator; the text size is set at launch.

import UIKit
import XCTest

@MainActor
final class ReadingTextSizeTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func launch(_ extra: [String] = []) throws -> XCUIApplication {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .phone, "the narrowest screen is the check")
        let app = XCUIApplication()
        app.launchArguments = [
            "-KaeriScene", "readinghome", "-KaeriMedium", "read",
            "-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL",
        ] + extra
        app.launch()
        return app
    }

    /// Every word and control on screen whose leading edge is left of the screen's, or whose
    /// trailing edge is right of it when `trailing` says so. Shelf tiles run on past the
    /// trailing edge by design, so only the named controls are held to it.
    private func outside(_ app: XCUIApplication, trailing: Set<String>) throws -> [String] {
        let screen = app.windows.firstMatch.frame
        var found: [String] = []
        func walk(_ node: XCUIElementSnapshot) {
            let frame = node.frame
            let kinds: Set<XCUIElement.ElementType> = [.staticText, .button, .image, .other]
            if kinds.contains(node.elementType), !node.label.isEmpty, frame.width > 0, frame.height > 0,
               frame.maxX > screen.minX, frame.minX < screen.maxX {
                if frame.minX < screen.minX - 1 {
                    found.append("\(node.label) starts at x \(Int(frame.minX))")
                }
                if trailing.contains(node.label), frame.maxX > screen.maxX + 1 {
                    found.append("\(node.label) ends at x \(Int(frame.maxX)) of \(Int(screen.maxX))")
                }
            }
            node.children.forEach(walk)
        }
        walk(try app.snapshot())
        return found
    }

    func testReadHomeFitsTheScreenAtTheLargestTextSize() throws {
        let app = try launch()
        let viewAll = app.buttons.matching(NSPredicate(format: "label == 'View all'")).firstMatch
        XCTAssertTrue(viewAll.waitForExistence(timeout: 30), "no shelf with View all on Read Home")
        Thread.sleep(forTimeInterval: 1.5)
        let found = try outside(app, trailing: ["View all"])
        XCTAssertTrue(found.isEmpty, "Read Home is wider than the screen: " + found.prefix(8).joined(separator: "; "))
    }

    func testLibraryFitsTheScreenAtTheLargestTextSize() throws {
        let app = try launch(["-KaeriSection", "library"])
        let list = app.buttons["List"].firstMatch
        XCTAssertTrue(list.waitForExistence(timeout: 30), "no list menu on Library")
        Thread.sleep(forTimeInterval: 1.5)
        let found = try outside(app, trailing: ["List", "Order"])
        XCTAssertTrue(found.isEmpty, "Library is wider than the screen: " + found.prefix(8).joined(separator: "; "))
    }
}
