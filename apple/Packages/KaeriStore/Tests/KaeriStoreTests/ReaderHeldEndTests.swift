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
}
