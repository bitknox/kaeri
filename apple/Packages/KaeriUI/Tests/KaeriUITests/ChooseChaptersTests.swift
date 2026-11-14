// Download's Choose chapters starts with the next ten unread chapters not downloaded yet, from
// the reader's place on, rather than every unread chapter, and opens centred on that place.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Testing

struct ChooseChaptersTests {
    /// Thirty chapters, the first five read, the sixth half read and the eighth downloaded.
    private var chapters: ReadChapters {
        shelf((1...30).map { n -> (Int, ChapterReadState) in (n, n <= 5 ? .read : (n == 6 ? .inProgress(page: 4, of: 20) : .unread)) })
    }

    private func shelf(_ states: [(Int, ChapterReadState)]) -> ReadChapters {
        let rows = states.map { n, state in
            ChapterRow(
                id: ChapterId("c\(n)"), label: "Chapter \(n)", state: state,
                number: "\(n)", title: nil, volume: nil, published: nil, pages: 20, newChapter: false,
                download: n == 8 ? .done : nil, versions: 1, bookmarked: false)
        }
        return ReadChapters(edition: nil, volumes: [VolumeGroup(chapters: rows)], failure: nil)
    }

    private func ids(_ numbers: some Sequence<Int>) -> Set<String> { Set(numbers.map { "c\($0)" }) }

    @Test func itStartsWithTheNextTenFromTheChapterInProgress() {
        #expect(SeriesChoosing.start(chapters) == ids([6, 7] + Array(9...16)))
        // A chapter read further on does not move the place past the one in progress.
        let skipped = shelf((1...30).map { n -> (Int, ChapterReadState) in (n, n == 6 ? .inProgress(page: 4, of: 20) : (n == 20 ? .read : .unread)) })
        #expect(SeriesChoosing.start(skipped) == ids([6, 7] + Array(9...16)))
    }

    @Test func withNothingInProgressItStartsAfterTheLastChapterRead() {
        let read = shelf((1...30).map { n -> (Int, ChapterReadState) in (n, n <= 24 ? .read : .unread) })
        #expect(SeriesChoosing.start(read) == ids(25...30))
    }

    @Test func withNothingUnreadAfterThePlaceItTakesTheFirstUnreadOnes() {
        let behind = shelf((1...30).map { n -> (Int, ChapterReadState) in (n, n <= 3 || (12...14).contains(n) ? .unread : .read) })
        #expect(SeriesChoosing.start(behind) == ids([1, 2, 3, 12, 13, 14]))
    }

    @Test func itOpensCentredOnTheReadersPlace() {
        #expect(SeriesChoosing.place(chapters) == "c6")
        let read = shelf((1...30).map { n -> (Int, ChapterReadState) in (n, n <= 24 ? .read : .unread) })
        #expect(SeriesChoosing.place(read) == "c25")
        // Nothing read yet, or everything read: no place, and the list opens where it is.
        let fresh = shelf((1...30).map { n -> (Int, ChapterReadState) in (n, .unread) })
        #expect(SeriesChoosing.place(fresh) == nil)
        let done = shelf((1...30).map { n -> (Int, ChapterReadState) in (n, .read) })
        #expect(SeriesChoosing.place(done) == nil)
    }

    @Test func selectAllStillTakesEveryUnreadChapter() {
        #expect(SeriesChoosing.unread(chapters) == ids(6...30))
    }

    /// A folder's chapters are files on this device: none is offered for download, Choose
    /// chapters starts with none of them, and the series' Download rests.
    private func folder(_ count: Int, downloadedToo: Bool = false) -> ReadChapters {
        let rows = (1...count).map { n in
            ChapterRow(
                id: ChapterId("c\(n)"), label: "Chapter \(n)", state: .unread,
                number: "\(n)", title: nil, volume: nil, published: nil, pages: 20, newChapter: false,
                download: downloadedToo && n == 1 ? .done : nil, versions: 1, bookmarked: false, onDevice: !(downloadedToo && n == 1))
        }
        return ReadChapters(edition: nil, volumes: [VolumeGroup(chapters: rows)], failure: nil)
    }

    @Test func chaptersOnTheDeviceOfferNoDownload() {
        let files = folder(5)
        #expect(files.volumes[0].chapters.allSatisfy { !$0.downloadable })
        #expect(SeriesChoosing.start(files).isEmpty)
        #expect(!SeriesChoosing.hasDownloads(files))
        // A chapter from a source beside them can be downloaded, and one downloaded can be deleted.
        #expect(SeriesChoosing.hasDownloads(chapters))
        #expect(SeriesChoosing.hasDownloads(folder(5, downloadedToo: true)))
        #expect(!SeriesChoosing.hasDownloads(nil))
    }
}
