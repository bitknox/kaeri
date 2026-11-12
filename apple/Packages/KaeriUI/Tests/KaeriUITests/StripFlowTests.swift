// The strip holds the place on screen while rows change height, and the lead under a marker
// never holds it: when the next chapter's images come, they take the lead's room and the strip
// stays where it was, rather than running on past them to that chapter's end.

import CoreGraphics
import KaeriKit
@testable import KaeriUI
import Testing

@MainActor
struct StripFlowTests {
    private typealias Row = StripReader.Row

    private func chapter(_ id: String) -> ChapterRow {
        ChapterRow(
            id: ChapterId(id), label: id, state: .unread, number: id, title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
    }

    private func part(_ page: UInt32) -> LeafPart {
        LeafPart(page: page, kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1),
                 rotate: false, half: nil)
    }

    /// Rows one after another from the top, as the strip lays them out.
    private func stack(_ rows: [(String, Row.Kind, CGFloat, Int?)]) -> [Row] {
        var y: CGFloat = 0
        return rows.map { id, kind, height, trail in
            defer { y += height }
            return Row(id: id, kind: kind, top: y, height: height, trail: trail)
        }
    }

    /// Chapter 21 read to its marker, flowing into 22: its images, its marker, and the lead.
    private var flowing: [Row] {
        stack([
            ("21#0", .image(chapter: "21", part: part(0)), 1200, 0),
            ("21#1", .image(chapter: "21", part: part(1)), 1200, 0),
            ("21>", .marker(next: chapter("22"), source: nil, chapter: "21"), 168, 0),
            ("lead", .lead, 844, nil),
        ])
    }

    /// Chapter 22 opened under it with 27 images not landed yet, each at its guessed height.
    private var opened: [Row] {
        var rows: [(String, Row.Kind, CGFloat, Int?)] = [
            ("21#0", .image(chapter: "21", part: part(0)), 1200, 0),
            ("21#1", .image(chapter: "21", part: part(1)), 1200, 0),
            ("21>", .marker(next: chapter("22"), source: nil, chapter: "21"), 168, 0),
        ]
        for page in 0..<27 {
            rows.append(("22#\(page)", .image(chapter: "22", part: part(UInt32(page))), 585, nil))
        }
        rows.append(("22>", .marker(next: chapter("22.1"), source: nil, chapter: "22"), 168, nil))
        rows.append(("lead", .lead, 844, nil))
        return stack(rows)
    }

    /// Scrolled on into the lead while 22 opens: when its images come, the strip stays put, with
    /// 22's first images where the lead was.
    @Test func theNextChaptersImagesTakeTheLeadsRoom() throws {
        let y: CGFloat = 2568 + 300 // 300 points into the lead
        let anchor = try #require(StripReader.anchor(in: flowing, at: y))
        #expect(anchor.id == "21>", "the marker above the lead holds the place")
        #expect(StripReader.following(anchor, in: opened) == nil, "nothing above the place moved")
        let after = try #require(StripReader.anchor(in: opened, at: y))
        #expect(after.id == "22#0")
    }

    /// Images above the place that land at another height move it, and the strip follows by the
    /// same amount, past the marker's foot as well.
    @Test func rowsAboveThePlaceStillMoveIt() throws {
        let y: CGFloat = 2568 + 300
        let anchor = try #require(StripReader.anchor(in: flowing, at: y))
        let taller = stack([
            ("21#0", .image(chapter: "21", part: part(0)), 1500, 0),
            ("21#1", .image(chapter: "21", part: part(1)), 1200, 0),
            ("21>", .marker(next: chapter("22"), source: nil, chapter: "21"), 168, 0),
            ("lead", .lead, 844, nil),
        ])
        #expect(StripReader.following(anchor, in: taller) == y + 300)
    }

    /// Inside an image the place is a share of it, so it stays on the same part of the picture
    /// when the image above it lands taller.
    @Test func insideAnImageThePlaceIsAShareOfIt() throws {
        let anchor = try #require(StripReader.anchor(in: flowing, at: 1800))
        #expect(anchor.id == "21#1")
        #expect(anchor.into == 0.5)
        let taller = stack([
            ("21#0", .image(chapter: "21", part: part(0)), 1500, 0),
            ("21#1", .image(chapter: "21", part: part(1)), 1200, 0),
        ])
        #expect(StripReader.following(anchor, in: taller) == 2100)
    }
}
