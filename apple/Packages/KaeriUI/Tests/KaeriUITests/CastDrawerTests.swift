// The cast drawer's motion: the spring it settles on and where a release sends it.

@testable import KaeriUI
import CoreGraphics
import Testing

struct CastDrawerTests {
    /// Steps a spring at 120 frames a second until it rests, with every value it passed.
    func run(_ spring: inout DrawerSpring, limit: Int = 240) -> [CGFloat] {
        var values: [CGFloat] = []
        for _ in 0..<limit {
            let done = spring.step(1.0 / 120)
            values.append(spring.value)
            if done { break }
        }
        return values
    }

    @Test func itOpensOnTheStandardSpringAndRestsOnItsEnd() {
        var spring = DrawerSpring(value: 0, target: 1)
        let values = run(&spring)
        #expect(spring.atRest)
        #expect(spring.value == 1)
        // Zero bounce: it only ever moves toward the end, and never past it.
        #expect(zip(values, values.dropFirst()).allSatisfy { $0 <= $1 })
        #expect(values.allSatisfy { $0 <= 1 })
        // The standard token's 0.3 s spring is there within 0.7 s.
        #expect(values.count <= 84)
    }

    @Test func aFlingHandsItsSpeedOnWithoutPassingTheEnd() {
        var spring = DrawerSpring(value: 0.6, velocity: 12, target: 1)
        let values = run(&spring)
        #expect(spring.value == 1)
        #expect(values.allSatisfy { $0 <= 1 })
        #expect(values.first.map { $0 > 0.6 } == true)
    }

    @Test func caughtOnItsWayItHoldsWhereItIs() {
        var spring = DrawerSpring(value: 1, target: 0)
        for _ in 0..<6 { spring.step(1.0 / 120) }
        let caught = spring.value
        #expect(caught > 0 && caught < 1)
        #expect(spring.velocity < 0)
        // The drawer takes the value as it stands: a grab starts here, with no jump.
        spring.target = 1
        spring.velocity = 0
        spring.step(1.0 / 120)
        #expect(spring.value >= caught)
    }

    @Test func aReleaseGoesTheWayOfAFlingElseToTheNearerEnd() {
        #expect(DrawerSpring.settles(at: 0.2, speed: 1.5) == 1)
        #expect(DrawerSpring.settles(at: 0.9, speed: -1.5) == 0)
        #expect(DrawerSpring.settles(at: 0.6, speed: 0.1) == 1)
        #expect(DrawerSpring.settles(at: 0.4, speed: -0.1) == 0)
        #expect(DrawerSpring.settles(at: 0.4, speed: 0) == 0)
    }

    @Test func aDragIsReadByItsLongerSide() {
        #expect(DrawerDrag(CGSize(width: 2, height: -10)) == .up)
        #expect(DrawerDrag(CGSize(width: -3, height: 12)) == .down)
        #expect(DrawerDrag(CGSize(width: 14, height: 6)) == .across)
    }
}
