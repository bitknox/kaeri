// The end page offline, with the next chapter only a source has: no next chapter to go on to,
// and the heading names that chapter, not the one just read.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct ReaderEndOfflineTests {
    private func row(_ number: String) -> ChapterRow {
        ChapterRow(
            id: ChapterId(number), label: number, state: .unread, number: number, title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
    }

    /// Chapter 12 open, with 12.5 listed after it.
    private func model(next: String?) -> ReaderModel {
        let model = ReaderModel()
        model.send = { _ in }
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: row("12"), mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("12"), page: 0, strip: nil, novel: nil),
            next: next.map(row), previous: nil, settings: nil)))
        return model
    }

    @Test func untilTheEnginesWordTheViewsNextChapterIsTheOneToGoOnTo() {
        #expect(model(next: "12.5").endNext?.id == ChapterId("12.5"))
    }

    @Test func offlineTheEndPageHasNoChapterToGoOnTo() {
        let model = model(next: "12.5")
        model.apply(.endPage(session: 1, next: nil, whyNone: .offline, nextSource: nil, skipTo: nil, extras: 0))
        #expect(model.endNext == nil)
        #expect(model.end?.why == .offline)
    }

    @Test func theHeadingNamesTheNextChapterListed() {
        #expect(ReaderEndPage.following(model(next: "12.5").view) == "Chapter 12.5")
    }

    @Test func pastTheLastChapterListedTheHeadingNamesTheNextWholeNumber() {
        #expect(ReaderEndPage.following(model(next: nil).view) == "Chapter 13")
    }
}
