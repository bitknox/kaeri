// The iPhone's pinned filter row: when Clear all leads it, what the filter pill counts and says,
// and which changes move the capsules.

import KaeriKit
import KaeriStrings
@testable import KaeriUI
import Testing

@Suite struct PinnedFilterStateTests {
    @Test func nothingSetShowsNoClearAllAndAGlyphAlone() {
        let state = PinnedFilterState(SearchQuery())
        #expect(!state.showsClearAll)
        #expect(state.filterCount == 0)
        #expect(state.filtersLabel == L.searchAllFilters)
    }

    @Test func aRowFilterShowsClearAllButNoCount() {
        // Genre and year are the row's own: they say themselves, and the pill counts the sheet's.
        let state = PinnedFilterState(SearchQuery(genres: ["Fantasy"], years: Bounds(min: 2026, max: 2026)))
        #expect(state.showsClearAll)
        #expect(state.filterCount == 0)
    }

    @Test func theSheetsFiltersAreTheFilterPillsCount() {
        let state = PinnedFilterState(SearchQuery(genres: ["Fantasy"], minScore: 70, licensedOnly: true))
        #expect(state.showsClearAll)
        #expect(state.filterCount == 2)
        #expect(state.filtersLabel == "\(L.searchAllFilters), \(L.searchFiltersSet(count: 2))")
    }

    @Test func theWordsAndTheSortAloneAreNotFilters() {
        let state = PinnedFilterState(SearchQuery(text: "lantern", sort: .score))
        #expect(!state.showsClearAll)
        #expect(state.filterCount == 0)
    }

    @Test func clearAllLeavesTheWordsAndTheSortAndEmptiesTheRow() {
        let set = SearchQuery(text: "lantern", sort: .score, genres: ["Fantasy"], minScore: 70)
        let cleared = set.cleared()
        #expect(cleared.text == "lantern")
        #expect(cleared.sort == SearchSort.score)
        let state = PinnedFilterState(cleared)
        #expect(!state.showsClearAll)
        #expect(state.filterCount == 0)
    }

    @Test func theLayoutMovesWithAFilterAndNotWithEachLetterTyped() {
        let base = SearchQuery(genres: ["Fantasy"])
        // Typing changes the layout only through the sort in effect: relevance once there are
        // words, popularity before. A longer word is the same row.
        let typed = SearchQuery(text: "lan", genres: ["Fantasy"])
        #expect(PinnedFilterState.Layout(typed) == PinnedFilterState.Layout(SearchQuery(text: "lantern", genres: ["Fantasy"])))
        #expect(PinnedFilterState.Layout(base) != PinnedFilterState.Layout(SearchQuery(genres: ["Fantasy", "Drama"])))
        #expect(PinnedFilterState.Layout(base) != PinnedFilterState.Layout(SearchQuery()))
        #expect(PinnedFilterState.Layout(base) != PinnedFilterState.Layout(SearchQuery(sort: .score, genres: ["Fantasy"])))
    }
}
