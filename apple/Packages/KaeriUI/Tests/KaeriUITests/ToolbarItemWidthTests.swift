#if os(macOS)
@testable import KaeriUI
import AppKit
import SwiftUI
import Testing

/// A Mac toolbar item answers its size in whole points, as the toolbar lays it out, so the item's
/// hosting view and the toolbar agree on its frame after one layout pass.
@MainActor
struct ToolbarItemWidthTests {
    @Test func anItemBetweenWholePointsAnswersTheNextWholePoint() {
        let item = NSHostingView(rootView: Color.clear.frame(width: 301.5, height: 20.25).toolbarItemWidth(1))
        #expect(item.fittingSize == CGSize(width: 302, height: 21))
    }

    @Test func aWidthPartWayThroughAnAnimationAnswersWholePoints() {
        // A width the sync capsule's spring passes through.
        let item = NSHostingView(rootView: Color.clear.frame(width: 97.318, height: 36).toolbarItemWidth(2))
        #expect(item.fittingSize == CGSize(width: 98, height: 36))
    }

    @Test func aWholeWidthStaysAsItIs() {
        let item = NSHostingView(rootView: Color.clear.frame(width: 132, height: 36).toolbarItemWidth(3))
        #expect(item.fittingSize == CGSize(width: 132, height: 36))
    }

    @Test func theItemKeepsItsWidthAtTheFrameTheToolbarGivesIt() {
        // The toolbar sets the item to its whole width; measured there, it asks for no other.
        let item = NSHostingView(rootView: Color.clear.frame(width: 131.5, height: 36).toolbarItemWidth(4))
        let width = item.fittingSize.width
        item.frame = CGRect(x: 0, y: 0, width: width, height: 36)
        item.layoutSubtreeIfNeeded()
        #expect(item.fittingSize.width == width)
        #expect(item.intrinsicContentSize.width == width)
    }
}
#endif
