// Your sources offers one Add a source: with nothing added, the empty state's own button; once a
// source is listed, the toolbar's. Mac, iPad and iPhone read the same rule.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct AddSourceOnceTests {
    private func store(_ answer: KaeriIsFor) -> AppStore {
        let store = AppStore()
        store.apply(.medium(MediumView(answer: answer, edition: .full, fixed: false, readingAvailable: true, ask: nil, account: nil)))
        return store
    }

    private func server() -> BookServerRow {
        BookServerRow(id: "s1", kind: .kavita, name: "shelf.example", address: "https://shelf.example", series: 12,
                      reportProgress: true, off: false, signedOut: false, libraries: 1, empty: false, rechecksAt: nil,
                      keyLast4: nil, progressPending: 0, progressSending: false, progressSavedAt: nil,
                      progressProblem: nil, progressProblemAt: nil)
    }

    @Test(arguments: [KaeriIsFor.read, .both])
    func anEmptyListLeavesAddASourceToItsEmptyState(_ answer: KaeriIsFor) {
        let store = store(answer)
        #expect(!BookSourcesToolbar.shows(store), "nothing said yet: nothing to add to")
        store.apply(.bookSources(.view(BookSourcesView())))
        #expect(store.bookSources.view != nil && store.bookSources.isEmpty, "the empty state shows its own button")
        #expect(!BookSourcesToolbar.shows(store), "so the toolbar has none")
    }

    @Test(arguments: [KaeriIsFor.read, .both])
    func aListedSourceBringsTheToolbarsAddASource(_ answer: KaeriIsFor) {
        let store = store(answer)
        store.apply(.bookSources(.view(BookSourcesView(servers: [server()]))))
        #expect(!store.bookSources.isEmpty)
        #expect(BookSourcesToolbar.shows(store))
    }

    @Test func watchingAloneHasNoAddASource() {
        let store = store(.watch)
        store.apply(.bookSources(.view(BookSourcesView(servers: [server()]))))
        #expect(!BookSourcesToolbar.shows(store))
    }
}
