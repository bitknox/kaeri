// What the iPhone's pinned filter row shows for a query, apart from how it is drawn: whether Clear
// all leads it, what the filter pill counts and says, and what the row's layout depends on.

import KaeriKit
import KaeriStrings

struct PinnedFilterState: Equatable {
    /// The row's capsules, each keyed by what it filters, never by its place in the row.
    enum Pill: Hashable {
        case sort, genres, year, season, formats, statuses, available
    }

    /// Clear all leads the row once any filter is set. It is `cleared()`, which keeps the words
    /// and the sort.
    let showsClearAll: Bool
    /// What the sheet holds that the row's own capsules don't say: the filter pill's count.
    let filterCount: UInt32

    init(_ query: SearchQuery) {
        showsClearAll = !query.active().isEmpty
        filterCount = query.advancedCount()
    }

    /// Reading's row, whose query the engine words differently: the same row and pill.
    init(showsClearAll: Bool, filterCount: UInt32) {
        self.showsClearAll = showsClearAll
        self.filterCount = filterCount
    }

    /// The filter pill's spoken label: its name, then how many are set.
    var filtersLabel: String {
        filterCount == 0 ? L.searchAllFilters : "\(L.searchAllFilters), \(L.searchFiltersSet(count: Int(filterCount)))"
    }

    /// What decides the row's width and order. A change to it moves the capsules on standard; the
    /// words typed and the sheet's own filters, which the capsules don't show, change nothing.
    struct Layout: Hashable {
        let clearAll: Bool
        let words: [String]
        let sort: SearchSort

        init(_ query: SearchQuery) {
            clearAll = !query.active().isEmpty
            words = query.active().map(\.label)
            sort = query.sortInEffect()
        }
    }
}
