// A page scrolled by the harness, as a wheel scrolls it, for frames of what moves with it.

import AppKit

@MainActor
enum ScrollClip {
    /// The scroll view of the page in the middle of the window: the one the window's centre
    /// hits, or failing that the last one drawn there, which is the one on top.
    static func page(in window: NSWindow) -> NSScrollView? {
        guard let content = window.contentView else { return nil }
        let centre = NSPoint(x: content.bounds.midX, y: content.bounds.midY)
        var hit = content.hitTest(centre)
        while let view = hit {
            if let scroll = view as? NSScrollView, scroll.hasVerticalScroller || scroll.documentView != nil {
                return scroll
            }
            hit = view.superview
        }
        var found: [NSScrollView] = []
        func walk(_ view: NSView) {
            if let scroll = view as? NSScrollView, !scroll.isHiddenOrHasHiddenAncestor,
               scroll.convert(scroll.bounds, to: nil).contains(content.convert(centre, to: nil)) {
                found.append(scroll)
            }
            view.subviews.forEach(walk)
        }
        walk(content)
        return found.last
    }

    /// Where `path` puts the page `ms` after it starts: eased in and out between its stops.
    static func offset(at ms: Double, on path: [(ms: Int, y: Double)]) -> Double {
        guard let first = path.first, let last = path.last else { return 0 }
        if ms <= Double(first.ms) { return first.y }
        if ms >= Double(last.ms) { return last.y }
        for (from, to) in zip(path, path.dropFirst()) where ms <= Double(to.ms) {
            let t = (ms - Double(from.ms)) / Double(max(to.ms - from.ms, 1))
            let eased = t * t * (3 - 2 * t)
            return from.y + (to.y - from.y) * eased
        }
        return last.y
    }

    /// Move `scroll` along `path` once per display frame until its last stop, from its top
    /// (under the toolbar's inset).
    static func run(_ scroll: NSScrollView, path: [(ms: Int, y: Double)]) async {
        let clip = scroll.contentView
        let top = -clip.contentInsets.top
        let start = ContinuousClock.now
        let end = Double(path.last?.ms ?? 0)
        while true {
            let elapsed = ContinuousClock.now - start
            let ms = Double(elapsed.components.seconds) * 1000 + Double(elapsed.components.attoseconds) / 1e15
            clip.scroll(to: NSPoint(x: clip.bounds.origin.x, y: top + offset(at: ms, on: path)))
            scroll.reflectScrolledClipView(clip)
            if ms >= end { break }
            try? await Task.sleep(for: .milliseconds(8))
        }
    }
}
