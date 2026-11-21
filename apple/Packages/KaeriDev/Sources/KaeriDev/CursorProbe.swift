// The pointer's shape at points in the window, read without moving the pointer: at each point two
// mouse moves are posted to the app's own queue, as AppKit hands them to the window, and the
// cursor the app has set after them is read back.
//
//     swift run KaeriDev --cursor "poster=hand@120,200;gutter=arrow@40,700"
//
// Each point is `<name>=<shape>@<x>,<y>`, in points from the window content's top leading
// corner, with the shape the pointer should take there: arrow, hand or ibeam. Steps between the
// points move the scene on: `go=<section>` opens a sidebar section, `title=<id>` a title's page,
// `back` goes back (or leaves the player), `read=<work>` opens a book and `read=close` closes
// it, `click@<x>,<y>` clicks there and `type=<text>` types into whatever has the focus. The
// probe prints one line per point and KAERI_CURSOR_PASSED, or KAERI_CURSOR_FAILED, and the
// process exits 1.
//
// AppKit and WebKit also read where the pointer really is. KAERI_CURSOR_UNDER=1 moves the window
// so that the pointer, where it rests, is over each point before the moves are posted; a point
// the screen's edge keeps from the pointer is said. KAERI_CURSOR_AREAS=1 lists the tracking areas
// that hold each point, and KAERI_CURSOR_SHOTS=<dir> renders the window at each step.

import AppKit

@MainActor
enum CursorProbe {
    nonisolated enum Step: Equatable, Sendable {
        /// The shape wanted at a point.
        case point(name: String, want: String, at: CGPoint)
        case click(CGPoint)
        case section(String)
        case title(String)
        /// Back: the player closed, or the page before.
        case back
        /// A book opened in the reader, or the reader closed (`close`).
        case read(String)
        /// Keys typed into whatever has the focus.
        case type(String)
    }

    nonisolated static let shapes: Set<String> = ["arrow", "hand", "ibeam"]

    /// `<name>=<shape>@<x>,<y>;go=<section>;…`, as `--cursor` takes it.
    nonisolated static func parse(_ spec: String) -> [Step]? {
        func xy(_ text: Substring) -> CGPoint? {
            let parts = text.split(separator: ",").compactMap { Double($0) }
            return parts.count == 2 ? CGPoint(x: parts[0], y: parts[1]) : nil
        }
        let steps = spec.split(separator: ";").map { part -> Step? in
            if part == "back" { return .back }
            if part.hasPrefix("type=") { return .type(String(part.dropFirst(5))) }
            if part.hasPrefix("go=") { return .section(String(part.dropFirst(3))) }
            if part.hasPrefix("title=") { return .title(String(part.dropFirst(6))) }
            if part.hasPrefix("read=") { return .read(String(part.dropFirst(5))) }
            if part.hasPrefix("click@") { return xy(part.dropFirst(6)).map(Step.click) }
            guard let equals = part.firstIndex(of: "="), let at = part.firstIndex(of: "@"), equals < at,
                  let point = xy(part[part.index(after: at)...])
            else { return nil }
            let name = String(part[..<equals])
            let want = String(part[part.index(after: equals)..<at])
            guard !name.isEmpty, shapes.contains(want) else { return nil }
            return .point(name: name, want: want, at: point)
        }
        guard !steps.isEmpty, steps.allSatisfy({ $0 != nil }) else { return nil }
        return steps.compactMap { $0 }
    }

    /// Each step in turn, and whether every point showed the shape it wants. `navigate` takes the
    /// steps that move the scene on.
    static func run(_ steps: [Step], in window: NSWindow, navigate: (Step) async -> Void) async -> Bool {
        let environment = ProcessInfo.processInfo.environment
        let under = environment["KAERI_CURSOR_UNDER"] == "1"
        let shots = environment["KAERI_CURSOR_SHOTS"].map { URL(filePath: $0) }
        if !window.isKeyWindow {
            // Only the key window sets the pointer's shape: a reading here would say nothing.
            print("KaeriDev: the window is not key, so the pointer's shape is not the app's")
            print("KAERI_CURSOR_FAILED")
            return false
        }
        var passed = true
        let height = window.contentView?.bounds.height ?? 0
        func local(_ point: CGPoint) -> NSPoint { NSPoint(x: point.x, y: height - point.y) }
        // Somewhere neutral first, so the first point's move is a move into it.
        for nudge in [0.0, 1.0] {
            move(to: NSPoint(x: 1 + nudge, y: 1), in: window)
            try? await Task.sleep(for: .milliseconds(300))
        }
        for step in steps {
            switch step {
            case .type(let text):
                for character in text {
                    for type in [NSEvent.EventType.keyDown, .keyUp] {
                        if let event = NSEvent.keyEvent(
                            with: type, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                            windowNumber: window.windowNumber, context: nil, characters: String(character),
                            charactersIgnoringModifiers: String(character), isARepeat: false, keyCode: 0)
                        { NSApp.postEvent(event, atStart: false) }
                    }
                    try? await Task.sleep(for: .milliseconds(80))
                }
                try? await Task.sleep(for: .milliseconds(1500))
                print("KaeriDev: typed \(text)")
            case .click(let point):
                Clicks.click(at: local(point), in: window)
                try? await Task.sleep(for: .milliseconds(600))
                print("KaeriDev: clicked at \(Int(point.x)),\(Int(point.y))")
            case .section, .title, .back, .read:
                await navigate(step)
                try? await Task.sleep(for: .milliseconds(1500))
                print("KaeriDev: went to \(step)")
                if let shots {
                    try? Snapshot.write(window, to: shots.appending(path: "cursor-\(String(describing: step).filter(\.isLetter)).png"))
                }
            case .point(let label, let want, let at):
                // Two moves a point apart, each given time: SwiftUI sets its pointer style a
                // display cycle or more after the move that enters a view.
                for nudge in [0.0, 1.0] {
                    let target = NSPoint(x: local(at).x + nudge, y: local(at).y)
                    if under {
                        let mouse = NSEvent.mouseLocation
                        window.setFrameOrigin(NSPoint(x: mouse.x - target.x, y: mouse.y - target.y))
                        try? await Task.sleep(for: .milliseconds(100))
                    }
                    move(to: target, in: window)
                    try? await Task.sleep(for: .milliseconds(600))
                }
                if under {
                    let real = window.convertPoint(fromScreen: NSEvent.mouseLocation)
                    if abs(real.x - local(at).x - 1) > 2 || abs(real.y - local(at).y) > 2 {
                        print("KaeriDev:   the pointer is at \(Int(real.x)),\(Int(height - real.y)), not over the point")
                    }
                }
                if let shots { try? Snapshot.write(window, to: shots.appending(path: "cursor-\(label).png")) }
                let shown = name(of: NSCursor.current)
                let ok = shown == want
                if !ok { passed = false }
                if environment["KAERI_CURSOR_AREAS"] == "1" {
                    for line in areas(at: local(at), in: window) { print("KaeriDev:   \(line)") }
                }
                print("KaeriDev: cursor \(label) at \(Int(at.x)),\(Int(at.y)): \(shown)" + (ok ? "" : ", wants \(want)"))
            }
        }
        print(passed ? "KAERI_CURSOR_PASSED" : "KAERI_CURSOR_FAILED")
        return passed
    }

    /// Every tracking area in the window's views that holds the point, with its owner and options.
    /// An area in the visible rect reaches past its view's bounds unless an ancestor clips.
    static func areas(at point: NSPoint, in window: NSWindow) -> [String] {
        guard let root = window.contentView?.superview ?? window.contentView else { return [] }
        var lines: [String] = []
        func visit(_ view: NSView, depth: Int) {
            for area in view.trackingAreas where !view.isHiddenOrHasHiddenAncestor {
                let rect = area.options.contains(.inVisibleRect) ? view.visibleRect : area.rect
                guard view.convert(rect, to: nil).contains(point) else { continue }
                let options: [(NSTrackingArea.Options, String)] = [
                    (.cursorUpdate, "cursor"), (.mouseMoved, "moved"), (.mouseEnteredAndExited, "enter"),
                    (.activeAlways, "always"), (.activeInKeyWindow, "key"), (.activeInActiveApp, "app"),
                    (.inVisibleRect, "visible"),
                ]
                let flags = options.filter { area.options.contains($0.0) }.map(\.1).joined(separator: ",")
                let owner = area.owner.map { String(describing: type(of: $0)) } ?? "nil"
                lines.append("\(String(repeating: " ", count: depth))\(type(of: view)) [\(flags)] owner \(owner), "
                    + "view \(view.convert(view.bounds, to: nil).integral), area \(view.convert(rect, to: nil).integral)")
            }
            for sub in view.subviews { visit(sub, depth: depth + 1) }
        }
        visit(root, depth: 0)
        return lines
    }

    /// A mouse move to `point`, in the window's coordinates, posted as the window server sends one.
    static func move(to point: NSPoint, in window: NSWindow) {
        guard let event = NSEvent.mouseEvent(
            with: .mouseMoved, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 0, pressure: 0)
        else { return }
        NSApp.postEvent(event, atStart: false)
    }

    /// The shape's name, by the system cursor it is, or its image when another object holds it.
    static func name(of cursor: NSCursor) -> String {
        let known: [(String, NSCursor)] = [
            ("arrow", .arrow), ("hand", .pointingHand), ("ibeam", .iBeam),
            ("ibeam-vertical", .iBeamCursorForVerticalLayout), ("open-hand", .openHand),
            ("closed-hand", .closedHand), ("crosshair", .crosshair), ("not-allowed", .operationNotAllowed),
            ("link", .dragLink), ("copy", .dragCopy), ("menu", .contextualMenu),
        ]
        if let found = known.first(where: { $0.1 === cursor }) { return found.0 }
        let data = cursor.image.tiffRepresentation
        if let found = known.first(where: { $0.1.hotSpot == cursor.hotSpot && $0.1.image.tiffRepresentation == data }) {
            return found.0
        }
        return "other(\(cursor))"
    }
}
