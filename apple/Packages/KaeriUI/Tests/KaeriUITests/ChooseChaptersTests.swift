// Download's Choose chapters starts with the next ten unread chapters not downloaded yet, from
// the reader's place on, rather than every unread chapter.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Testing

struct ChooseChaptersTests {
    /// Thirty chapters, the first five read, the sixth half read and the eighth downloaded.
    private var chapters: ReadChapters {
        let rows = (1...30).map { n in
            ChapterRow(
                id: ChapterId("c\(n)"), label: "Chapter \(n)",
                state: n <= 5 ? .read : (n == 6 ? .inProgress(page: 4, of: 20) : .unread),
                number: "\(n)", title: nil, volume: nil, published: nil, pages: 20, newChapter: false,
                download: n == 8 ? .done : nil, versions: 1, bookmarked: false)
        }
        return ReadChapters(edition: nil, volumes: [VolumeGroup(chapters: rows)], failure: nil)
    }

    private func ids(_ numbers: some Sequence<Int>) -> Set<String> { Set(numbers.map { "c\($0)" }) }

    @Test func itStartsWithTheNextTenFromThePlace() {
        let chosen = SeriesChoosing.start(chapters, action: .continue(chapter: ChapterId("c6"), label: "Chapter 6", page: 4))
        #expect(chosen == ids([6, 7] + Array(9...16)))
    }

    @Test func withoutAnActionItStartsAfterTheLastChapterRead() {
        #expect(SeriesChoosing.start(chapters) == ids([6, 7] + Array(9...16)))
        let later = SeriesChoosing.start(chapters, action: .read(chapter: ChapterId("c25"), label: "Chapter 25"))
        #expect(later == ids(25...30))
    }

    @Test func selectAllStillTakesEveryUnreadChapter() {
        #expect(SeriesChoosing.unread(chapters) == ids(6...30))
    }
}
