// The Sources tab's answers past the first listing: the next episode playing with the tab open asks
// again for its sources, and a listing that asks no source ends with why. Pictures on either side
// of each moment, for the before and after of each.

import XCTest

@MainActor
final class PanelAnswersFlows: XCTestCase {
    override func setUp() {
        continueAfterFailure = true
    }

    /// Episode 7's tab, 3 s into its listing, then episode 8 playing with it open: the tab lists
    /// episode 8's sources, whose releases answer at once.
    func testTheNextEpisodeAsksAgain() throws {
        try run(shot: "panel-next-episode", moments: [("next-before", -0.5), ("next-after", 4)]) { moment in
            switch moment {
            case "next-before": []
            default: [
                ("5 releases", true, "episode 8's sources were not asked for"),
                ("Asking", false, "a row still asks for episode 7"),
            ]
            }
        }
    }

    /// The tab asks for an episode that has not aired: no source is asked, and the tab says why in
    /// place of rows that would count on.
    func testAListingThatAsksNoSourceEnds() throws {
        try run(shot: "panel-sources-failed", moments: [("failed-1s", 1), ("failed-6s", 6)]) { moment in
            switch moment {
            case "failed-1s": []
            default: [
                ("airs", true, "the tab does not say why nothing was asked"),
                ("Asking", false, "a row still counts"),
            ]
            }
        }
    }

    // MARK: Launching and pictures

    private func run(
        shot: String, moments: [(name: String, at: TimeInterval)],
        wanted: (String) -> [(words: String, there: Bool, why: String)]
    ) throws {
        let app = launch(shot: shot)
        defer { app.terminate() }
        let marker = app.descendants(matching: .any)["panel-asks-at"]
        XCTAssertTrue(marker.waitForExistence(timeout: 120), "the player did not come up with the panel's moment")
        let moment = try XCTUnwrap(Double(marker.label), "no moment in \(marker.label)") / 1000
        for (name, at) in moments {
            let wait = moment + at - Date().timeIntervalSince1970
            if wait > 0 { Thread.sleep(forTimeInterval: wait) }
            #if os(macOS)
            let screenshot = app.windows.firstMatch.screenshot()
            #else
            let screenshot = XCUIScreen.main.screenshot()
            #endif
            let picture = XCTAttachment(screenshot: screenshot)
            picture.name = "\(Self.form)-\(name)"
            picture.lifetime = .keepAlways
            add(picture)
            let labels = self.labels(app)
            let wrong = wanted(name).filter { want in labels.contains { $0.contains(want.words) } != want.there }
            guard !wrong.isEmpty else { continue }
            let said = XCTAttachment(string: labels.joined(separator: "\n"))
            said.name = "\(Self.form)-\(name)-labels"
            said.lifetime = .keepAlways
            add(said)
            XCTFail("\(name): " + wrong.map(\.why).joined(separator: "; "))
        }
    }

    private func labels(_ app: XCUIApplication) -> [String] {
        guard let snapshot = try? app.snapshot() else { return [] }
        var labels: [String] = []
        func walk(_ element: XCUIElementSnapshot) {
            if !element.label.isEmpty { labels.append(element.label) }
            element.children.forEach(walk)
        }
        walk(snapshot)
        return labels
    }

    private func launch(shot: String) -> XCUIApplication {
        let app = XCUIApplication()
        var arguments = ["-KaeriScene", "sourcesloading", "-KaeriPlayerShot", shot]
        let environment = ProcessInfo.processInfo.environment
        if let media = environment["KAERI_MEDIA"], !media.isEmpty {
            arguments += ["-KaeriMedia", media]
        }
        if let home = environment["KAERI_HOME"], !home.isEmpty {
            // A home of its own per launch, so one test's listing never meets the other's.
            let own = home + "/" + shot
            try? FileManager.default.createDirectory(atPath: own, withIntermediateDirectories: true)
            arguments += ["-KaeriHome", own]
            app.launchEnvironment["KAERI_HOME_OVERRIDE"] = own
        }
        #if os(macOS)
        arguments += ["-ApplePersistenceIgnoreState", "YES"]
        #else
        XCUIDevice.shared.orientation = .landscapeLeft
        #endif
        app.launchArguments = arguments
        app.launch()
        return app
    }

    static var form: String {
        #if os(macOS)
        "mac"
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        #endif
    }
}
