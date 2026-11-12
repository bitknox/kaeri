// A pinch the system cancels never calls its end, so the zoom must let it go another way, or
// every later drag is taken as the pinch's and the page never turns again.

import CoreGraphics
@testable import KaeriUI
import Testing

@MainActor
struct PageZoomTests {
    private let size = CGSize(width: 400, height: 800)

    @Test func aPageTurnLetsAPinchAtFitGo() {
        let zoom = PageZoom()
        zoom.pinch(1, at: CGPoint(x: 200, y: 400), in: size)
        #expect(zoom.pinching)
        zoom.reset()
        #expect(!zoom.pinching)
    }

    @Test func aCancelledPinchSettlesAndDragsPanAgain() {
        let zoom = PageZoom()
        zoom.pinch(2, at: CGPoint(x: 200, y: 400), in: size)
        #expect(zoom.pinching)
        zoom.pinchGone(in: size, motion: false)
        #expect(!zoom.pinching)
        #expect(zoom.zoomed)
        let before = zoom.pan
        zoom.drag(CGSize(width: 0, height: 40), in: size)
        zoom.drag(CGSize(width: 0, height: 80), in: size)
        #expect(zoom.pan != before)
    }

    @Test func aPinchThatEndedIsNotSettledTwice() {
        let zoom = PageZoom()
        zoom.pinch(2, at: CGPoint(x: 200, y: 400), in: size)
        zoom.pinchEnded(in: size, motion: false)
        let scale = zoom.scale
        zoom.pinchGone(in: size, motion: false)
        #expect(zoom.scale == scale)
        #expect(!zoom.pinching)
    }
}
