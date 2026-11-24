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
        // The other version, which Lantern Index has from its local file, with no wait.
        try step(app, "3-dub", expected: "Lantern Index", dubbed: true) {
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
        var seen: [String] = []
        var curtainShot = false
        var noticeShot = false
        var curtainSeen = false
        let began = Date()
        var settledSince: Date?
        while Date().timeIntervalSince(began) < 30 {
            let texts = shownTexts(app)
            for text in texts where !seen.contains(text) { seen.append(text) }
            let from = texts.compactMap { Self.name(after: "From ", in: $0) }
            let waiting = texts.compactMap { Self.name(after: "Still waiting for ", in: $0) }
            named.formUnion(from + waiting)
            let curtain = !from.isEmpty
                || texts.contains { $0.contains("Opening the stream") || $0.contains("Buffering") }
            curtainSeen = curtainSeen || curtain
            // The starting screen's own version switch, with the panel's out of the way.
            if curtain, dubbed, !texts.contains("Playing now") {
                let sub = app.radioButtons["Subtitled"].firstMatch
                if sub.exists, (sub.value as? Int) == 1 {
                    named.insert("Subtitled on the starting screen")
                }
            }
            // Its detail line, with the name, comes after a second.
            if !from.isEmpty, !curtainShot { keep(app, "\(name)-starting-screen"); curtainShot = true }
            if let notice = texts.first(where: { $0.contains("Playing from ") || $0.contains("Audio is now") }) {
                if let source = Self.name(after: "Playing from ", in: notice) {
                    named.insert(source.components(separatedBy: " at ").first ?? source)
                }
                if !noticeShot { keep(app, "\(name)-notice"); noticeShot = true }
            }
            // Landed: the starting screen came and went, and has stayed away for two seconds, or
            // never came in 12 s.
            if !curtain, curtainSeen || Date().timeIntervalSince(began) > 12 {
                if settledSince == nil { settledSince = Date() }
                if Date().timeIntervalSince(settledSince!) > 2 { break }
            } else {
                settledSince = nil
            }
            Thread.sleep(forTimeInterval: 0.1)
        }
        let texts = XCTAttachment(string: seen.joined(separator: "\n"))
        texts.name = "texts-\(name)"
        texts.lifetime = .keepAlways
        add(texts)
        // The other version's file can show its first frame before the switch lands, with the
        // picture held over it all along: then no starting screen comes up to name anything.
        XCTAssertTrue(curtainSeen || dubbed, "\(name): the starting screen never came up over the switch's file")
        if !curtainShot { keep(app, "\(name)-after") }
        XCTAssertEqual(named.subtracting([expected]), [], "\(name): names other than \(expected) shown")
        XCTAssertEqual(playingNow(app, "\(name)-sources"), expected, "\(name): Playing now")
        if dubbed {
            XCTAssertEqual(app.radioButtons["Dubbed"].firstMatch.value as? Int, 1, "\(name): Dubbed is not picked")
        }
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
        XCTAssertTrue(waitFor { self.shownTexts(app).contains("Playing now") }, "the Sources tab did not open")
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
        // Any space in it may be a no-break one.
        let words = NSRegularExpression.escapedPattern(for: title).replacingOccurrences(of: " ", with: ".")
        let row = app.descendants(matching: .any).matching(NSPredicate(format: "label MATCHES %@", "(?s)\(words).*"))
            .element(boundBy: 0)
        XCTAssertTrue(row.waitForExistence(timeout: 10), "no row for \(title)")
        return row
    }

    // MARK: Reading the window

    /// Every text the app's windows show, labels and values, in the accessibility tree's order.
    private func shownTexts(_ app: XCUIApplication) -> [String] {
        guard let root = try? app.snapshot() else { return [] }
        var texts: [String] = []
        func walk(_ element: XCUIElementSnapshot) {
            // The panel keeps a heading's or a fact's words together with no-break spaces.
            for text in [element.label, element.value as? String ?? ""] where !text.isEmpty {
                texts.append(text.replacingOccurrences(of: "\u{00A0}", with: " "))
            }
            element.children.forEach(walk)
        }
        // The windows only: the menu bar's items are always there.
        root.children.filter { $0.elementType == .window }.forEach(walk)
        return texts
    }

    /// The name after `prefix` in `text`, up to the detail line's next fact.
    private static func name(after prefix: String, in text: String) -> String? {
        guard let range = text.range(of: prefix) else { return nil }
        let rest = text[range.upperBound...]
        return rest.components(separatedBy: " · ").first.map { String($0) }
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
        // A stale name in one step still leaves the next step's screens to look at.
        continueAfterFailure = true
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

    private func waitFor(timeout: TimeInterval = 10, _ condition: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return true }
            Thread.sleep(forTimeInterval: 0.2)
        }
        return condition()
    }

    private func mainWindow(_ app: XCUIApplication) -> XCUIElement {
        let windows = (try? app.snapshot())?.children.filter { $0.elementType == .window } ?? []
        let area = { (frame: CGRect) in frame.width * frame.height }
        guard let index = windows.enumerated().max(by: { area($0.element.frame) < area($1.element.frame) })?.offset
        else { return app.windows.firstMatch }
        return app.windows.element(boundBy: index)
    }
}
