// What the Mac player names after a switch picked in its Episodes and Sources panel: the
// starting screen a slow file brings up, the notice as the switch lands, and Playing now in the
// Sources tab name the source switched to, never the one the episode started on.
//
// The Fixtures app's Switching scene plays episode 7 from Lantern Index, with Harbor Index after
// it, and serves every switch's file slowly enough that the starting screen comes up over it.
// Each step keeps screenshots of the starting screen, the notice and the Sources tab in the
// result bundle:
//
//   xcodebuild -project apple/Kaeri.xcodeproj -scheme "Kaeri Mac Player" \
//     -destination platform=macOS -only-testing:KaeriMacPlayerTests/MacPlayerSwitchTests test
//
// KAERI_HOME_OVERRIDE, passed on as TEST_RUNNER_KAERI_HOME_OVERRIDE, puts the app's home there.

import AppKit
import XCTest

@MainActor
final class MacPlayerSwitchTests: XCTestCase {
    func testEveryNameFollowsTheSwitch() throws {
        let app = launch()
        defer { app.terminate() }
        XCTAssertEqual(playingNow(app, "0-start"), "Lantern Index")

        // Another source.
        try step(app, "1-source", expected: "Harbor Index") {
            self.row(app, "Harbor Index").click()
        }
        // A release of the first source, under its row.
        try step(app, "2-release", expected: "Lantern Index") {
            self.row(app, "Lantern Index").click()
            let release = self.row(app, "Stardust Wayfarer 07 [Ember Subs]")
            XCTAssertTrue(release.waitForExistence(timeout: 10), "no Ember Subs release")
            release.click()
        }
        // The other version, which only Harbor Index has for this episode.
        try step(app, "3-dub", expected: "Harbor Index", dubbed: true) {
            let dubbed = app.radioButtons["Dubbed"].firstMatch
            XCTAssertTrue(dubbed.waitForExistence(timeout: 10), "no Dubbed")
            dubbed.click()
        }
    }

    // MARK: A step

    /// Open the Sources tab, pick what `pick` clicks, close the panel, and watch the switch land:
    /// every name the starting screen and the notice give is `expected`, and so is Playing now.
    private func step(
        _ app: XCUIApplication, _ name: String, expected: String, dubbed: Bool = false, pick: () -> Void
    ) throws {
        openSources(app)
        pick()
        // Closed, so the landing says so in a notice and the starting screen is in full view.
        click(app, "Close")

        var named: Set<String> = []
        var curtainShot = false
        var noticeShot = false
        let deadline = Date().addingTimeInterval(30)
        var settledSince: Date?
        while Date() < deadline {
            let texts = shownTexts(app)
            let from = texts.compactMap { Self.name(after: "From ", in: $0) }
            let waiting = texts.compactMap { Self.name(after: "Still waiting for ", in: $0) }
            named.formUnion(from + waiting)
            let curtain = !from.isEmpty || texts.contains { $0.hasPrefix("Opening") || $0.hasPrefix("Buffering") }
            if curtain {
                if dubbed {
                    let sub = app.radioButtons["Subtitled"].firstMatch
                    if sub.exists, (sub.value as? Int) == 1 || sub.isSelected {
                        named.insert("Subtitled on the starting screen")
                    }
                }
                if !curtainShot {
                    // Its detail line comes after a second.
                    if !from.isEmpty { keep(app, "\(name)-starting-screen"); curtainShot = true }
                }
            }
            let notice = texts.first { $0.hasPrefix("Playing from ") || $0.hasPrefix("Audio is now") }
            if let notice {
                if let source = Self.name(after: "Playing from ", in: notice) {
                    named.insert(source.components(separatedBy: " at ").first ?? source)
                }
                if !noticeShot { keep(app, "\(name)-notice"); noticeShot = true }
            }
            // Landed: the notice is up and the starting screen has gone for a second.
            if noticeShot, !curtain {
                if settledSince == nil { settledSince = Date() }
                if Date().timeIntervalSince(settledSince!) > 1 { break }
            } else {
                settledSince = nil
            }
            Thread.sleep(forTimeInterval: 0.15)
        }
        XCTAssertTrue(noticeShot, "\(name): no notice that the switch landed")
        if !curtainShot { keep(app, "\(name)-after") }
        XCTAssertEqual(named.subtracting([expected]), [], "\(name): names other than \(expected) shown")
        XCTAssertEqual(playingNow(app, "\(name)-sources"), expected, "\(name): Playing now")
        click(app, "Close")
    }

    // MARK: The panel

    /// The panel on its Sources tab, wherever it was.
    private func openSources(_ app: XCUIApplication) {
        if !app.radioButtons["Sources"].firstMatch.exists {
            wake(app)
            click(app, "Episodes")
        }
        let sources = app.radioButtons["Sources"].firstMatch
        XCTAssertTrue(sources.waitForExistence(timeout: 10), "no Sources tab")
        sources.click()
        XCTAssertTrue(
            app.staticTexts["Playing now"].firstMatch.waitForExistence(timeout: 10), "the Sources tab did not open")
        // Its releases, listed once the engine has them.
        Thread.sleep(forTimeInterval: 1.5)
    }

    /// The name Playing now gives, read from the Sources tab, which is kept as `shot`.
    private func playingNow(_ app: XCUIApplication, _ shot: String) -> String? {
        openSources(app)
        keep(app, shot)
        let texts = shownTexts(app)
        guard let heading = texts.firstIndex(of: "Playing now"), heading + 1 < texts.count else { return nil }
        return texts[heading + 1].components(separatedBy: ", ").first
    }

    /// A row of the Sources tab, by the start of its spoken label.
    private func row(_ app: XCUIApplication, _ title: String) -> XCUIElement {
        let row = app.descendants(matching: .any).matching(NSPredicate(format: "label BEGINSWITH %@", title))
            .element(boundBy: 0)
        XCTAssertTrue(row.waitForExistence(timeout: 10), "no row for \(title)")
        return row
    }

    // MARK: Reading the window

    /// Every text the window shows, labels and values, in the accessibility tree's order.
    private func shownTexts(_ app: XCUIApplication) -> [String] {
        guard let root = try? app.snapshot() else { return [] }
        var texts: [String] = []
        func walk(_ element: XCUIElementSnapshot) {
            if element.elementType == .staticText || element.elementType == .button || element.elementType == .group {
                for text in [element.label, element.value as? String ?? ""] where !text.isEmpty {
                    texts.append(text)
                }
            }
            element.children.forEach(walk)
        }
        walk(root)
        return texts
    }

    /// The name after `prefix` in `text`, up to the detail line's next fact.
    private static func name(after prefix: String, in text: String) -> String? {
        guard let range = text.range(of: prefix) else { return nil }
        let rest = text[range.upperBound...]
        return rest.components(separatedBy: " · ").first.map(String.init)
    }

    /// A screenshot of the main window, kept in the result bundle as `name`.
    private func keep(_ app: XCUIApplication, _ name: String) {
        let shot = XCTAttachment(screenshot: mainWindow(app).screenshot())
        shot.name = name
        shot.lifetime = .keepAlways
        add(shot)
    }

    // MARK: The app

    /// The Fixtures app on the Switching scene, the player up on episode 7 with its controls
    /// pinned.
    private func launch() -> XCUIApplication {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = [
            "-KaeriScene", "switching", "-KaeriPlayerShot", "playing",
            "-ApplePersistenceIgnoreState", "YES",
        ]
        let environment = ProcessInfo.processInfo.environment
        if let media = environment["KAERI_MEDIA"], !media.isEmpty {
            app.launchArguments += ["-KaeriMedia", media]
        }
        if let home = environment["KAERI_HOME_OVERRIDE"], !home.isEmpty {
            app.launchArguments += ["-KaeriHome", home]
            app.launchEnvironment["KAERI_HOME_OVERRIDE"] = home
        }
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 30))
        let up = app.buttons["Enter full screen"].firstMatch
        XCTAssertTrue(up.waitForExistence(timeout: 90), "the player did not come up")
        Thread.sleep(forTimeInterval: 1.5)
        return app
    }

    private func click(_ app: XCUIApplication, _ label: String) {
        let control = app.buttons[label].firstMatch
        XCTAssertTrue(control.waitForExistence(timeout: 10), "no \(label)")
        control.hover()
        control.click()
    }

    /// The pointer over the video, which brings the controls back.
    private func wake(_ app: XCUIApplication) {
        let window = mainWindow(app)
        window.coordinate(withNormalizedOffset: CGVector(dx: 0.4, dy: 0.45)).hover()
        window.coordinate(withNormalizedOffset: CGVector(dx: 0.42, dy: 0.5)).hover()
        Thread.sleep(forTimeInterval: 0.4)
    }

    private func mainWindow(_ app: XCUIApplication) -> XCUIElement {
        let windows = (try? app.snapshot())?.children.filter { $0.elementType == .window } ?? []
        let area = { (frame: CGRect) in frame.width * frame.height }
        guard let index = windows.enumerated().max(by: { area($0.element.frame) < area($1.element.frame) })?.offset
        else { return app.windows.firstMatch }
        return app.windows.element(boundBy: index)
    }
}
