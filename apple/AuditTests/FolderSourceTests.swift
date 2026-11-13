// A folder that is already a source, picked again, stays a source however its sheet is left: by
// its close button, or by Change for another folder that is then not kept. Run on an iPad
// Simulator with the Audit scheme. The script makes two folders of made-up books and names them in
// TEST_RUNNER_KAERI_APP_FIXTURE_FOLDER and TEST_RUNNER_KAERI_APP_FIXTURE_OTHER_FOLDER: the
// Fixtures app takes them in place of the system's folder picker. Without them the test is skipped.

import XCTest

@MainActor
final class FolderSourceTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-KaeriScene", "readinghome", "-KaeriSection", "providers", "-KaeriMedium", "read"]
        for (name, value) in ProcessInfo.processInfo.environment where name.hasPrefix("KAERI_APP_") {
            app.launchEnvironment["KAERI_" + name.dropFirst("KAERI_APP_".count)] = value
        }
        app.launch()
        return app
    }

    /// Add a source, then A folder: the Fixtures app reads the test's folder.
    private func addFolder(_ app: XCUIApplication) {
        let add = app.buttons["Add a source"].firstMatch
        if !add.waitForExistence(timeout: 15), let marks = ProcessInfo.processInfo.environment["KAERI_TEST_MARKS"] {
            try? app.debugDescription.write(to: URL(filePath: marks).appending(path: "folder-tree.txt"), atomically: true, encoding: .utf8)
        }
        XCTAssertTrue(add.exists, "no Add a source")
        add.tap()
        let folder = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'A folder'")).firstMatch
        XCTAssertTrue(folder.waitForExistence(timeout: 5), "no A folder in Add a source")
        folder.tap()
    }

    private func folderRows(_ app: XCUIApplication, named name: String) -> Int {
        Thread.sleep(forTimeInterval: 1.5)
        return app.staticTexts.matching(NSPredicate(format: "label == %@", name)).count
    }

    func testAFolderPickedAgainStaysWhateverTheSheetDoes() throws {
        let env = ProcessInfo.processInfo.environment
        let first = try XCTUnwrap(env["KAERI_APP_FIXTURE_FOLDER"], "needs the script's folders")
        let other = try XCTUnwrap(env["KAERI_APP_FIXTURE_OTHER_FOLDER"], "needs the script's folders")
        let name = URL(filePath: first).lastPathComponent
        let otherName = URL(filePath: other).lastPathComponent
        let app = launch()

        // Added the first time, and kept.
        addFolder(app)
        let keep = app.buttons["Add folder"].firstMatch
        XCTAssertTrue(keep.waitForExistence(timeout: 30), "the folder's sheet did not come")
        keep.tap()
        XCTAssertTrue(keep.waitForNonExistence(timeout: 5))
        XCTAssertEqual(folderRows(app, named: name), 1, "the folder is not in Your sources")

        // Picked again: the sheet says so and offers Done; its close button leaves the folder.
        addFolder(app)
        let already = app.staticTexts["\(name) is already in your sources"]
        XCTAssertTrue(already.waitForExistence(timeout: 30), "the sheet did not say the folder is already a source")
        XCTAssertTrue(app.buttons["Done"].exists, "no Done for a folder already added")
        app.buttons["Close"].firstMatch.tap()
        XCTAssertTrue(already.waitForNonExistence(timeout: 5))
        XCTAssertEqual(folderRows(app, named: name), 1, "closing the sheet took the folder out")

        // Picked again, then Change to another folder, whose sheet is closed: the other folder
        // goes, as it was only just added, and the first stays.
        addFolder(app)
        XCTAssertTrue(already.waitForExistence(timeout: 30))
        app.buttons["Change"].firstMatch.tap()
        let otherSheet = app.staticTexts[otherName].firstMatch
        XCTAssertTrue(otherSheet.waitForExistence(timeout: 30), "Change did not read the other folder")
        app.buttons["Close"].firstMatch.tap()
        XCTAssertEqual(folderRows(app, named: name), 1, "Change took the folder out")
        XCTAssertEqual(folderRows(app, named: otherName), 0, "the other folder stayed without Add folder")
    }
}
