// A flat chapter list, drawn with no volume headings, still offers Download by volume from the
// volume each chapter says it is in.

import KaeriKit
@testable import KaeriUI
import Testing

struct DownloadVolumesTests {
    private func row(_ n: Int, _ volume: String?) -> ChapterRow {
        ChapterRow(
            id: ChapterId("c\(n)"), label: "Chapter \(n)", state: .unread,
            number: "\(n)", title: nil, volume: volume, published: nil, pages: 20, newChapter: false,
            download: nil, versions: 1, bookmarked: false)
    }

    @Test func aFlatListOffersTheVolumesItsRowsSay() {
        let flat = VolumeGroup(chapters: [row(1, "1"), row(2, "1"), row(3, "2"), row(4, nil)], flat: true)
        let volumes = ReadSeriesWords.downloadVolumes([flat])
        #expect(volumes.map(\.volume) == ["1", "2"])
        #expect(volumes.map(\.chapters.count) == [2, 1])
        #expect(volumes.allSatisfy { !$0.flat })
    }

    @Test func groupedVolumesStayAsTheyAre() {
        let groups = [VolumeGroup(volume: "1", chapters: [row(1, "1")]), VolumeGroup(chapters: [row(2, nil)])]
        #expect(ReadSeriesWords.downloadVolumes(groups) == groups)
    }
}
