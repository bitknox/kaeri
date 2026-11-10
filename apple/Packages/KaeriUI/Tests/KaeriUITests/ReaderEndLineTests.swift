// The reader's bottom bar on the end page: "end of chapter", with the slider's numbers at the
// chapter's last page, never past it.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct ReaderEndLineTests {
    /// Chapter 1 of three pages, the middle one split in two, then its end page.
    private func model() -> ReaderModel {
        let model = ReaderModel()
        model.send = { _ in }
        let place = UnitRect(x: 0, y: 0, width: 1, height: 1)
        let part = { (page: UInt32, half: PageHalf?) in
            LeafPart(page: page, kind: .story, file: nil, place: place, rotate: false, half: half)
        }
        let leaves = [
            Leaf(index: 0, parts: [part(0, nil)], endPage: false),
            Leaf(index: 1, parts: [part(1, .right)], endPage: false),
            Leaf(index: 2, parts: [part(1, .left)], endPage: false),
            Leaf(index: 3, parts: [part(2, nil)], endPage: false),
            Leaf(index: 4, parts: [], endPage: true),
        ]
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: ChapterRow(
                id: ChapterId("1"), label: "Chapter 1", state: .unread, number: "1", title: nil, volume: nil,
                published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false),
            mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("1"), page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        model.apply(.leaves(session: 1, chapter: ChapterId("1"), leaves: leaves))
        return model
    }

    @Test func theEndPageSaysEndOfChapter() {
        let model = model()
        #expect(ReaderChrome.line(model: model, index: 4) == "Chapter 1 · end of chapter")
        #expect(ReaderChrome.line(model: model, index: 3) == "Chapter 1 · page 3 of 3")
    }

    @Test func theSliderEndsStayWithinThePages() {
        let model = model()
        #expect(ReaderChrome.ends(model: model, index: 4)?.at == 3)
        #expect(ReaderChrome.ends(model: model, index: 4)?.total == 3)
        #expect(ReaderChrome.ends(model: model, index: 2)?.at == 2)
    }
}
