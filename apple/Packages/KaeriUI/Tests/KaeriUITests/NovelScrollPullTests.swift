// A scrolled novel chapter goes on by scrolling: a drag let go well past the foot turns forward,
// one well past the top turns back, and a drag that only reaches either end does nothing.

import CoreGraphics
@testable import KaeriUI
import Testing

struct NovelScrollPullTests {
    // A chapter 3 000 pt tall in an 800 pt view: its foot is at 2 200.
    private func pulled(_ offset: CGFloat, content: CGFloat = 3_000) -> Bool? {
        NovelWebController.pulledPast(offset: offset, content: content, view: 800)
    }

    @Test func pastTheFootGoesOn() {
        #expect(pulled(2_200 + 60) == true)
    }

    @Test func pastTheTopGoesBack() {
        #expect(pulled(-60) == false)
    }

    @Test func reachingAnEndIsNotATurn() {
        #expect(pulled(2_200) == nil)
        #expect(pulled(2_200 + 20) == nil)
        #expect(pulled(0) == nil)
        #expect(pulled(-20) == nil)
        #expect(pulled(1_000) == nil)
    }

    /// A document shorter than the view has its foot at its top.
    @Test func aShortDocumentGoesOnFromItsTop() {
        #expect(pulled(60, content: 300) == true)
        #expect(pulled(-60, content: 300) == false)
    }

    @Test func insetsMoveTheEnds() {
        #expect(NovelWebController.pulledPast(offset: -80, content: 3_000, view: 800, top: 40) == nil)
        #expect(NovelWebController.pulledPast(offset: 2_200 + 80, content: 3_000, view: 800, bottom: 40) == nil)
        #expect(NovelWebController.pulledPast(offset: 2_200 + 100, content: 3_000, view: 800, bottom: 40) == true)
    }
}
