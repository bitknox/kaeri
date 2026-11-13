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

    private func listAsks(_ box: Box) -> [LibrarySegmentRead] {
        box.commands.compactMap {
            if case .reading(.loadLibrary(let segment)) = $0 { segment } else { nil }
        }
    }

    /// The Library's reading lists are in the order things were last read: closing the reader
    /// asks again for the lists the Library asked for, so the series just read comes first.
    @Test func closingTheReaderAsksForTheReadingListsAgain() {
        let (model, box) = reading()
        model.loadList(.reading)
        model.loadList(.planning)
        model.observe(.reading(.library(segment: .reading, tiles: [])))
        model.observe(.reading(.library(segment: .planning, tiles: [])))
        box.commands.removeAll()
        model.observe(.reader(.opened(view())))
        model.observe(.reader(.closed(session: 1)))
        #expect(listAsks(box) == [.reading, .planning])
    }

    @Test func aBookWithNoPageOpenAsksForNothing() {
        let model = ReadingModel()
        let box = Box()
        model.send = { box.commands.append($0) }
        model.observe(.reader(.opened(view())))
        model.observe(.reader(.closed(session: 1)))
        #expect(asks(box) == 0)
    }
}
