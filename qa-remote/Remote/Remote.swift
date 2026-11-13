// QA only, never merged: an XCUITest remote for the round 6 iPad hunt. Reads command lines from
// $DIR/rcmd, appends what it did to $DIR/rlog and writes $DIR/rack when a batch is done.
import XCTest

final class Remote: XCTestCase {
    let dir = "/Users/kaeri/hunt-r6-ipad/rc"
    lazy var app = XCUIApplication(bundleIdentifier: "app.kaeri.qa")
    let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")

    func say(_ line: String) {
        let text = String(format: "%.3f ", Date().timeIntervalSince1970) + line + "\n"
        let url = URL(fileURLWithPath: dir + "/rlog")
        if let h = try? FileHandle(forWritingTo: url) {
            h.seekToEndOfFile(); h.write(text.data(using: .utf8)!); try? h.close()
        } else { try? text.write(to: url, atomically: true, encoding: .utf8) }
    }

    func at(_ x: Double, _ y: Double, _ base: XCUIElement? = nil) -> XCUICoordinate {
        (base ?? app).coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y))
    }

    func d(_ w: [String], _ i: Int, _ def: Double) -> Double { w.count > i ? Double(w[i]) ?? def : def }

    func keyFor(_ name: String) -> String {
        switch name {
        case "left": return XCUIKeyboardKey.leftArrow.rawValue
        case "right": return XCUIKeyboardKey.rightArrow.rawValue
        case "up": return XCUIKeyboardKey.upArrow.rawValue
        case "down": return XCUIKeyboardKey.downArrow.rawValue
        case "pgup": return XCUIKeyboardKey.pageUp.rawValue
        case "pgdn": return XCUIKeyboardKey.pageDown.rawValue
        case "home": return XCUIKeyboardKey.home.rawValue
        case "end": return XCUIKeyboardKey.end.rawValue
        case "esc": return XCUIKeyboardKey.escape.rawValue
        case "space": return XCUIKeyboardKey.space.rawValue
        case "return": return XCUIKeyboardKey.return.rawValue
        case "tab": return XCUIKeyboardKey.tab.rawValue
        case "del": return XCUIKeyboardKey.delete.rawValue
        default: return name
        }
    }

    func mods(_ s: String) -> XCUIElement.KeyModifierFlags {
        var m: XCUIElement.KeyModifierFlags = []
        for p in s.split(separator: "+") {
            switch p {
            case "cmd": m.insert(.command)
            case "shift": m.insert(.shift)
            case "opt": m.insert(.option)
            case "ctrl": m.insert(.control)
            default: break
            }
        }
        return m
    }

    func describe(_ e: XCUIElement) -> String {
        "\(e.elementType.rawValue) id='\(e.identifier)' label='\(e.label)' value='\(String(describing: e.value ?? ""))' frame=\(e.frame) hit=\(e.isHittable) en=\(e.isEnabled) sel=\(e.isSelected)"
    }

    func matching(_ text: String, exact: Bool, in root: XCUIElement) -> [XCUIElement] {
        let p = exact
            ? NSPredicate(format: "label == %@ OR identifier == %@", text, text)
            : NSPredicate(format: "label CONTAINS[c] %@ OR identifier CONTAINS[c] %@", text, text)
        return root.descendants(matching: .any).matching(p).allElementsBoundByIndex
    }

    func perform(_ op: String, _ w: [String]) {
        switch op {
        case "sleep": Thread.sleep(forTimeInterval: d(w, 0, 1000) / 1000)
        case "frame":
            say("app frame=\(app.frame) state=\(app.state.rawValue) windows=\(app.windows.allElementsBoundByIndex.map { $0.frame }) orient=\(XCUIDevice.shared.orientation.rawValue)")
        case "tap": at(d(w, 0, 0), d(w, 1, 0)).tap()
        case "dtap": at(d(w, 0, 0), d(w, 1, 0)).doubleTap()
        case "press": at(d(w, 0, 0), d(w, 1, 0)).press(forDuration: d(w, 2, 1))
        case "drag":
            at(d(w, 0, 0), d(w, 1, 0)).press(forDuration: d(w, 4, 0.05), thenDragTo: at(d(w, 2, 0), d(w, 3, 0)),
                                             withVelocity: XCUIGestureVelocity(CGFloat(d(w, 5, 1500))), thenHoldForDuration: d(w, 6, 0))
        case "sdrag":
            at(d(w, 0, 0), d(w, 1, 0), sb).press(forDuration: d(w, 4, 0.6), thenDragTo: at(d(w, 2, 0), d(w, 3, 0), sb),
                                                 withVelocity: XCUIGestureVelocity(CGFloat(d(w, 5, 600))), thenHoldForDuration: d(w, 6, 0.3))
        case "stap": at(d(w, 0, 0), d(w, 1, 0), sb).tap()
        case "swipe":
            switch w.first {
            case "l": app.swipeLeft()
            case "r": app.swipeRight()
            case "u": app.swipeUp()
            default: app.swipeDown()
            }
        case "pinch": app.pinch(withScale: CGFloat(d(w, 0, 2)), velocity: CGFloat(d(w, 1, 1)))
        case "rot":
            switch w.first {
            case "ll": XCUIDevice.shared.orientation = .landscapeLeft
            case "lr": XCUIDevice.shared.orientation = .landscapeRight
            case "pu": XCUIDevice.shared.orientation = .portraitUpsideDown
            default: XCUIDevice.shared.orientation = .portrait
            }
        case "key":
            app.typeKey(keyFor(w.first ?? "space"), modifierFlags: mods(w.count > 1 ? w[1] : ""))
        case "keys":
            // keys <name> <n> <gapms> [mods]
            for _ in 0..<Int(d(w, 1, 1)) {
                app.typeKey(keyFor(w.first ?? "space"), modifierFlags: mods(w.count > 3 ? w[3] : ""))
                Thread.sleep(forTimeInterval: d(w, 2, 500) / 1000)
            }
        case "type": app.typeText(w.joined(separator: " "))
        case "homebtn": XCUIDevice.shared.press(.home)
        case "activate": app.activate()
        case "settings": XCUIApplication(bundleIdentifier: "com.apple.Preferences").activate()
        case "tree":
            let name = w.first ?? "tree"
            try? app.debugDescription.write(toFile: dir + "/\(name).txt", atomically: true, encoding: .utf8)
            say("tree written \(name).txt")
        case "sbtree":
            try? sb.debugDescription.write(toFile: dir + "/\(w.first ?? "sbtree").txt", atomically: true, encoding: .utf8)
            say("sbtree written")
        case "find", "findx":
            let all = matching(w.joined(separator: " "), exact: op == "findx", in: app)
            say("find '\(w.joined(separator: " "))' \(all.count)")
            for e in all.prefix(25) { say("  " + describe(e)) }
        case "sbfind":
            let all = matching(w.joined(separator: " "), exact: false, in: sb)
            say("sbfind '\(w.joined(separator: " "))' \(all.count)")
            for e in all.prefix(25) { say("  " + describe(e)) }
        case "tapl", "tapc":
            // tapl <index> <label...>: tap the index-th element whose label is (tapl) or holds (tapc) the text
            let i = Int(w.first ?? "0") ?? 0
            let text = w.dropFirst().joined(separator: " ")
            let all = matching(text, exact: op == "tapl", in: app).filter { $0.isHittable }
            if all.indices.contains(i) { say("tapping " + describe(all[i])); all[i].tap() } else { say("no hittable '\(text)' (\(all.count))") }
        case "sbtapc":
            let text = w.joined(separator: " ")
            let all = matching(text, exact: false, in: sb).filter { $0.isHittable }
            if let e = all.first { say("tapping " + describe(e)); e.tap() } else { say("no sb '\(text)'") }
        case "wheel":
            // wheel <dx> <dy> [x y]
            let c = w.count > 3 ? at(d(w, 2, 0), d(w, 3, 0)) : app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            c.scroll(byDeltaX: CGFloat(d(w, 0, 0)), deltaY: CGFloat(d(w, 1, 0)))
        case "hover": at(d(w, 0, 0), d(w, 1, 0)).hover()
        case "audit":
            do {
                try app.performAccessibilityAudit { issue in
                    self.say("audit \(issue.auditType.rawValue) '\(issue.compactDescription)' el=\(issue.element.map { self.describe($0) } ?? "-")")
                    return true
                }
                say("audit done")
            } catch { say("audit error \(error)") }
        case "shot":
            let data = XCUIScreen.main.screenshot().pngRepresentation
            try? data.write(to: URL(fileURLWithPath: dir + "/\(w.first ?? "shot").png"))
            say("shot \(w.first ?? "shot")")
        case "ashot":
            let data = app.windows.firstMatch.screenshot().pngRepresentation
            try? data.write(to: URL(fileURLWithPath: dir + "/\(w.first ?? "ashot").png"))
            say("ashot \(w.first ?? "ashot")")
        default: say("unknown \(op)")
        }
    }

    func testServe() {
        continueAfterFailure = true
        executionTimeAllowance = 6 * 3600
        let cmd = URL(fileURLWithPath: dir + "/rcmd")
        say("remote up")
        let end = Date().addingTimeInterval(5 * 3600)
        while Date() < end, !FileManager.default.fileExists(atPath: dir + "/rstop") {
            if let text = try? String(contentsOf: cmd, encoding: .utf8) {
                try? FileManager.default.removeItem(at: cmd)
                for line in text.split(separator: "\n") {
                    let words = line.split(separator: " ").map(String.init)
                    guard let first = words.first, first != "#" else { continue }
                    say("> \(line)")
                    perform(first, Array(words.dropFirst()))
                }
                say("ack")
                try? "done".write(toFile: dir + "/rack", atomically: true, encoding: .utf8)
            }
            Thread.sleep(forTimeInterval: 0.15)
        }
        say("remote end")
    }
}
