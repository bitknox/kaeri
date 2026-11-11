// Continue from the end page: the old end page is held over the next chapter until that
// chapter's first page has drawn, with a cue when the page is slow, and only then let go.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
struct ReaderHeldEndTests {
    private let session: UInt64 = 1

    private func row(_ id: String) -> ChapterRow {
        ChapterRow(
            id: ChapterId(id), label: "Chapter \(id)", state: .unread, number: nil, title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
    }

    private func view(_ chapter: String, next: String?, mode: ReaderMode = .paged) -> ReaderView {
        ReaderView(
            session: session, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: row(chapter), mode: mode, direction: .rightToLeft,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId(chapter), page: 0, strip: nil, novel: nil),
            next: next.map(row), previous: nil, settings: nil)
    }

    private func leaves(_ pages: Int) -> [Leaf] {
        var all = (0..<pages).map { page in
            Leaf(index: UInt32(page), parts: [
                LeafPart(page: UInt32(page), kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1), rotate: false, half: nil),
            ], endPage: false)
        }
        all.append(Leaf(index: UInt32(pages), parts: [], endPage: true))
        return all
    }

    /// Chapter 13 open on its end page.
    private func onEndPage(mode: ReaderMode = .paged) -> ReaderModel {
        let model = ReaderModel()
        model.send = { _ in }
        model.heldCue = .milliseconds(30)
        model.apply(.opened(view("13", next: "14", mode: mode)))
        model.apply(.leaves(session: session, chapter: ChapterId("13"), leaves: leaves(3)))
        model.shown(3, how: .turn)
        model.stageRead(trackers: ["AniList"])
        return model
    }

    /// A source's chapter laid out again once its pages have landed keeps the strip's place:
    /// the same image and how far down it, not the point the old heights put it at.
    @Test func aSecondLayoutKeepsTheStripsPlace() {
        let model = ReaderModel()
        model.send = { _ in }
        model.apply(.opened(view("13", next: "14", mode: .strip)))
        model.apply(.leaves(session: session, chapter: ChapterId("13"), leaves: leaves(3)))
        #expect(model.stripRelaid == 0, "the first layout puts the start place, not a kept one")
        let here = StripPos(image: 2, offset: 0.4)
        model.stripAt(here, footVisible: false)
        model.apply(.leaves(session: session, chapter: ChapterId("13"), leaves: leaves(3)))
        #expect(model.stripRestore == here)
        #expect(model.stripRelaid == 1)
        #expect(model.phase == .open)
    }

    /// Pages turn by leaf, so a second layout leaves them where they are.
    @Test func aSecondLayoutDoesNotMovePages() {
        let model = onEndPage()
        model.apply(.leaves(session: session, chapter: ChapterId("13"), leaves: leaves(3)))
        #expect(model.stripRelaid == 0)
        #expect(model.stripRestore == nil)
    }

    /// Back from a chapter's first page: the chapter before opens at its last page, not at its
    /// end page or its start.
    @Test func goingBackOpensTheChapterBeforeAtItsLastPage() {
        let model = ReaderModel()
        var sent: [Command] = []
        model.send = { sent.append($0) }
        model.apply(.opened(view("13", next: "14")))
        model.apply(.leaves(session: session, chapter: ChapterId("13"), leaves: leaves(3)))
        model.openChapter(ChapterId("12"), atEnd: true)
        #expect(sent.contains { if case .reader(.openChapter(_, ChapterId("12"))) = $0 { true } else { false } })
        #expect(model.held == nil, "a first page is not held as an end page is")
        model.apply(.opened(view("12", next: "13")))
        model.apply(.leaves(session: session, chapter: ChapterId("12"), leaves: leaves(3)))
        #expect(model.current == 2)
        #expect(!model.leaves[model.current].endPage)
        // A later open of that chapter starts as usual.
        model.openChapter(ChapterId("12"))
        model.apply(.opened(view("12", next: "13")))
        model.apply(.leaves(session: session, chapter: ChapterId("12"), leaves: leaves(3)))
        #expect(model.current == 0)
    }

    @Test func continueHoldsTheEndPageAsItWas() {
        let model = onEndPage()
        model.openChapter(ChapterId("14"))
        #expect(model.phase == .opening)
        let held = model.held
        #expect(held?.view.chapter.id == ChapterId("13"))
        #expect(held?.read == true)
        #expect(held?.readTrackers == ["AniList"])
        #expect(held?.landed == false)
        // The next chapter opens under it; nothing is drawn yet, so it stays.
        model.apply(.opened(view("14", next: "15")))
        model.apply(.leaves(session: session, chapter: ChapterId("14"), leaves: leaves(3)))
        #expect(model.held?.landed == false)
        #expect(model.held?.view.chapter.id == ChapterId("13"))
    }

    @Test func theHeldPageGoesOnceTheFirstPageHasDrawn() {
        let model = onEndPage()
        model.openChapter(ChapterId("14"))
        model.apply(.opened(view("14", next: "15")))
        model.apply(.leaves(session: session, chapter: ChapterId("14"), leaves: leaves(3)))
        // A neighbour drawing off screen is not the page that shows.
        model.pagePainted(chapter: "14", page: 1)
        #expect(model.held?.landed == false)
        model.pagePainted(chapter: "14", page: 0)
        #expect(model.held?.landed == true)
        let tick = model.held?.tick ?? -1
        model.releaseHeld(tick: tick + 1)
        #expect(model.held != nil)
        model.releaseHeld(tick: tick)
        #expect(model.held == nil)
    }

    @Test func aPageFromTheOldChapterLandsNothing() {
        let model = onEndPage()
        model.openChapter(ChapterId("14"))
        model.pagePainted(chapter: "13", page: 0)
        #expect(model.held?.landed == false)
    }

    @Test func aSlowFirstPageShowsTheCueOnTheHeldPage() async throws {
        let model = onEndPage()
        model.openChapter(ChapterId("14"))
        #expect(model.held?.slow == false)
        // The main actor is shared with the other suites: wait for the cue rather than a set time.
        let deadline = ContinuousClock.now + .seconds(3)
        while model.held?.slow != true, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(model.held?.slow == true)
        #expect(model.held?.landed == false)
    }

    @Test func aQuickFirstPageShowsNoCue() async throws {
        let model = onEndPage()
        model.openChapter(ChapterId("14"))
        model.apply(.opened(view("14", next: "15")))
        model.apply(.leaves(session: session, chapter: ChapterId("14"), leaves: leaves(3)))
        model.pagePainted(chapter: "14", page: 0)
        try await Task.sleep(for: .milliseconds(150))
        #expect(model.held?.slow == false)
        #expect(model.held?.landed == true)
    }

    @Test func aChapterThatFailsLetsTheHeldPageGo() {
        let model = onEndPage()
        model.openChapter(ChapterId("14"))
        model.apply(.opened(view("14", next: "15")))
        model.apply(.pageFailed(session: session, chapter: ChapterId("14"), page: 0, failure: .offline))
        #expect(model.phase == .failed(.offline))
        #expect(model.held?.landed == true)
    }

    @Test func aFirstPageThatFailsInPlaceLetsTheHeldPageGo() {
        let model = onEndPage()
        model.openChapter(ChapterId("14"))
        model.apply(.opened(view("14", next: "15")))
        model.apply(.leaves(session: session, chapter: ChapterId("14"), leaves: leaves(3)))
        model.apply(.pageFailed(session: session, chapter: ChapterId("14"), page: 0, failure: .offline))
        #expect(model.held?.landed == true)
    }

    @Test func aChapterPickedMidChapterHoldsNothing() {
        let model = onEndPage()
        model.shown(1, how: .turn)
        model.openChapter(ChapterId("14"))
        #expect(model.held == nil)
    }

    @Test func aStripHoldsNothing() {
        let model = onEndPage(mode: .strip)
        model.openChapter(ChapterId("14"))
        #expect(model.held == nil)
    }

    @Test func closingLetsTheHeldPageGo() {
        let model = onEndPage()
        model.openChapter(ChapterId("14"))
        model.close()
        #expect(model.held == nil)
    }

    /// The end page names the chapter past the extras, and the skip holds the end page while
    /// that chapter opens, as Continue does.
    @Test func theSkipPastExtrasOpensTheWholeChapterAfterThem() {
        let model = onEndPage()
        var sent: [Command] = []
        model.send = { sent.append($0) }
        model.apply(.endPage(session: session, next: row("13.5"), whyNone: nil, nextSource: nil, skipTo: row("14")))
        #expect(model.end?.next?.id == ChapterId("13.5"))
        #expect(model.end?.skipTo?.id == ChapterId("14"))
        model.openChapter(ChapterId("14"))
        #expect(sent.contains { if case .reader(.openChapter(_, ChapterId("14"))) = $0 { true } else { false } })
        #expect(model.held?.end?.skipTo?.id == ChapterId("14"))
        #expect(model.held?.to == ChapterId("14"))
    }
}

/// The bottom bar on the end page: the slider's numbers and the line never count past the
/// chapter's pages, wherever spreads were split into more leaves than pages.
@MainActor
struct ReaderEndPageNumberTests {
    private func model(pages: Int, split: Set<Int>) -> ReaderModel {
        let model = ReaderModel()
        model.send = { _ in }
        let place = UnitRect(x: 0, y: 0, width: 1, height: 1)
        var leaves: [Leaf] = []
        for page in 0..<pages {
            let halves: [PageHalf?] = split.contains(page) ? [.right, .left] : [nil]
            for half in halves {
                leaves.append(Leaf(index: UInt32(leaves.count), parts: [
                    LeafPart(page: UInt32(page), kind: .story, file: nil, place: place, rotate: false, half: half),
                ], endPage: false))
            }
        }
        leaves.append(Leaf(index: UInt32(leaves.count), parts: [], endPage: true))
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: ChapterRow(
                id: ChapterId("1"), label: "1", state: .unread, number: "1", title: nil, volume: nil,
                published: nil, pages: UInt32(pages), newChapter: false, download: nil, versions: 1, bookmarked: false),
            mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("1"), page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        model.apply(.leaves(session: 1, chapter: ChapterId("1"), leaves: leaves))
        return model
    }

    @Test func theEndPageCountsAsTheLastPage() {
        // 51 pages, four split in two: 55 leaves of pages, then the end page as leaf 55.
        let model = model(pages: 51, split: [10, 20, 30, 40])
        #expect(model.leaves.count == 56)
        #expect(model.pageCount == 51)
        #expect(model.isEndPage(55))
        #expect(model.pageNumber(of: 55) == 51)
        #expect(!model.isEndPage(54))
        #expect(model.pageNumber(of: 54) == 51)
        #expect(model.pageNumber(of: 11) == 11, "both halves of page 11 say 11")
        #expect(model.pageNumber(of: 12) == 12)
        #expect(model.pageNumber(of: 0) == 1)
        for leaf in 0..<model.leaves.count {
            #expect((1...51).contains(model.pageNumber(of: leaf)))
        }
        #expect(model.pageNumber(of: 80) == 51, "a leaf past the end is still the last page")
    }

}
