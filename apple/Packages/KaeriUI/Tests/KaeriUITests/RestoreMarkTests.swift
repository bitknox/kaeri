// A resumed strip scrolls to its place and keeps scrolling there as images land, until the
// person scrolls. A scroll with no finger on the strip (VoiceOver, the keyboard, the status bar)
// has no interacting phase, so the strip tells it from its own by where the scroll went.

import CoreGraphics
@testable import KaeriUI
import Testing

@MainActor
struct RestoreMarkTests {
    @Test func theRestoresOwnScrollIsNotThePersons() {
        let mark = RestoreMark()
        #expect(!mark.moved(to: 0, foot: 9000))
        mark.target = 4200
        #expect(!mark.moved(to: 4200, foot: 9000))
        // Pixel rounding.
        #expect(!mark.moved(to: 4200.4, foot: 9000))
    }

    @Test func aScrollToTheTopWithNoFingerIsThePersons() {
        let mark = RestoreMark()
        mark.target = 4200
        _ = mark.moved(to: 4200, foot: 9000)
        #expect(mark.moved(to: 0, foot: 9000))
    }

    @Test func rowsLandingAroundAStillScrollAreNotAScroll() {
        let mark = RestoreMark()
        mark.target = 4200
        _ = mark.moved(to: 4200, foot: 9000)
        // The image above came in taller: the scroll did not move, the restore has a new target.
        mark.target = 4500
        #expect(!mark.moved(to: 4200, foot: 9300))
        #expect(!mark.moved(to: 4500, foot: 9300))
    }

    @Test func aScrollHeldShortAtTheFootIsNotThePersons() {
        let mark = RestoreMark()
        mark.target = 4200
        _ = mark.moved(to: 3000, foot: 3000)
        // The rows below shrank: the scroll is held at the new foot, still short of the place.
        #expect(!mark.moved(to: 2800, foot: 2800))
        // They grew again and the restore reached the place.
        #expect(!mark.moved(to: 4200, foot: 9000))
        #expect(mark.moved(to: 1000, foot: 9000))
    }

    @Test func nothingIsThePersonsBeforeTheRestoreHasAPlace() {
        let mark = RestoreMark()
        _ = mark.moved(to: 0, foot: 9000)
        #expect(!mark.moved(to: 600, foot: 9000))
    }
}
