// A chapter with no number, as a book in one file has its title for its one chapter: Read from
// counts its chapters rather than run a range to that name, and the end page names it.

import KaeriKit
@testable import KaeriUI
import Testing

@MainActor
struct UnnumberedChapterWordsTests {
    private func edition(chapters: UInt32, latest: String?) -> BookEditionRow {
        BookEditionRow(
            sourceId: "folder-1", sourceName: "Books", language: nil, versionLabel: nil, chapters: chapters,
            latest: latest, preferred: false, inUse: true, listing: .listed, groups: [], groupChosen: false)
    }

    private func chapter(_ label: String, number: String?) -> ChapterRow {
        ChapterRow(
            id: ChapterId(label), label: label, state: .unread, number: number, title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
    }

    @Test func aNumberedListRunsToItsNewestChapter() {
        #expect(ReadFromSheet.chapters(edition(chapters: 88, latest: "88"), first: true) == "Chapters 1 to 88")
        #expect(ReadFromSheet.chapters(edition(chapters: 13, latest: "12.5"), first: false) == "chapters 1 to 12.5")
        #expect(ReadFromSheet.chapters(edition(chapters: 60, latest: nil), first: true) == "Chapters 1 to 60")
    }

    @Test func aBookInOneFileCountsItsChapter() {
        #expect(ReadFromSheet.chapters(edition(chapters: 1, latest: "Snow Station Letters"), first: true) == "1 chapter")
        #expect(ReadFromSheet.chapters(edition(chapters: 40, latest: "The Last Lantern"), first: false) == "40 chapters")
    }

    @Test func theEndPageNamesAChapterWithNoNumber() {
        #expect(ReaderEndPage.ending(chapter("12", number: "12")) == "End of chapter 12")
        #expect(ReaderEndPage.ending(chapter("12.5", number: nil)) == "End of chapter 12.5")
        #expect(ReaderEndPage.ending(chapter("Snow Station Letters", number: nil)) == "End of Snow Station Letters")
    }
}
