// Every tooltip the main window could show, found without moving the pointer: what AppKit has
// registered for each view, and what SwiftUI's hosting views answer at each point. With
// KAERI_PROBE_TOOLTIPS=1 the harness lists them once it has staged its scene, and with the player
// up it counts those from the browsing under it, which should be none:
//
//     KAERI_PROBE_TOOLTIPS=1 swift run KaeriDev --scene playing -KaeriTitle 900001 \
//         --player-shot playing --snapshot /tmp/shot.png

import AppKit
import ObjectiveC

@MainActor
enum TooltipProbe {
    static var asked: Bool { ProcessInfo.processInfo.environment["KAERI_PROBE_TOOLTIPS"] != nil }

    /// One line per tooltip, its text and where in the window it shows. With the player up, the
    /// window's root hosting view is the player's: the browsing's own is out of its hit testing.
    /// Any other view's tooltip is the browsing's, shown through the player.
    static func run(window: NSWindow, playerUp: Bool) {
        guard let frame = window.contentView?.superview ?? window.contentView else { return }
        var views: [NSView] = []
        collect(frame, into: &views)
        var found: [String] = []
        var under: [String] = []
        for view in views where !view.isHiddenOrHasHiddenAncestor {
            var lines = registered(in: view) + dynamic(in: view)
            if view.className.contains("WKWebView") {
                lines.append("a web view, which shows its page's own tooltips, at \(rect(view.bounds, of: view))")
            }
            found += lines
            if view !== window.contentView { under += lines }
        }
        for line in unique(found) { print("KaeriDev: tooltip \(line)") }
        print("KaeriDev: \(unique(found).count) tooltips, \(playerUp ? unique(under).count : 0) under the player")
    }

    private static func unique(_ lines: [String]) -> [String] {
        var seen = Set<String>()
        return lines.filter { seen.insert($0).inserted }
    }

    private static func collect(_ view: NSView, into views: inout [NSView]) {
        views.append(view)
        for sub in view.subviews { collect(sub, into: &views) }
    }

    private static func rect(_ rect: NSRect, of view: NSView) -> String {
        let r = view.convert(rect, to: nil)
        return "(\(Int(r.minX)), \(Int(r.minY)), \(Int(r.width))×\(Int(r.height))) in \(String(describing: type(of: view)).prefix { $0 != "<" })"
    }

    /// The tooltips AppKit's tooltip manager keeps for the view: a `toolTip`, or the rects added
    /// with `addToolTip(_:owner:userData:)`.
    private static func registered(in view: NSView) -> [String] {
        guard let type = NSClassFromString("NSToolTipManager") as? NSObject.Type,
              type.responds(to: NSSelectorFromString("sharedToolTipManager")),
              let manager = type.perform(NSSelectorFromString("sharedToolTipManager"))?.takeUnretainedValue() as? NSObject,
              manager.responds(to: NSSelectorFromString("_toolTipsForView:")),
              let tips = manager.perform(NSSelectorFromString("_toolTipsForView:"), with: view)?.takeUnretainedValue() as? [NSObject]
        else { return [] }
        return tips.map { tip in
            var text = tip.value(forKey: "string") as? String
            let area = (tip.value(forKey: "trackingRect") as? NSValue)?.rectValue ?? view.bounds
            if text == nil, let owner = tip.value(forKey: "owner") as? NSViewToolTipOwner {
                text = owner.view(view, stringForToolTip: 0, point: NSPoint(x: area.midX, y: area.midY), userData: nil)
            }
            return "\"\(text ?? "?")\" registered at \(rect(area, of: view))"
        }
    }

    /// What a hosting view's dynamic tooltips say across its visible part, every 6 points.
    private static func dynamic(in view: NSView) -> [String] {
        let get = NSSelectorFromString("_dynamicToolTipManager")
        guard view.responds(to: get),
              let manager = view.perform(get)?.takeUnretainedValue() as? NSObject
        else { return [] }
        let ask = NSSelectorFromString("dynamicToolTipStringAtPoint:trackingRect:")
        guard let method = class_getInstanceMethod(type(of: manager), ask) else { return [] }
        typealias Ask = @convention(c) (AnyObject, Selector, NSPoint, UnsafeMutablePointer<NSRect>) -> Unmanaged<AnyObject>?
        let call = unsafeBitCast(method_getImplementation(method), to: Ask.self)
        var lines: [String] = []
        let visible = view.visibleRect
        for x in stride(from: visible.minX + 3, to: visible.maxX, by: 6) {
            for y in stride(from: visible.minY + 3, to: visible.maxY, by: 6) {
                var area = NSRect.zero
                guard let text = call(manager, ask, NSPoint(x: x, y: y), &area)?.takeUnretainedValue() as? String,
                      !text.isEmpty
                else { continue }
                lines.append("\"\(text)\" at \(rect(area, of: view))")
            }
        }
        return lines
    }
}
