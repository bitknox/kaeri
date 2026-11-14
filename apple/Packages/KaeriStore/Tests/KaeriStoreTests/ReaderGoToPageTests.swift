// Go to page submits from Return in the field and from Go alike. The Simulator never hands
// Return from a hardware keyboard to the app, so the audit test can't press it; these check
// what the submit does with what was typed.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
struct ReaderGoToPageTests {
    private let session: UInt64 = 1

    private func view(mode: ReaderMode) -> ReaderView {
        let chapter = ChapterRow(
            id: ChapterId("7"), label: "Chapter 7", state: .unread, number: nil, title: nil, volume: nil,
            published: nil, pages: 8, newChapter: false, download: nil, versions: 1, bookmarked: false)
        return ReaderView(
            session: session, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: chapter, mode: mode, direction: .rightToLeft,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("7"), page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)
    }

    private func part(_ page: UInt32) -> LeafPart {
        LeafPart(page: page, kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1), rotate: false, half: nil)
    }

    /// Eight pages with pages 2 and 3 on one spread, so a page's leaf comes before its number.
    private func leaves() -> [Leaf] {
        let groups: [[UInt32]] = [[0], [1], [2, 3], [4], [5], [6], [7]]
        var all = groups.enumerated().map { index, pages in
            Leaf(index: UInt32(index), parts: pages.map(part), endPage: false)
        }
        all.append(Leaf(index: UInt32(groups.count), parts: [], endPage: true))
        return all
    }

    private func asking(mode: ReaderMode) -> ReaderModel {
        let model = ReaderModel()
        model.send = { _ in }
        model.apply(.opened(view(mode: mode)))
        model.apply(.leaves(session: session, chapter: ChapterId("7"), leaves: leaves()))
        model.askingPage = true
        return model
    }

    @Test func fiveJumpsToTheLeafOfPageFiveAndCloses() {
        let model = asking(mode: .paged)
        #expect(model.goToPage(typed: "5"))
        #expect(model.jump?.index == 3, "page 5 is on the leaf after the spread")
        #expect(model.pageNumber(of: model.jump?.index ?? -1) == 5)
        #expect(!model.askingPage)
    }

    @Test func spacesAroundThePageAreFine() {
        let model = asking(mode: .paged)
        #expect(model.goToPage(typed: " 5 "))
        #expect(model.jump?.index == 3)
        #expect(!model.askingPage)
    }

    @Test func aPageThereIsNotLeavesThePromptOpen() {
        for typed in ["", "0", "9", "five", "-1"] {
            let model = asking(mode: .paged)
            #expect(!model.goToPage(typed: typed), "\(typed) is not a page")
            #expect(model.jump == nil)
            #expect(model.askingPage)
        }
    }

    @Test func aStripJumpsToTheImage() {
        let model = asking(mode: .strip)
        model.position = (index: 0, count: 8)
        #expect(model.goToPage(typed: "5"))
        #expect(model.jump?.index == 4)
        #expect(!model.askingPage)
    }
}
