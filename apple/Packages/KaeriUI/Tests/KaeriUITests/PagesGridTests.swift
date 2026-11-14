#if os(macOS)
// The Pages grid has a cell for each half of a wide page split in two (iPad R7P-5: the halves
// shared the page's number as their id, so one drew grey and the next cell stayed empty). The
// grid is drawn in a window; each cell with no image yet asks for its page as it appears, so
// the asks say which cells drew.

@testable import KaeriStore
@testable import KaeriUI
import KaeriKit
import SwiftUI
import Testing

@MainActor
private final class Asked {
    var all: [Command] = []
    /// The pages the grid's cells asked for, in the order they appeared.
    var pages: [UInt32] {
        all.compactMap { if case .reader(.needPage(_, _, let page)) = $0 { page } else { nil } }
    }
}

extension WindowEventTests {
    @MainActor
    @Suite(.serialized)
    struct PagesGridTests {
        private let work = "a:900301"

        private func part(_ page: UInt32, _ half: PageHalf? = nil) -> LeafPart {
            LeafPart(page: page, kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1), rotate: false, half: half)
        }

        /// A manga chapter on `leaves`, every page still to come.
        private func chapter(_ leaves: [Leaf], asked: Asked) -> ReaderModel {
            let model = ReaderModel()
            model.send = { asked.all.append($0) }
            model.open(work: work, chapter: "c1")
            model.surfaceChanged(ReaderSurface(widthPx: 1400, heightPx: 1800, scale: 2, eink: false, twoPages: false, dark: true))
            let row = ChapterRow(
                id: ChapterId("c1"), label: "Chapter 1", state: .unread, number: "1", title: nil, volume: nil,
                published: nil, pages: 4, newChapter: false, download: nil, versions: 1, bookmarked: false)
            model.apply(.opened(ReaderView(
                session: 1, work: WorkId(work), title: "The Lantern Ferry", medium: .manga, chapter: row,
                mode: .paged, direction: .rightToLeft,
                start: ReadPlace(work: WorkId(work), chapter: ChapterId("c1"), page: 0, strip: nil, novel: nil),
                next: nil, previous: nil, settings: nil)))
            model.apply(.leaves(session: 1, chapter: ChapterId("c1"), leaves: leaves))
            return model
        }

        /// The pages the grid's cells ask for once it has drawn.
        private func drawn(_ leaves: [Leaf]) async throws -> [UInt32] {
            let asked = Asked()
            let model = chapter(leaves, asked: asked)
            asked.all = []
            let window = TapWindow(ReaderChaptersSheet(model: model, theme: .dark), size: CGSize(width: 520, height: 520))
            defer { window.close() }
            try await window.settle()
            return asked.pages.sorted()
        }

        @Test func eachHalfOfASplitPageIsACellOfItsOwn() async throws {
            // Pages 1 to 4, page 2 wide and split in two, read right to left: its right half first.
            let pages = try await drawn([
                Leaf(index: 0, parts: [part(0)], endPage: false),
                Leaf(index: 1, parts: [part(1, .right)], endPage: false),
                Leaf(index: 2, parts: [part(1, .left)], endPage: false),
                Leaf(index: 3, parts: [part(2)], endPage: false),
                Leaf(index: 4, parts: [part(3)], endPage: false),
                Leaf(index: 5, parts: [], endPage: true),
            ])
            #expect(pages == [0, 1, 1, 2, 3], "five cells drew, both halves of page 2 among them: \(pages)")
        }

        @Test func aSpreadsTwoPagesAreTwoCells() async throws {
            let pages = try await drawn([
                Leaf(index: 0, parts: [part(0)], endPage: false),
                Leaf(index: 1, parts: [part(2), part(1)], endPage: false),
                Leaf(index: 2, parts: [], endPage: true),
            ])
            #expect(pages == [0, 1, 2], "\(pages)")
        }
    }
}
#endif
