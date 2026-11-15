// A chapter opening, or one that would not open, has the screen to itself: the reader's group,
// left up from the end page Continue was pressed on, stays away from Try again and Close, and so
// do its line, scrubber and Go to page for the chapter that was left, and the chapters sheet's
// pages of that chapter.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct ReaderFailedChromeTests {
    /// Chapter 23 open, with the group up.
    private func open() -> ReaderModel {
        let model = ReaderModel()
        model.send = { _ in }
        model.open(work: "a:900301", chapter: "23")
        let chapter = ChapterRow(
            id: ChapterId("23"), label: "23", state: .unread, number: "23", title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: chapter, mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("23"), page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        model.chromeShown = true
        return model
    }

    @Test func theGroupShowsOverAChapterThatOpened() {
        #expect(ReaderChrome.shows(open()))
    }

    @Test func aChapterThatFailsHasNoGroupOverItsButtons() {
        let model = open()
        model.observe(.reading(.failed(about: .reader, failure: .offline)))
        #expect(ReaderChrome.failed(model))
        #expect(model.chromeShown, "the model still asks for it, as the end page left it")
        #expect(!ReaderChrome.shows(model))
    }

    @Test func aScreenReaderOrThePointerDoesNotBringItBackOverTheFailure() {
        let model = open()
        model.observe(.reading(.failed(about: .reader, failure: .offline)))
        model.pinChrome = true
        model.chromeShown = true
        #expect(!ReaderChrome.shows(model))
    }

    @Test func aChapterStillOpeningHasNoGroupOverIt() {
        let model = open()
        model.openChapter(ChapterId("18"))
        #expect(!ReaderChrome.ready(model))
        model.chromeShown = true
        #expect(!ReaderChrome.shows(model), "the pointer or a tap does not bring it over the opening")
    }

    @Test func theChaptersSheetListsPagesOnlyForTheChapterOpen() {
        let model = open()
        #expect(ReaderChaptersSheet.hasPages(model))
        model.openChapter(ChapterId("18"))
        #expect(!ReaderChaptersSheet.hasPages(model), "the leaves are still chapter 23's")
        model.observe(.reading(.failed(about: .reader, failure: .offline)))
        #expect(!ReaderChaptersSheet.hasPages(model))
    }
}
