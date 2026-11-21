#if os(macOS)
@testable import KaeriUI
import AppKit
import SwiftUI
import Testing

/// A Mac toolbar item answers its size in whole points, as the toolbar lays it out, so the item's
/// hosting view holds it at the same width AppKit's layout gives it, and one layout pass settles.
@MainActor
struct ToolbarItemWidthTests {
    /// The width the hosting view holds the item at, and the width AppKit's layout gives it.
    private func widths(of item: some View) -> (held: CGFloat, laidOut: CGFloat) {
        let host = NSHostingView(rootView: item)
        return (host.intrinsicContentSize.width, host.fittingSize.width)
    }

    @Test func anItemBetweenWholePointsIsHeldAtTheNextWholePoint() {
        let (held, laidOut) = widths(of: Color.clear.frame(width: 301.5, height: 20).toolbarItemWidth(1))
        #expect(held == 302)
        #expect(held == laidOut)
    }

    @Test func aWidthPartWayThroughAnAnimationIsHeldAtWholePoints() {
        // A width the sync capsule's spring passes through.
        let (held, laidOut) = widths(of: Color.clear.frame(width: 97.318, height: 36).toolbarItemWidth(2))
        #expect(held == 98)
        #expect(held == laidOut)
    }

    @Test func aWholeWidthStaysAsItIs() {
        let (held, laidOut) = widths(of: Color.clear.frame(width: 132, height: 36).toolbarItemWidth(3))
        #expect(held == 132)
        #expect(held == laidOut)
    }

    @Test func theItemKeepsItsWidthAtTheFrameTheToolbarGivesIt() {
        // The toolbar sets the item to the width it lays it out at; measured there, it asks for
        // no other.
        let item = NSHostingView(rootView: Color.clear.frame(width: 131.5, height: 36).toolbarItemWidth(4))
        let width = item.fittingSize.width
        item.frame = CGRect(x: 0, y: 0, width: width, height: 36)
        item.layoutSubtreeIfNeeded()
        #expect(item.fittingSize.width == width)
        #expect(item.intrinsicContentSize.width == width)
    }
}
#endif
