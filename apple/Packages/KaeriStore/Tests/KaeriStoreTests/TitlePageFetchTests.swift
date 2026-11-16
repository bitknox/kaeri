// A title page's fetch: whether its page came, so the page knows when to stop holding its
// placeholders and let the list's row stand.

@testable import KaeriStore
import KaeriKit
import Testing

@MainActor
struct TitlePageFetchTests {
    @Test func aFetchThatEndsWithoutThePageHasFailed() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries([entry(900_001, "Stardust Wayfarer")])))
        store.detailFetched(900_001, by: 41)
        #expect(!store.titles.failed.contains(900_001), "still on its way")
        // A season's header ahead of the page is not the page.
        store.apply(.detail(entry(900_001, "Stardust Wayfarer") { $0.detail = TitleDetail(partial: true) }))
        store.apply(.jobDone(41))
        #expect(store.titles.failed.contains(900_001))
        // Asked again, it is on its way again, and its page coming ends that.
        store.detailFetched(900_001, by: 42)
        #expect(!store.titles.failed.contains(900_001))
        store.apply(.detail(entry(900_001, "Stardust Wayfarer") { $0.detail = TitleDetail() }))
        store.apply(.jobDone(42))
        #expect(!store.titles.failed.contains(900_001), "the page came")
    }

    @Test func anotherJobEndingLeavesThePageWaiting() {
        let store = AppStore()
        store.detailFetched(900_002, by: 7)
        store.apply(.jobDone(8))
        #expect(!store.titles.failed.contains(900_002))
    }
}
