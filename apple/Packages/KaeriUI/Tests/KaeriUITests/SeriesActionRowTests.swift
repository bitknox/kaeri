// The series page's action row on a wide page: the main action and its menus on one line while
// they fit, else the main action over the rest, and at the largest text sizes the rest wraps
// again instead of running off the page.

import CoreGraphics
@testable import KaeriUI
import Testing

struct SeriesActionRowTests {
    private func lines(_ widths: [CGFloat], _ width: CGFloat?) -> [[Int]] {
        SeriesActionRow.wrap(widths, width: width, spacing: 10)
    }

    @Test func everythingOnOneLineWhileItFits() {
        // Read chapter 12, Reading, Download and More at the default size: 170 + 110 + 120 + 44.
        #expect(lines([170, 110, 120, 44], 600) == [[0, 1, 2, 3]])
        #expect(lines([170, 110, 120, 44], nil) == [[0, 1, 2, 3]])
    }

    @Test func theMainActionGoesOverTheRest() {
        #expect(lines([170, 110, 120, 44], 400) == [[0], [1, 2, 3]])
    }

    /// AX XXXL on an upright iPad: the menus no longer fit beside one another under the main
    /// action, so Download and More go on under Reading rather than off the row.
    @Test func theRestWrapsAtTheLargestSizes() {
        #expect(lines([502, 232, 268, 44], 493) == [[0], [1], [2, 3]])
        #expect(lines([502, 232, 268, 44], 260) == [[0], [1], [2], [3]])
    }

    @Test func aRowOfOneIsOneLine() {
        #expect(lines([600], 300) == [[0]])
    }
}
