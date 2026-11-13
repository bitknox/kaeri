// The Go menu follows the reader's model: on while a book is open and its window has the keys,
// its keys with no modifier only while no sheet or Go to page is up. It does not wait on where
// focus is in the window, which left the whole menu dimmed with a book open on the Mac.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct ReaderGoMenuTests {
    private func open() -> ReaderModel {
        let model = ReaderModel()
        model.send = { _ in }
        model.open(work: "a:900301", chapter: "12")
        let chapter = ChapterRow(
            id: ChapterId("12"), label: "12", state: .unread, number: "12", title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: chapter, mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("12"), page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        return model
    }

    @Test func noBookNoGoMenu() {
        #expect(ReaderInFront.of(ReaderModel()) == nil)
    }

    @Test func aBookOpenHasTheGoMenuWithItsKeys() {
        let front = ReaderInFront.of(open())
        #expect(front != nil)
        #expect(front?.plain == true)
        #expect(front?.keys.rtl == true)
    }

    @Test func aBookStillOpeningHasTheGoMenu() {
        let model = ReaderModel()
        model.send = { _ in }
        model.open(work: "a:900301", chapter: "12")
        #expect(ReaderInFront.of(model) != nil)
    }

    @Test func anotherWindowInFrontHasNoGoMenu() {
        let model = open()
        model.windowInFront = false
        #expect(ReaderInFront.of(model) == nil)
        model.windowInFront = true
        #expect(ReaderInFront.of(model) != nil)
    }

    @Test func aSheetOrGoToPageKeepsTheKeysWithNoModifier() {
        let model = open()
        model.sheet = .chapters
        #expect(ReaderInFront.of(model)?.plain == false)
        model.sheet = nil
        model.askingPage = true
        #expect(ReaderInFront.of(model)?.plain == false)
    }

    @Test func pagesHaveTheZoomItemsDimmed() {
        #expect(ReaderInFront.of(open())?.keys.zooms == false)
    }

    @Test func aStripAndANovelZoom() {
        let model = ReaderModel()
        model.send = { _ in }
        model.open(work: "a:900301", chapter: "12")
        let books: [(Medium, ReaderMode)] = [(.manga, .strip), (.novel, .paged)]
        for (medium, mode) in books {
            let chapter = ChapterRow(
                id: ChapterId("12"), label: "12", state: .unread, number: "12", title: nil, volume: nil,
                published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
            model.apply(.opened(ReaderView(
                session: 1, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: medium,
                chapter: chapter, mode: mode, direction: .leftToRight,
                start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("12"), page: 0, strip: nil, novel: nil),
                next: nil, previous: nil, settings: nil)))
            #expect(ReaderInFront.of(model)?.keys.zooms == true, "\(medium) \(mode)")
        }
    }

    @Test func closingTheBookTakesTheGoMenuAway() {
        let model = open()
        model.close()
        #expect(ReaderInFront.of(model) == nil)
    }
}
