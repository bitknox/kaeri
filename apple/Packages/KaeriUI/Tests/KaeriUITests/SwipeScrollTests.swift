// A two-finger swipe read from the trackpad's scroll events, as the Mac's paged reader takes it:
// sideways it is a drag the page follows, let go with the fingers' speed, and its momentum goes
// nowhere else. Up and down it is not the page's.

import CoreGraphics
@testable import KaeriUI
import Testing

struct SwipeScrollTests {
    /// A swipe of `steps` moves of `dx` and `dy`, 1/60 s apart, from `start` seconds.
    private func moves(
        _ swipe: inout SwipeScroll, steps: Int, dx: CGFloat, dy: CGFloat = 0, start: Double = 0
    ) -> [SwipeScroll.Out] {
        var out = [swipe.feed(phase: .began, momentum: false, dx: dx, dy: dy, at: start)]
        for n in 1..<steps {
            out.append(swipe.feed(phase: .changed, momentum: false, dx: dx, dy: dy, at: start + Double(n) / 60))
        }
        return out
    }

    @Test func aSidewaysSwipeIsADragThePageFollows() {
        var swipe = SwipeScroll()
        let out = moves(&swipe, steps: 10, dx: -12)
        #expect(out.first == .follow(-12))
        #expect(out.last == .follow(-120))
        let end = swipe.feed(phase: .ended, momentum: false, dx: 0, dy: 0, at: 10.0 / 60)
        guard case let .letGo(x, velocity) = end else {
            Issue.record("no let-go: \(end)")
            return
        }
        #expect(x == -120)
        // 12 points a sixtieth of a second, about 720 a second.
        #expect(velocity < -600 && velocity > -800, "velocity \(velocity)")
    }

    @Test func itsMomentumIsItsOwn() {
        var swipe = SwipeScroll()
        _ = moves(&swipe, steps: 6, dx: 10)
        _ = swipe.feed(phase: .ended, momentum: false, dx: 0, dy: 0, at: 0.1)
        #expect(swipe.feed(phase: .wheel, momentum: true, dx: 8, dy: 0, at: 0.12) == .take)
    }

    @Test func upAndDownIsNotThePages() {
        var swipe = SwipeScroll()
        let out = moves(&swipe, steps: 6, dx: 1, dy: -10)
        #expect(out.last == .pass)
        #expect(swipe.feed(phase: .ended, momentum: false, dx: 0, dy: 0, at: 0.1) == .pass)
        #expect(swipe.feed(phase: .wheel, momentum: true, dx: 0, dy: -8, at: 0.12) == .pass)
    }

    @Test func aSmallMoveWaitsForAWay() {
        var swipe = SwipeScroll()
        #expect(swipe.feed(phase: .began, momentum: false, dx: -2, dy: 1, at: 0) == .take)
        #expect(swipe.feed(phase: .changed, momentum: false, dx: -5, dy: 0, at: 0.02) == .follow(-7))
    }

    @Test func aCancelledSwipeGoesBack() {
        var swipe = SwipeScroll()
        _ = moves(&swipe, steps: 6, dx: -20)
        #expect(swipe.feed(phase: .cancelled, momentum: false, dx: 0, dy: 0, at: 0.1) == .letGo(0, velocity: 0))
    }

    @Test func aWheelsNotchSidewaysIsAStep() {
        var swipe = SwipeScroll()
        #expect(swipe.feed(phase: .wheel, momentum: false, dx: -3, dy: 0, at: 0) == .step(-3))
        #expect(swipe.feed(phase: .wheel, momentum: false, dx: 0, dy: -3, at: 0.1) == .pass)
    }
}
