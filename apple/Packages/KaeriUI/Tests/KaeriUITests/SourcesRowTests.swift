// Settings' Sources row speaks of what Kaeri is for (iPhone R8I-5: in Read only it said
// "Torrents off" and its page listed only the shows' settings).

import KaeriKit
@testable import KaeriStore
import KaeriStrings
@testable import KaeriUI
import Testing

@MainActor
struct SourcesRowTests {
    private func store(_ answer: KaeriIsFor) -> AppStore {
        let store = AppStore()
        store.apply(.medium(MediumView(answer: answer, edition: .full, fixed: false, readingAvailable: true, ask: nil, account: nil)))
        return store
    }

    private func server(_ id: String) -> BookServerRow {
        BookServerRow(id: id, kind: .kavita, name: "\(id).example", address: "https://\(id).example", series: 12,
                      reportProgress: true, off: false, signedOut: false, libraries: 1, empty: false, rechecksAt: nil,
                      keyLast4: nil, progressPending: 0, progressSending: false, progressSavedAt: nil,
                      progressProblem: nil, progressProblemAt: nil)
    }

    @Test func readingOnlyCountsTheBookSources() {
        let store = store(.read)
        store.apply(.bookSources(.view(BookSourcesView(servers: [server("shelf"), server("attic")]))))
        #expect(SourcesRowWords.value(store) == L.settingsSourceCount(count: 2))
        #expect(SourcesRowWords.value(store) != L.settingsTorrentsValueOff)
    }

    @Test func watchingSaysWhetherTorrentsAreOn() {
        let store = store(.watch)
        #expect(SourcesRowWords.value(store) == (store.settings.isOn(.torrents) ? L.settingsTorrentsValueOn : L.settingsTorrentsValueOff))
    }

    @Test func bothCountsEverySource() {
        let store = store(.both)
        store.apply(.bookSources(.view(BookSourcesView(servers: [server("shelf")]))))
        #expect(SourcesRowWords.value(store) == L.settingsSourceCount(count: store.providers.rows.count + 1))
    }
}
