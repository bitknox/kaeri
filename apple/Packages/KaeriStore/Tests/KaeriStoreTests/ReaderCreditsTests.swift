// Credit pages and parts in the reader model: what page 1 and the end page name, stepping onto
// and off a credit page brought back, Not a credit page with its Undo, and the scrubber's ticks.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
struct ReaderCreditsTests {
    final class Box { var commands: [Command] = [] }

    private let work = WorkId("a:900301")
    private let chapter = ChapterId("c3")
    private let file = PageFile(path: "/tmp/credit.png", encoding: .original(mime: "image/png"), width: 800,
                                height: 1200, crop: nil, colour: true, bytes: 1)

    /// A paged chapter of six kept pages, with the end page after them.
    private func opened() -> (ReaderModel, Box) {
        let model = ReaderModel()
        let box = Box()
        model.send = { box.commands.append($0) }
        let row = ChapterRow(
            id: chapter, label: "Chapter 3", state: .unread, number: nil, title: nil, volume: nil,
            published: nil, pages: 6, newChapter: false, download: nil, versions: 1, bookmarked: false)
        model.apply(.opened(ReaderView(
            session: 1, work: work, title: "The Lantern Ferry", medium: .manga, chapter: row,
            mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: work, chapter: chapter, page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        var leaves = (0..<6).map { page in
            Leaf(index: UInt32(page), parts: [
                LeafPart(page: UInt32(page), kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1),
                         rotate: false, half: nil),
            ], endPage: false)
        }
        leaves.append(Leaf(index: 6, parts: [], endPage: true))
        model.apply(.leaves(session: 1, chapter: chapter, leaves: leaves))
        return (model, box)
    }

    private func credits(_ model: ReaderModel, start: Bool = true, end: Bool = false) {
        var pages: [CreditPage] = []
        if start { pages.append(CreditPage(page: 0, atEnd: false, file: file, fingerprint: 11)) }
        if end { pages.append(CreditPage(page: UInt32(6 + pages.count), atEnd: true, file: file, fingerprint: 12)) }
        model.apply(.creditPages(session: 1, chapter: chapter, pages: pages, msg: .reading(.readerCreditPagesSkipped(count: Int64(pages.count)))))
    }

    @Test func pageOneNamesTheStartAndTheEndPageTheEnd() {
        let (model, _) = opened()
        credits(model, end: true)
        #expect(model.startSkipped.map(\.fingerprint) == [11])
        #expect(model.endSkipped(chapter).map(\.fingerprint) == [12])
        // The end credit sits after the six kept pages, on the end page's leaf.
        let end = model.chapterCredits(chapter)[1]
        #expect(model.anchor(of: end, in: chapter) == 6)
        #expect(model.creditLeaf(end) == 6)
        #expect(model.creditLeaf(model.chapterCredits(chapter)[0]) == 0)
    }

    @Test func showTakesPageOnesPlaceAndAForwardTurnLeavesIt() {
        let (model, _) = opened()
        credits(model)
        model.showStartCredits()
        #expect(model.onCredit == 0)
        #expect(model.startSkipped.isEmpty)
        #expect(model.notice?.local == .keepCredit)
        #expect(model.creditPlace?.n == 1 && model.creditPlace?.of == 1)
        // Forward goes to page 1 on the same leaf; the notice leaves with the credit page.
        #expect(model.creditStep(forward: true) == .leaf(0))
        model.showCredit(nil, leaf: 0, how: .turn)
        #expect(model.onCredit == nil)
        #expect(model.notice == nil)
        // Back from page 1 shows it again, since it was brought back.
        #expect(model.creditStep(forward: false) == .credit(0, leaf: 0))
    }

    @Test func notACreditPageCountsAtOnceAndUndoPutsItBack() {
        let (model, box) = opened()
        credits(model)
        model.showStartCredits()
        #expect(model.shownPageCount == 6)
        model.act(.keepCredit)
        #expect(model.keeping?.anchor == 0)
        #expect(model.shownPageCount == 7)
        #expect(model.shownPageNumber(of: 0) == 1)
        #expect(model.notice?.local == .undoKeep)
        model.undoKeep()
        #expect(model.keeping == nil)
        #expect(model.notice?.local == .keepCredit)
        #expect(!box.commands.contains { if case .reader(.notACreditPage) = $0 { true } else { false } })
    }

    private func sent(_ box: Box) -> [UInt64] {
        box.commands.compactMap { command -> UInt64? in
            if case .reader(.notACreditPage(_, let fingerprint)) = command { fingerprint } else { nil }
        }
    }

    @Test func undoOutlastsATurnAndTheEngineHearsWhenItRunsOut() {
        let (model, box) = opened()
        credits(model)
        model.showStartCredits()
        model.act(.keepCredit)
        model.showCredit(nil, leaf: 0, how: .turn)
        model.shown(1, how: .turn)
        #expect(model.keeping != nil)
        #expect(model.notice?.local == .undoKeep)
        #expect(sent(box).isEmpty)
        // Undo from the next page still puts it back.
        model.undoKeep()
        #expect(model.keeping == nil)
        #expect(sent(box).isEmpty)
        // Back on it and kept again; the notice running out tells the engine.
        model.showCredit(0, leaf: 0, how: .turn)
        model.act(.keepCredit)
        model.shown(1, how: .turn)
        model.commitKeep()
        #expect(sent(box) == [11])
    }

    @Test func undoLastsEightSecondsAndSixteenWithAScreenReader() {
        #expect(ReaderModel.keepHold(screenReader: false) == 8_000)
        #expect(ReaderModel.keepHold(screenReader: true) == 16_000)
    }

    @Test func twoShownCreditPagesInARowEachGetTheirNotice() {
        let (model, box) = opened()
        model.apply(.creditPages(session: 1, chapter: chapter, pages: [
            CreditPage(page: 0, atEnd: false, file: file, fingerprint: 11),
            CreditPage(page: 1, atEnd: false, file: file, fingerprint: 13),
        ], msg: .reading(.readerCreditPagesSkipped(count: 2))))
        model.showStartCredits()
        #expect(model.onCredit == 0)
        #expect(model.creditPlace?.n == 1 && model.creditPlace?.of == 2)
        model.act(.keepCredit)
        // On to the second credit page: the first one's Undo stays up.
        model.showCredit(1, leaf: 0, how: .turn)
        #expect(model.onCredit == 1)
        #expect(model.notice?.local == .undoKeep)
        #expect(model.keeping?.credit.fingerprint == 11)
        // Undo runs out: the first goes to the engine and the second gets Not a credit page.
        model.commitKeep()
        #expect(sent(box) == [11])
        #expect(model.notice?.local == .keepCredit)
        // A second keep while the first waits sends the first at once.
        model.act(.keepCredit)
        #expect(model.keeping?.credit.fingerprint == 13)
    }

    @Test func theLayoutWithTheKeptPageMovesTheLandedPagesUp() {
        let (model, _) = opened()
        credits(model)
        let page = PageFile(path: "/tmp/p0.png", encoding: .original(mime: "image/png"), width: 800, height: 1200,
                            crop: nil, colour: true, bytes: 1)
        model.apply(.pageReady(session: 1, chapter: chapter, page: 0, half: nil, file: page))
        model.showStartCredits()
        model.act(.keepCredit)
        // The reader is on another leaf when the keep commits and the layout lands.
        model.shown(2, how: .turn)
        model.commitKeep()
        #expect(model.keeping == nil)
        model.apply(.leaves(session: 1, chapter: chapter, leaves: model.leaves))
        #expect(model.pages[ReaderPageKey(chapter: chapter, page: 0)]?.path == file.path)
        #expect(model.pages[ReaderPageKey(chapter: chapter, page: 1)]?.path == page.path)
        #expect(model.chapterCredits(chapter).isEmpty)
        #expect(model.onCredit == nil)
    }

    @Test func eachLaterPartHasATickAndAMissingOneIsMarked() {
        let (model, _) = opened()
        model.apply(.chapterParts(session: 1, chapter: chapter, parts: [
            ChapterPart(number: 1, firstPage: 1, group: nil, otherGroup: false),
            ChapterPart(number: 3, firstPage: 5, group: nil, otherGroup: false),
        ], total: 3, missing: [2]))
        let ticks = model.partTicks
        #expect(ticks.map(\.part) == [3, 2])
        #expect(ticks.first?.leaf == 4)
        #expect(ticks.last?.missing == true)
        #expect(model.partStarting(atPage: 5) == 3)
        #expect(model.partStarting(atPage: 1) == nil)
        #expect(model.partMissing(beforePage: 5) == 2)
    }
}
