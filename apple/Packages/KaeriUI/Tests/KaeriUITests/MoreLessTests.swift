// What a cut text offers: the title page's synopsis and the Mac's episode rows.

import Foundation
@testable import KaeriUI
import Testing

struct MoreLessTests {
    @Test func aCutTextOffersMore() {
        #expect(MoreLess.control(expanded: false, full: 120, shown: 60) == .more)
    }

    @Test func aTextThatFitsOffersNothing() {
        #expect(MoreLess.control(expanded: false, full: 60, shown: 60) == nil)
        // Rounding between the two measures is not a cut.
        #expect(MoreLess.control(expanded: false, full: 60.5, shown: 60) == nil)
    }

    @Test func theWholeTextAfterMoreOffersLess() {
        #expect(MoreLess.control(expanded: true, full: 120, shown: 120) == .less)
    }
}
