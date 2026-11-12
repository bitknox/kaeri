// Zooming a page on iPhone and iPad: pinch zooms about the fingers, one finger pans the zoomed
// page, a double tap zooms in at the tapped point or returns to fit. While zoomed a tap never
// turns the page and a drag never turns it either. Turning the page fits it again.

import KaeriDesign
import SwiftUI

/// The zoom of the page on screen, and where the zoomed page has been moved to.
@MainActor @Observable
final class PageZoom {
    /// How far the page is zoomed: 1 is fitted.
    private(set) var scale: CGFloat = 1
    /// How far the zoomed page is moved from the centre, in points.
    private(set) var pan: CGSize = .zero
    /// A pinch is under way: a drag at the same time is the pinch's, not a turn or a pan.
    private(set) var pinching = false
    /// The zoom the page's pixels are decoded at: the zoom once a pinch or a double tap settles.
    private(set) var detail: CGFloat = 1

    @ObservationIgnored private var pinchFrom: (scale: CGFloat, point: CGPoint)?
    /// Where the page was, and how far the finger had gone, when this pan began: a finger left
    /// down after a pinch pans on from there.
    @ObservationIgnored private var panFrom: (pan: CGSize, translation: CGSize)?

    static let most: CGFloat = 4
    /// Where a double tap zooms to.
    static let tapped: CGFloat = 2.5

    var zoomed: Bool { scale > 1.01 }

    /// Back to fitted at once, as a page turn leaves it. A pinch under way is let go too.
    func reset() {
        pinchFrom = nil
        panFrom = nil
        if pinching { pinching = false }
        guard zoomed || pan != .zero else { return }
        scale = 1
        pan = .zero
        detail = 1
    }

    /// The pinch gesture is over, ended or cancelled by the system: a cancelled one never calls
    /// `pinchEnded`, so this settles it the same way and swipes turn again.
    func pinchGone(in size: CGSize, motion: Bool) {
        guard pinching || pinchFrom != nil else { return }
        pinchEnded(in: size, motion: motion)
    }

    /// A pinch moved: the point it started on stays under the fingers.
    func pinch(_ magnification: CGFloat, at start: CGPoint, in size: CGSize) {
        if pinchFrom == nil {
            pinching = true
            // The page point under the fingers, from the centre, at the zoom the pinch began at.
            let centre = CGPoint(x: size.width / 2, y: size.height / 2)
            pinchFrom = (scale, CGPoint(x: (start.x - centre.x - pan.width) / scale,
                                        y: (start.y - centre.y - pan.height) / scale))
        }
        guard let from = pinchFrom else { return }
        // A little past each end while the fingers are down, so the limit is felt.
        let to = min(max(from.scale * magnification, 0.85), Self.most * 1.1)
        let centre = CGPoint(x: size.width / 2, y: size.height / 2)
        scale = to
        pan = CGSize(width: start.x - centre.x - to * from.point.x, height: start.y - centre.y - to * from.point.y)
        if to >= 1 { pan = clamped(pan, scale: to, in: size) }
    }

    /// The pinch ended: back inside the limits, and fitted when hardly zoomed.
    func pinchEnded(in size: CGSize, motion: Bool) {
        pinchFrom = nil
        pinching = false
        panFrom = nil
        let to = min(max(scale, 1), Self.most)
        withAnimation(motion ? .standard : nil) {
            if to < 1.05 {
                scale = 1
                pan = .zero
            } else {
                scale = to
                pan = clamped(pan, scale: to, in: size)
            }
        }
        detail = scale
    }

    /// One finger moved the zoomed page.
    func drag(_ translation: CGSize, in size: CGSize) {
        guard zoomed, !pinching else { return }
        let from = panFrom ?? (pan, translation)
        panFrom = from
        pan = clamped(CGSize(width: from.pan.width + translation.width - from.translation.width,
                             height: from.pan.height + translation.height - from.translation.height),
                      scale: scale, in: size)
    }

    func dragEnded() { panFrom = nil }

    /// A double tap: in at the point, or back to fitted.
    func doubleTap(at point: CGPoint, in size: CGSize, motion: Bool) {
        withAnimation(motion ? .standard : nil) {
            if zoomed {
                scale = 1
                pan = .zero
            } else {
                let centre = CGPoint(x: size.width / 2, y: size.height / 2)
                let to = Self.tapped
                scale = to
                pan = clamped(CGSize(width: (1 - to) * (point.x - centre.x), height: (1 - to) * (point.y - centre.y)),
                              scale: to, in: size)
            }
        }
        detail = scale
    }

    /// The page's edges stay at or past the screen's: a zoomed page is never moved off it.
    private func clamped(_ pan: CGSize, scale: CGFloat, in size: CGSize) -> CGSize {
        let x = max((scale - 1) * size.width / 2, 0)
        let y = max((scale - 1) * size.height / 2, 0)
        return CGSize(width: min(max(pan.width, -x), x), height: min(max(pan.height, -y), y))
    }
}

extension View {
    /// The page drawn at the zoom, when it is the one on screen.
    func zoomed(_ zoom: PageZoom, on: Bool) -> some View {
        scaleEffect(on ? zoom.scale : 1)
            .offset(on ? zoom.pan : .zero)
    }
}
