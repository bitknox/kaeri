// The series page under the reader asks for itself again once a chapter is read or the reader
// closes: the engine sends nothing for either, and the main button would go on offering the
// chapter just read.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
struct SeriesAfterReadingTests {
    final class Box { var commands: [Command] = [] }

    private let work = "a:900301"

    private func reading() -> (ReadingModel, Box) {
        let model = ReadingModel()
        let box = Box()
        model.send = { box.commands.append($0) }
        // The series page is open on the book: its chapters came.
        model.observe(.reading(.chapters(work: WorkId(work), edition: nil, volumes: [], failure: nil)))
        return (model, box)
    }

    private func view() -> ReaderView {
        let row = ChapterRow(
            id: ChapterId("1"), label: "1", state: .unread, number: "1", title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
        return ReaderView(
            session: 1, work: WorkId(work), title: "The Lantern Ferry", medium: .manga,
            chapter: row, mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: WorkId(work), chapter: ChapterId("1"), page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)
    }

    private func asks(_ box: Box) -> Int {
        box.commands.filter {
            if case .reading(.loadSeries(let asked)) = $0 { asked == WorkId(work) } else { false }
        }.count
    }

    @Test func aChapterReadAsksForTheSeriesAgain() {
        let (model, box) = reading()
        model.observe(.reader(.opened(view())))
        #expect(asks(box) == 0)
        model.observe(.reader(.chapterRead(work: WorkId(work), chapter: ChapterId("1"), trackers: [])))
        #expect(asks(box) == 1)
    }

    @Test func closingTheReaderAsksForTheSeriesAgain() {
        let (model, box) = reading()
        model.observe(.reader(.opened(view())))
        model.observe(.reader(.closed(session: 1)))
        #expect(asks(box) == 1)
        // Once: a second close has no book.
        model.observe(.reader(.closed(session: 1)))
        #expect(asks(box) == 1)
    }

    @Test func aBookWithNoPageOpenAsksForNothing() {
        let model = ReadingModel()
        let box = Box()
        model.send = { box.commands.append($0) }
        model.observe(.reader(.opened(view())))
        model.observe(.reader(.closed(session: 1)))
        #expect(asks(box) == 0)
    }

    private func chapterAsks(_ box: Box) -> [String?] {
        box.commands.compactMap {
            if case .reading(.loadChapters(let asked, let edition)) = $0, asked == WorkId(work) { edition } else { nil }
        }
    }

    private func listed(_ model: ReadingModel, inUse: Bool) {
        let edition = BookEditionRow(sourceId: "pages.example", sourceName: "Pages", inUse: inUse)
        model.observe(.reading(.chapters(work: WorkId(work), edition: edition, volumes: [], failure: nil)))
    }

    /// Opening and closing a book asks its source for the list once, through the series: the
    /// edition it reads from is listed with it. Every close asked twice before, and a book opened
    /// and closed again and again left a queue of asks behind it.
    @Test func closingAsksForTheEditionInUseOnlyThroughTheSeries() {
        let (model, box) = reading()
        listed(model, inUse: true)
        model.observe(.reader(.opened(view())))
        model.observe(.reader(.closed(session: 1)))
        #expect(asks(box) == 1)
        #expect(chapterAsks(box).isEmpty)
    }

    /// The page showing another edition than the one in use asks for that one as well.
    @Test func closingOnAnotherEditionAsksForItToo() {
        let (model, box) = reading()
        listed(model, inUse: false)
        model.observe(.reader(.opened(view())))
        model.observe(.reader(.closed(session: 1)))
        #expect(chapterAsks(box) == ["pages.example"])
    }

    /// The whole store follows the reader once: a close asks for the series once and for the list
    /// of the edition in use not at all, and a chapter read asks once more.
    @Test func theStoreAsksOnceForAClose() {
        let store = AppStore()
        let box = Box()
        store.reading.send = { box.commands.append($0) }
        store.apply(.reading(.chapters(work: WorkId(work), edition: nil, volumes: [], failure: nil)))
        store.apply(.reader(.opened(view())))
        store.apply(.reader(.closed(session: 1)))
        #expect(asks(box) == 1)
        #expect(chapterAsks(box).isEmpty)
        store.apply(.reader(.chapterRead(work: WorkId(work), chapter: ChapterId("1"), trackers: [])))
        #expect(asks(box) == 2)
    }
}
