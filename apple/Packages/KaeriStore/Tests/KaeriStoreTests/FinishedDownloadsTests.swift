// Clear Finished Downloads has something to clear only while a finished download is listed.

@testable import KaeriStore
import KaeriKit
import Testing

@MainActor
struct FinishedDownloadsTests {
    @Test func onlyAFinishedDownloadGivesSomethingToClear() {
        let model = DownloadsModel()
        #expect(!model.hasFinished)
        model.apply([download(1, "1", .active, done: 10), download(2, "2", .queued, done: 0), download(3, "3", .failed, done: 0)])
        #expect(!model.hasFinished)
        model.apply([download(1, "1", .done, done: 1_100_000_000), download(2, "2", .queued, done: 0)])
        #expect(model.hasFinished)
        model.apply([])
        #expect(!model.hasFinished)
    }
}
