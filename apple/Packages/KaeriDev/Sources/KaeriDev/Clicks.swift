// Clicks at points in the window, and the page scrolled, while frames are taken. A click is a
// press and a release posted to the app's own queue, as AppKit hands one to the window: the
// control's own hit testing and tracking run, and the pointer never moves.

import AppKit

@MainActor
enum Clicks {
    /// A click at `point`, or the page scrolled to `scroll`, `ms` after the frames start.
    nonisolated struct Step: Equatable, Sendable {
        let ms: Int
        /// Where to click, from the window content's top leading corner, in points.
        var point: CGPoint? = nil
        /// How far down to scroll the page, from its top, in points.
        var scroll: Double? = nil
    }

    /// `<ms>:@<x>,<y>;<ms>:scroll=<y>;…`, as `--clicks` takes it: "3000:@742,595;4500:@460,651"
    /// clicks twice, "4300:scroll=500" scrolls the page to 500 pt from its top.
    nonisolated static func parse(_ spec: String) -> [Step]? {
        let steps = spec.split(separator: ";").map { part -> Step? in
            guard let colon = part.firstIndex(of: ":"), let ms = Int(part[..<colon]) else { return nil }
            let rest = part[part.index(after: colon)...]
            if rest.hasPrefix("scroll="), let y = Double(rest.dropFirst("scroll=".count)) {
                return Step(ms: ms, scroll: y)
            }
            guard rest.hasPrefix("@") else { return nil }
            let xy = rest.dropFirst().split(separator: ",").compactMap { Double($0) }
            guard xy.count == 2 else { return nil }
            return Step(ms: ms, point: CGPoint(x: xy[0], y: xy[1]))
        }
        guard !steps.isEmpty, steps.allSatisfy({ $0 != nil }) else { return nil }
        return steps.compactMap { $0 }
    }

    /// Each step at its time from now, in order.
    static func run(_ steps: [Step], in window: NSWindow) async {
        let start = ContinuousClock.now
        for step in steps.sorted(by: { $0.ms < $1.ms }) {
            let at = start + .milliseconds(step.ms)
            if at > .now { try? await Task.sleep(until: at) }
            if let y = step.scroll, let scroll = ScrollClip.page(in: window) {
                let clip = scroll.contentView
                clip.scroll(to: NSPoint(x: clip.bounds.origin.x, y: -clip.contentInsets.top + y))
                scroll.reflectScrolledClipView(clip)
                print("KaeriDev: scrolled to \(Int(y)) (\(step.ms) ms)")
            }
            if let point = step.point {
                let height = window.contentView?.bounds.height ?? 0
                click(at: NSPoint(x: point.x, y: height - point.y), in: window)
                print("KaeriDev: clicked at \(Int(point.x)),\(Int(point.y)) (\(step.ms) ms)")
            }
        }
    }

    /// A press and a release at `point`, in the window's coordinates.
    static func click(at point: NSPoint, in window: NSWindow) {
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            if let event = NSEvent.mouseEvent(
                with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)
            {
                NSApp.postEvent(event, atStart: false)
            }
        }
    }
}
