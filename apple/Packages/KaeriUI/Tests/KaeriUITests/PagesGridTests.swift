// The Pages grid has a cell for each half of a wide page split in two, each with an id of its
// own (iPad R7P-5: the halves shared the page's number as their id, so one drew grey and the
// next cell stayed empty).

import KaeriKit
@testable import KaeriUI
import Testing

@MainActor
struct PagesGridTests {
    private func part(_ page: UInt32, _ half: PageHalf? = nil) -> LeafPart {
        LeafPart(page: page, kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1), rotate: false, half: half)
    }

    private func leaf(_ index: UInt32, _ parts: [LeafPart], end: Bool = false) -> Leaf {
        Leaf(index: index, parts: parts, endPage: end)
    }

    /// Pages 1 to 4, page 2 wide and split in two, read right to left: its right half first.
    private var leaves: [Leaf] {
        [
            leaf(0, [part(0)]),
            leaf(1, [part(1, .right)]),
            leaf(2, [part(1, .left)]),
            leaf(3, [part(2)]),
            leaf(4, [part(3)]),
            leaf(5, [], end: true),
        ]
    }

    @Test func eachHalfOfASplitPageIsACellOfItsOwn() {
        let list = ReaderChaptersSheet.pageList(leaves)
        #expect(list.map(\.number) == [1, 2, 2, 3, 4], "both halves carry the page's number")
        #expect(Set(list.map(\.id)).count == list.count, "every cell has an id of its own: \(list.map(\.id))")
    }

    @Test func theHalvesComeInTheOrderTheirLeavesRead() {
        let list = ReaderChaptersSheet.pageList(leaves)
        #expect(list.map(\.leaf) == [0, 1, 2, 3, 4])
        #expect(list[1].part.half == .right)
        #expect(list[2].part.half == .left)
    }

    @Test func aSpreadsTwoPagesAreTwoCellsOnOneLeaf() {
        let list = ReaderChaptersSheet.pageList([leaf(0, [part(0)]), leaf(1, [part(2), part(1)]), leaf(2, [], end: true)])
        #expect(list.map(\.number) == [1, 2, 3])
        #expect(list.map(\.leaf) == [0, 1, 1])
        #expect(Set(list.map(\.id)).count == 3)
    }
}
