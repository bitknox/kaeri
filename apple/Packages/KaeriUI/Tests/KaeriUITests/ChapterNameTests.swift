// A chapter with no number goes by its own name on the end page: a book in one file is named for
// the book, so the page says "End of Rooftop Lantern", never "End of chapter Rooftop Lantern".
// Numbered chapters keep their words.

import KaeriKit
import KaeriStrings
@testable import KaeriUI
import Testing

@MainActor
struct ChapterNameTests {
    private func row(_ label: String, number: String?, title: String? = nil, pages: UInt32? = 34) -> ChapterRow {
        ChapterRow(
            id: ChapterId(label), label: label, state: .unread, number: number, title: title, volume: nil,
            published: nil, pages: pages, newChapter: false, download: nil, versions: 1, bookmarked: false)
    }

    @Test func anUnnumberedChapterEndsByItsOwnName() {
        #expect(ReaderEndPage.endOf(row("Rooftop Lantern", number: nil)) == L.readerEndOfNamed(name: "Rooftop Lantern"))
        #expect(ReaderEndPage.endOf(row("Chapter 12", number: "12")) == L.readerEndOfChapterNumber(chapter: "12"))
    }

    @Test func theNextChaptersFactsNeverCallANameAChapter() {
        // Titled and numbered: "Chapter 13 · 34 pages".
        let numbered = row("Chapter 13", number: "13", title: "The Ferry")
        #expect(ReaderEndPage.facts(numbered, end: nil) == L.readerEndFacts(chapter: "13", facts: L.readerEndPages(count: 34)))
        // Titled with no number: its length alone, as the heading already names it.
        let named = row("Salt Road", number: nil, title: "Salt Road")
        #expect(ReaderEndPage.facts(named, end: nil) == L.readerEndPages(count: 34))
        let bare = row("Salt Road", number: nil, title: "Salt Road", pages: nil)
        #expect(ReaderEndPage.facts(bare, end: nil) == "")
    }
}
