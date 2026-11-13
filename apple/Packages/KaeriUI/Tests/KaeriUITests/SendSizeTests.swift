// The Send sheet's size: the downloaded chapters' share and the engine's estimate for the rest.
// The engine says 0 for a size it can't tell yet, which is unknown, never nothing to add.

@testable import KaeriUI
import KaeriKit
import Testing

struct SendSizeTests {
    @Test func anEstimateOfZeroIsUnknown() {
        #expect(SendWords.size(held: 0, of: 3, kept: nil, toFetch: 0) == nil)
        #expect(SendWords.size(held: 1, of: 3, kept: (bytes: 30_000_000, chapters: 3), toFetch: 0) == nil)
        #expect(SendWords.size(held: 0, of: 3, kept: nil, toFetch: nil) == nil)
    }

    @Test func knownPartsAddUp() {
        #expect(SendWords.size(held: 0, of: 3, kept: nil, toFetch: 45_000_000) == 45_000_000)
        #expect(SendWords.size(held: 1, of: 3, kept: (bytes: 30_000_000, chapters: 3), toFetch: 20_000_000) == 30_000_000)
        // Every chapter downloaded: no estimate is needed.
        #expect(SendWords.size(held: 2, of: 2, kept: (bytes: 30_000_000, chapters: 3), toFetch: nil) == 20_000_000)
    }

    private func row(_ id: String, number: String?, label: String) -> ChapterRow {
        ChapterRow(
            id: ChapterId(id), label: label, state: .unread, number: number, title: nil, volume: nil,
            published: nil, pages: 20, newChapter: false, download: nil, versions: 1, bookmarked: false)
    }

    /// A send's row names its series and its chapters, so two sends of one series tell apart.
    @Test func aSendRowNamesItsChapters() {
        let one = SendWords.rowChapters([row("c23", number: "23", label: "23")])
        #expect(one == "Chapter 23")
        #expect(SendWords.rowChapters([row("x", number: nil, label: "Oneshot")]) == "Oneshot")
        let three = SendWords.rowChapters([row("a", number: "21", label: "21"), row("b", number: "22", label: "22"), row("c", number: "23", label: "23")])
        #expect(three == "Chapters 21 to 23")
        #expect(SendWords.rowChapters([]) == nil)

        #expect(SendWords.rowLine(title: "The Lantern Ferry", chapters: one) == "The Lantern Ferry · Chapter 23")
        #expect(SendWords.rowLine(title: "The Lantern Ferry", chapters: nil) == "The Lantern Ferry")
        #expect(SendWords.rowLine(title: nil, chapters: three) == "Chapters 21 to 23")
        #expect(SendWords.rowLine(title: nil, chapters: nil) == nil)
    }
}
