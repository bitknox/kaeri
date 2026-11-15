// A title page opened from a list: what only its page brings waits under placeholders from the
// first frame, and the page replaces them the moment it lands, with no 600 ms wait and no hold.

import KaeriKit
import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct TitleHeldTests {
    @Test func aTitleOpenedFromItsListRowHoldsTheRestUntilItsPageLands() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries([entry { _ in }])))
        #expect(TitlePage.held(900_001, in: store), "only the list's row is here: placeholders from the first frame")
        store.apply(.detail(entry { $0.detail = detail() }))
        #expect(!TitlePage.held(900_001, in: store), "the page replaces them the moment it lands")
    }

    @Test func aSeasonsKeptHeaderShowsItsPageAtOnce() {
        let store = AppStore()
        var header = detail()
        header.partial = true
        store.apply(.detail(entry { $0.id = 920_002; $0.detail = header }))
        #expect(!TitlePage.held(920_002, in: store))
    }

    @Test func aTitleNothingIsKnownOfHoldsToo() {
        #expect(TitlePage.held(900_003, in: AppStore()))
    }

    /// The services line keeps its room until the answer, then shows it in that room, or closes
    /// the room when the answer has nothing to say.
    @Test func theServicesLineClosesOnlyForAnAnswerWithNothingToSay() {
        #expect(!LicensedLines.hasLines(TitleAvailability(id: 900_001, case: .playsHere)))
        #expect(LicensedLines.hasLines(TitleAvailability(id: 900_001, case: .playsHere, others: ["Lantern Library"]))) // i18n-skip
        #expect(LicensedLines.hasLines(TitleAvailability(id: 900_001, case: .notInSources, chooseServices: true)))
        #expect(!LicensedLines.hasLines(TitleAvailability(id: 900_001, case: .playsAlsoOn)))
    }
}
