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

    /// A title no source plays answers in two lines, and its placeholder holds both once its
    /// episodes say so, so the answer lands without growing the room.
    @Test func theServicesLineHoldsTwoLinesOnceNoSourcePlaysTheTitle() {
        let store = AppStore()
        #expect(!LicensedLines.placeholderUnlisted(900_001, in: store), "before the episodes, one line")
        let row = EpisodeRow(number: "1", title: nil, thumbnail: nil, description: nil, durationSecs: nil,
                             watched: 0, completed: false, kind: nil, skippable: false, airsAt: nil,
                             delayedFrom: nil, metadataOnly: true)
        store.apply(.episodes(id: 900_001, rows: [row]))
        #expect(LicensedLines.placeholderUnlisted(900_001, in: store))
    }

    /// The row above the episodes is reserved when the watched run will fold. The dub line and
    /// the range menu sit in the heading, so they reserve nothing.
    @Test func theRowAboveTheEpisodesIsReservedWhenTheListSaysItComes() {
        #expect(!TitlePage.rowAboveEpisodes(entry { _ in }))
        #expect(!TitlePage.rowAboveEpisodes(entry { $0.dub = DubKnown(through: 4, complete: false) }))
        #expect(TitlePage.rowAboveEpisodes(entry { $0.progress = TitleProgress(done: 3, next: 4) }))
        #expect(!TitlePage.rowAboveEpisodes(entry { $0.episodes = 220 }))
    }
}
