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
        var opened = false
        for _ in 0..<4 where !opened {
            more.tap()
            opened = app.buttons["Leave the room…"].waitForExistence(timeout: 3)
        }
        Thread.sleep(forTimeInterval: 1)
        let picture = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        picture.name = "\(Self.form)-room-menu"
        picture.lifetime = .keepAlways
        add(picture)
        XCTAssertTrue(opened, "More did not open its menu")
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
