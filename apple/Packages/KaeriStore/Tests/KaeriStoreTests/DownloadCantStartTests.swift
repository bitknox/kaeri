// A download that can't start says so, and a chapter list refresh that fails keeps the list
// already shown (Mac R7-M10, Android R8-2: a failing source left no reason and no chapters).

@testable import KaeriStore
import KaeriKit
import KaeriStrings
import Testing

@MainActor
struct DownloadCantStartTests {
    private let work = "a:900321"

    private func group() -> VolumeGroup {
        let row = ChapterRow(
            id: ChapterId("c1"), label: "Chapter 1", state: .unread, number: "1", title: nil, volume: nil,
            published: nil, pages: 12, newChapter: false, download: nil, versions: 1, bookmarked: false)
        return VolumeGroup(chapters: [row])
    }

    @Test func aDownloadThatCantStartSaysWhy() throws {
        let store = AppStore()
        store.apply(.reading(.failed(about: .downloads, failure: .offline)))
        let notice = try #require(store.notices.current, "a notice says the download didn't start")
        #expect(notice.notice.text == L.readDownloadNotStarted(reason: ReadingFailure.offline.words))
        #expect(notice.isAlert)
    }

    @Test func aFailedRefreshKeepsTheChaptersShown() throws {
        let model = ReadingModel()
        model.observe(.reading(.chapters(work: WorkId(work), edition: nil, volumes: [group()], failure: nil)))
        model.observe(.reading(.chapters(work: WorkId(work), edition: nil, volumes: [], failure: .offline)))
        let chapters = try #require(model.chapters[work])
        #expect(chapters.volumes.flatMap(\.chapters).map(\.id) == [ChapterId("c1")], "the kept list stays")
        #expect(chapters.failure == .offline, "under why the refresh failed")
    }

    @Test func aListThatComesBackClearsTheFailure() throws {
        let model = ReadingModel()
        model.observe(.reading(.chapters(work: WorkId(work), edition: nil, volumes: [group()], failure: nil)))
        model.observe(.reading(.chapters(work: WorkId(work), edition: nil, volumes: [], failure: .offline)))
        model.observe(.reading(.chapters(work: WorkId(work), edition: nil, volumes: [group()], failure: nil)))
        #expect(model.chapters[work]?.failure == nil)
    }
}
