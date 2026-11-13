// A folder that is already a source, picked again, stays a source however its sheet is left: by
// its close button, or by Change for another folder that is then not kept. Run on an iPad
// Simulator with the Audit scheme. The script makes two folders of made-up books and names them in
// TEST_RUNNER_KAERI_APP_FIXTURE_FOLDER and TEST_RUNNER_KAERI_APP_FIXTURE_OTHER_FOLDER: the
// Fixtures app takes them in place of the system's folder picker. Without them the test is skipped.

import XCTest

@MainActor
final class FolderSourceTests: XCTestCase {
    private var shown: XCUIApplication?

    override func setUp() {
        continueAfterFailure = false
    }

    /// A failed check keeps the screen's accessibility tree beside the script's folders.
    private func check(_ ok: Bool, _ message: String, file: StaticString = #filePath, line: UInt = #line) {
        guard !ok else { return }
        if let shown, let marks = ProcessInfo.processInfo.environment["KAERI_TEST_MARKS"] {
            try? shown.debugDescription.write(to: URL(filePath: marks).appending(path: "folder-tree.txt"), atomically: true, encoding: .utf8)
        }
        XCTFail(message, file: file, line: line)
    }

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-KaeriScene", "readinghome", "-KaeriSection", "providers", "-KaeriMedium", "read"]
        for (name, value) in ProcessInfo.processInfo.environment where name.hasPrefix("KAERI_APP_") {
            app.launchEnvironment["KAERI_" + name.dropFirst("KAERI_APP_".count)] = value
        }
        app.launch()
        shown = app
        return app
    }

    /// Add a source, then A folder: the Fixtures app reads the test's folder.
    private func addFolder(_ app: XCUIApplication) {
        let add = app.buttons["Add a source"].firstMatch
        check(add.waitForExistence(timeout: 15), "no Add a source")
        add.tap()
        let folder = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'A folder'")).firstMatch
        check(folder.waitForExistence(timeout: 5), "no A folder in Add a source")
        folder.tap()
    }

    /// Whether Your sources lists the folder: its row reads as its name, then its line.
    private func listed(_ app: XCUIApplication, _ name: String) -> Bool {
        Thread.sleep(forTimeInterval: 1.5)
        let row = NSPredicate(format: "label == %@ OR label BEGINSWITH %@", name, name + ", ")
        return app.descendants(matching: .any).matching(row).firstMatch.exists
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
        check(keep.waitForExistence(timeout: 30), "the folder's sheet did not come")
        keep.tap()
        check(keep.waitForNonExistence(timeout: 5), "the folder's sheet stayed")
        check(listed(app, name), "the folder is not in Your sources")

        // Picked again: the sheet says so and offers Done; its close button leaves the folder.
        addFolder(app)
        let already = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label BEGINSWITH %@", "\(name) is already in your sources")).firstMatch
        check(already.waitForExistence(timeout: 30), "the sheet did not say the folder is already a source")
        check(app.buttons["Done"].exists, "no Done for a folder already added")
        app.buttons["Close"].firstMatch.tap()
        check(already.waitForNonExistence(timeout: 5), "the sheet stayed after Close")
        check(listed(app, name), "closing the sheet took the folder out")

        // Picked again, then Change to another folder, whose sheet is closed: the other folder
        // goes, as it was only just added, and the first stays.
        addFolder(app)
        check(already.waitForExistence(timeout: 30), "the sheet did not come again")
        app.buttons["Change"].firstMatch.tap()
        // The other folder is new: its sheet offers Add folder.
        check(keep.waitForExistence(timeout: 30), "Change did not read the other folder")
        app.buttons["Close"].firstMatch.tap()
        check(keep.waitForNonExistence(timeout: 5), "the folder's sheet stayed")
        check(listed(app, name), "Change took the folder out")
        check(!listed(app, otherName), "the other folder stayed without Add folder")
    }
}
