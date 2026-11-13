// The opening screen names the chapter being opened, not the one on screen it is left from:
// "Opening chapter 3" after ] in chapter 2, and the chapter picked in the Chapters sheet.

@testable import KaeriStore
import KaeriKit
import Testing

@MainActor
struct ReaderOpeningChapterTests {
    private let session: UInt64 = 1

    private func row(_ id: String) -> ChapterRow {
        ChapterRow(
            id: ChapterId(id), label: "Chapter \(id)", state: .unread, number: id, title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
    }

    private func view(_ chapter: String, next: String?, previous: String? = nil) -> ReaderView {
        ReaderView(
            session: session, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: row(chapter), mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId(chapter), page: 0, strip: nil, novel: nil),
            next: next.map(row), previous: previous.map(row), settings: nil)
    }

    private func leaves(_ pages: Int) -> [Leaf] {
        (0..<pages).map { page in
            Leaf(index: UInt32(page), parts: [
                LeafPart(page: UInt32(page), kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1), rotate: false, half: nil),
            ], endPage: false)
        }
    }

    private func reading(_ chapter: String, next: String?, previous: String? = nil) -> ReaderModel {
        let model = ReaderModel()
        model.send = { _ in }
        model.apply(.opened(view(chapter, next: next, previous: previous)))
        model.apply(.leaves(session: session, chapter: ChapterId(chapter), leaves: leaves(3)))
        return model
    }

    @Test func theNextChapterIsNamedWhileItOpens() {
        let model = reading("2", next: "3")
        model.openChapter(ChapterId("3"))
        #expect(model.phase == .opening)
        #expect(model.view?.chapter.id == ChapterId("2"), "the chapter left stays the view until the next opens")
        #expect(model.openingChapter?.id == ChapterId("3"))
        #expect(model.openingChapter?.number == "3")
    }

    @Test func theChapterBeforeIsNamedGoingBack() {
        let model = reading("2", next: "3", previous: "1")
        model.openChapter(ChapterId("1"), atEnd: true)
        #expect(model.openingChapter?.id == ChapterId("1"))
    }

    /// A chapter picked from the whole list, far from the one on screen.
    @Test func aChapterPickedFromTheListIsNamed() {
        let model = reading("21.5", next: "22")
        model.observe(.reading(.chapters(
            work: WorkId("a:900301"), edition: nil,
            volumes: [VolumeGroup(chapters: [row("21.5"), row("22"), row("67")])], failure: nil)))
        model.openChapter(ChapterId("67"))
        #expect(model.openingChapter?.id == ChapterId("67"))
    }

    /// Once it opens, and before anything is asked, the chapter on screen is the one named.
    @Test func theOpenChapterIsNamedOnceItOpens() {
        let model = reading("2", next: "3")
        #expect(model.openingChapter?.id == ChapterId("2"))
        model.openChapter(ChapterId("3"))
        model.apply(.opened(view("3", next: "4")))
        #expect(model.openingChapter?.id == ChapterId("3"))
    }
}
