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

    @Test func zoomInStepsAboutTheMiddleAndFitReturnsToFitted() {
        let zoom = PageZoom()
        zoom.step(1, in: size, motion: false)
        #expect(zoom.zoomed)
        #expect(abs(zoom.scale - PageZoom.keyStep) < 0.001)
        #expect(zoom.pan == .zero, "the middle of the screen stays in the middle")
        #expect(zoom.detail == zoom.scale, "the page is decoded again at its zoom")
        zoom.step(0, in: size, motion: false)
        #expect(!zoom.zoomed)
        #expect(zoom.pan == .zero)
    }

    @Test func fitUndoesAPinchTheSystemCutOff() {
        let zoom = PageZoom()
        zoom.pinch(2.5, at: CGPoint(x: 300, y: 200), in: size)
        zoom.step(0, in: size, motion: false)
        #expect(!zoom.pinching)
        #expect(!zoom.zoomed)
        #expect(zoom.pan == .zero)
    }

    @Test func zoomOutAtFitStaysFittedAndZoomInStopsAtTheMost() {
        let zoom = PageZoom()
        zoom.step(-1, in: size, motion: false)
        #expect(zoom.scale == 1)
        for _ in 0..<20 { zoom.step(1, in: size, motion: false) }
        #expect(zoom.scale == PageZoom.most)
    }

    @Test func zoomingAPannedPageKeepsTheMiddleAndStaysOnScreen() {
        let zoom = PageZoom()
        zoom.doubleTap(at: CGPoint(x: 100, y: 200), in: size, motion: false)
        let pan = zoom.pan
        zoom.step(-1, in: size, motion: false)
        let ratio = zoom.scale / PageZoom.tapped
        #expect(abs(zoom.pan.width - pan.width * ratio) < 0.5)
        #expect(abs(zoom.pan.height - pan.height * ratio) < 0.5)
        #expect(abs(zoom.pan.width) <= (zoom.scale - 1) * size.width / 2 + 0.001)
    }

    @Test func aDoubleTapAfterACancelledPinchLetsItGo() {
        let zoom = PageZoom()
        zoom.pinch(2, at: CGPoint(x: 200, y: 400), in: size)
        zoom.doubleTap(at: CGPoint(x: 200, y: 400), in: size, motion: false)
        #expect(!zoom.pinching)
        #expect(!zoom.zoomed)
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
