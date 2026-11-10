// The series page's main button: "chapter" goes before a number, never before a chapter's own
// title.

import KaeriKit
@testable import KaeriUI
import Testing

struct SeriesActionWordsTests {
    private func words(_ action: SeriesAction) -> String? { ReadSeriesWords.action(action) }

    @Test func aNumberedChapterIsAChapter() {
        #expect(words(.read(chapter: ChapterId("12"), label: "12")) == "Read chapter 12")
        #expect(words(.read(chapter: ChapterId("10.5"), label: "10.5")) == "Read chapter 10.5")
        #expect(words(.continue(chapter: ChapterId("41"), label: "41", page: 8)) == "Continue chapter 41 · page 8")
    }

    @Test func aChapterWithNoNumberGoesByItsTitle() {
        #expect(words(.read(chapter: ChapterId("x-oneshot"), label: "Oneshot")) == "Read Oneshot")
        #expect(words(.read(chapter: ChapterId("x-lantern"), label: "The Lantern Festival")) == "Read The Lantern Festival")
        #expect(words(.continue(chapter: ChapterId("x-oneshot"), label: "Oneshot", page: 3)) == "Continue Oneshot · page 3")
        // A title that only starts with a digit is still a title.
        #expect(words(.read(chapter: ChapterId("x-anniv"), label: "1st Anniversary Extra")) == "Read 1st Anniversary Extra")
    }

    @Test func aPartWithNoNumberGoesByItsTitle() {
        #expect(words(.readPart(chapter: ChapterId("3"), label: "3", named: nil)) == "Read part 3")
        #expect(words(.readPart(chapter: ChapterId("x-prologue"), label: "Prologue", named: nil)) == "Read Prologue")
        #expect(words(.continuePart(chapter: ChapterId("x-prologue"), label: "Prologue", percent: 34, named: nil)) == "Continue Prologue · 34%")
    }
}
