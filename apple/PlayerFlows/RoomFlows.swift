// The room's frames that only a tap reaches: the More menu on iPhone, a system menu that no code
// can open. The room is staged as shots.sh stages it (playing:room=menu), then More is tapped and
// the screen kept.

import XCTest

@MainActor
final class RoomFlows: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    /// The upright room's More: Leave the room first, then Details, Copy invite link and Move to
    /// a new room.
    func testTheRoomsMoreMenu() throws {
        let app = launch(room: "menu")
        defer { app.terminate() }
        // Home's own More waits behind the player: the room's has an identifier of its own.
        let more = app.buttons["room-more"].firstMatch
        XCTAssertTrue(more.waitForExistence(timeout: 120), "the room did not come up with its More button")
        // Soon: the Fixtures episode is a minute long, and the room's scene starts 12 s in. A tap
        // before the room takes touches is lost, so it is tried again.
        let leave = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label BEGINSWITH %@", "Leave the room")).firstMatch
        let opened = open(more, showing: leave)
        Thread.sleep(forTimeInterval: 1)
        let picture = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        picture.name = "\(Self.form)-room-menu"
        picture.lifetime = .keepAlways
        add(picture)
        XCTAssertTrue(opened, "More did not open its menu")
    }

    /// The upright room's More stays open while the room goes on: a seek by someone else (which
    /// moves your player and posts a line) and a chat line both come 12 s after the scene is up.
    func testTheMoreMenuStaysOpenThroughASeek() throws { try menuStaysOpen("menuseek") }
    func testTheMoreMenuStaysOpenThroughAChatLine() throws { try menuStaysOpen("menusay") }

    private func menuStaysOpen(_ scene: String) throws {
        let app = launch(room: scene)
        defer { app.terminate() }
        let more = app.buttons["room-more"].firstMatch
        XCTAssertTrue(more.waitForExistence(timeout: 120), "the room did not come up with its More button")
        let leave = app.descendants(matching: .any)
            .matching(NSPredicate(format: "label BEGINSWITH %@", "Leave the room")).firstMatch
        XCTAssertTrue(open(more, showing: leave), "More did not open its menu")
        keep("\(Self.form)-\(scene)-open")
        // Past the event, which comes 12 s after the scene is up.
        Thread.sleep(forTimeInterval: 16)
        keep("\(Self.form)-\(scene)-after")
        XCTAssertTrue(leave.exists, "the menu closed during \(scene)")
    }

    /// Taps More until its menu shows. The tap goes to More's place on the screen through
    /// SpringBoard: a tap on the app's own element then waits for the app to go idle, which it
    /// doesn't while the menu is open over the playing room, so the wait outlasts the episode.
    /// An open menu hides what is behind it, More too, so it is tapped again only while it is
    /// still there to tap.
    private func open(_ more: XCUIElement, showing leave: XCUIElement) -> Bool {
        let screen = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        for _ in 0..<4 {
            if more.exists, more.isHittable {
                let frame = more.frame
                screen.coordinate(withNormalizedOffset: .zero)
                    .withOffset(CGVector(dx: frame.midX, dy: frame.midY))
                    .tap()
            }
            if leave.waitForExistence(timeout: 3) { return true }
        }
        return false
    }

    private func keep(_ name: String) {
        let picture = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        picture.name = name
        picture.lifetime = .keepAlways
        add(picture)
    }

    private func launch(room scene: String) -> XCUIApplication {
        let app = XCUIApplication()
        var arguments = ["-KaeriScene", "playing", "-KaeriRoom", scene]
        if let home = ProcessInfo.processInfo.environment["KAERI_HOME"], !home.isEmpty {
            let own = home + "/room-" + scene
            try? FileManager.default.createDirectory(atPath: own, withIntermediateDirectories: true)
            arguments += ["-KaeriHome", own]
            app.launchEnvironment["KAERI_HOME_OVERRIDE"] = own
        }
        XCUIDevice.shared.orientation = .portrait
        app.launchArguments = arguments
        app.launch()
        return app
    }

    static var form: String {
        UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
    }
}
