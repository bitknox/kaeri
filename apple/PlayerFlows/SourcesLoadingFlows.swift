// The player panel's Sources tab while its sources answer one by one (player panel board,
// "Sources loading"), on the Fixtures app's SourcesLoading scene: Home server lists one release at
// 0.45 s, Media folder can't match the title at 1.3 s, Example index lists five at 2.4 s, Archive
// example fails at 6.2 s, Second example takes too long at 20 s, and Old example is switched off.
// The panel opens on Sources by itself and asks at a moment the app says ahead, so each picture
// is taken at its moment from when the tab asked: 0.3 s, 1 s, 4 s, 12 s and 21 s. Each is attached to the
// test, kept, and checked against the rows' words.
//
//   xcodebuild -project apple/Kaeri.xcodeproj -scheme "Kaeri Mac Player" \
//     -destination platform=macOS -only-testing:KaeriMacPlayerTests/SourcesLoadingFlows test
//   xcodebuild -project apple/Kaeri.xcodeproj -scheme "Kaeri Player Flows" \
//     -destination "platform=iOS Simulator,name=<yours>" test
//
// KAERI_MEDIA (TEST_RUNNER_KAERI_MEDIA for xcodebuild) names the episode every source plays,
// and KAERI_HOME the folder the app keeps its data in; the scene's own otherwise.

import XCTest

@MainActor
final class SourcesLoadingFlows: XCTestCase {
    /// The moments after the tab asked, and how far a picture may be from its moment.
    static let moments: [(name: String, at: TimeInterval)] = [("0s", 0.3), ("1s", 1), ("4s", 4), ("12s", 12), ("21s", 21)]
    static let leeway: TimeInterval = 0.3

    override func setUp() {
        continueAfterFailure = true
    }

    func testRowsAnswerOneByOne() throws {
        let app = launch()
        defer { app.terminate() }
        let marker = app.descendants(matching: .any)["panel-asks-at"]
        XCTAssertTrue(marker.waitForExistence(timeout: 120), "the player did not come up with the panel's moment")
        let label = marker.label
        let opens = try XCTUnwrap(Double(label), "no moment in \(label)") / 1000
        XCTAssertGreaterThan(opens - Date().timeIntervalSince1970, Self.moments[0].at, "the moment came before it was seen")

        // A picture takes a moment of its own: started early by half of it.
        let took = shoot(app).took
        var late: [String] = []
        for moment in Self.moments {
            let start = opens + moment.at - took / 2
            let wait = start - Date().timeIntervalSince1970
            if wait > 0 { Thread.sleep(forTimeInterval: wait) }
            let shot = shoot(app)
            let middle = shot.started + shot.took / 2 - opens
            let attachment = XCTAttachment(screenshot: shot.screenshot)
            attachment.name = "\(Self.form)-\(moment.name)"
            attachment.lifetime = .keepAlways
            add(attachment)
            print(String(format: "SourcesLoadingFlows: %@ taken at %.2f s (%.2f s to take)", moment.name, middle, shot.took))
            if abs(middle - moment.at) > Self.leeway { late.append("\(moment.name) at \(middle)") }
            check(app, at: moment.name)
        }
        XCTAssertTrue(late.isEmpty, "pictures off their moment: \(late)")
    }

    // MARK: What each moment says

    /// The rows' words at a moment, read after its picture in one look at the whole screen: what the
    /// board's frame says. Every label on screen is kept with the test should a word be missing.
    private func check(_ app: XCUIApplication, at moment: String) {
        guard let snapshot = try? app.snapshot() else { return XCTFail("no look at the screen at \(moment)") }
        var labels: [String] = []
        func walk(_ element: XCUIElementSnapshot) {
            if !element.label.isEmpty { labels.append(element.label) }
            element.children.forEach(walk)
        }
        walk(snapshot)
        let wanted: [(words: String, there: Bool, why: String)] = switch moment {
        case "0s": [
            ("Playing now", true, "the Sources tab is not open"),
            ("Switched off", true, "Old example does not say it is switched off at once"),
        ]
        case "1s": [("Asking", true, "no row says Asking at 1 s")]
        case "4s": [
            ("Couldn't match this title", true, "Media folder has not answered"),
            ("5 releases", true, "Example index has not listed its releases"),
            ("Asking ·", true, "the slow sources do not count their seconds"),
        ]
        case "12s": [
            ("Didn't answer", true, "Archive example has not failed"),
            ("Asking ·", true, "Second example is no longer asked"),
        ]
        default: [
            ("Took too long", true, "Second example has not timed out"),
            ("Asking", false, "a row is still asked"),
        ]
        }
        let wrong = wanted.filter { want in labels.contains { $0.contains(want.words) } != want.there }
        guard !wrong.isEmpty else { return }
        let said = XCTAttachment(string: labels.joined(separator: "\n"))
        said.name = "\(Self.form)-\(moment)-labels"
        said.lifetime = .keepAlways
        add(said)
        XCTFail("\(moment): " + wrong.map(\.why).joined(separator: "; "))
    }

    // MARK: Launching and pictures

    private func launch() -> XCUIApplication {
        let app = XCUIApplication()
        var arguments = ["-KaeriScene", "sourcesloading", "-KaeriPlayerShot", "panel-sources-loading"]
        let environment = ProcessInfo.processInfo.environment
        if let media = environment["KAERI_MEDIA"], !media.isEmpty {
            arguments += ["-KaeriMedia", media]
        }
        if let home = environment["KAERI_HOME"], !home.isEmpty {
            arguments += ["-KaeriHome", home]
            app.launchEnvironment["KAERI_HOME_OVERRIDE"] = home
        }
        #if os(macOS)
        // A window of its own size each time, not the last run's, and never full screen.
        arguments += ["-ApplePersistenceIgnoreState", "YES"]
        #else
        XCUIDevice.shared.orientation = .landscapeLeft
        #endif
        app.launchArguments = arguments
        app.launch()
        return app
    }

    /// The form the pictures are named for.
    static var form: String {
        #if os(macOS)
        "mac"
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        #endif
    }

    /// The screen as it is now: the app's window on the Mac, the whole Simulator screen otherwise.
    private func shoot(_ app: XCUIApplication) -> (screenshot: XCUIScreenshot, started: TimeInterval, took: TimeInterval) {
        let started = Date().timeIntervalSince1970
        #if os(macOS)
        let screenshot = app.windows.firstMatch.screenshot()
        #else
        let screenshot = XCUIScreen.main.screenshot()
        #endif
        return (screenshot, started, Date().timeIntervalSince1970 - started)
    }
}
