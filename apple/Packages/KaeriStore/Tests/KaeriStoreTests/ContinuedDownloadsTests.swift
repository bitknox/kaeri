// The downloads window's life: asked for when a download starts or the app leaves with some
// queued, reported to while open, closed when nothing is left, when it runs out or is stopped,
// and what the engine is told at each step. Chapters share the window with episodes. The
// system's task is a fake throughout.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

private func row(_ id: Int64, _ state: DownloadState, _ fraction: Double = 0) -> DownloadRow {
    DownloadRow(
        id: id, anilistId: 900_001, title: "Stardust Wayfarer", episode: String(id), state: state,
        fraction: fraction, downloaded: UInt64(fraction * 1_000), total: 1_000, error: nil, path: nil)
}

private func chapter(_ id: String, _ state: ChapterDownloadState) -> BookDownloadRow {
    BookDownloadRow(
        work: WorkId("a:900401"), title: "The Lantern Ferry",
        chapter: ChapterRow(
            id: ChapterId(id), label: id, state: .unread, number: id, title: nil, volume: nil,
            published: nil, pages: 20, newChapter: false, download: state, versions: 1, bookmarked: false),
        state: state)
}

private func chapters(_ states: ChapterDownloadState...) -> BookDownloadsView {
    BookDownloadsView(
        rows: states.enumerated().map { chapter(String($0.offset + 1), $0.element) },
        meter: StorageMeter(used: 0, free: 0, chapters: 0, readBytes: 0), series: [], total: [])
}

@MainActor
private final class FakeWindow: DownloadWindow {
    var reports: [(completed: Int64, total: Int64, title: String, subtitle: String)] = []
    var ended: Bool?

    func report(completed: Int64, total: Int64, title: String, subtitle: String) {
        reports.append((completed, total, title, subtitle))
    }

    func end(success: Bool) {
        ended = success
    }
}

@MainActor
private final class Harness {
    let continued = ContinuedDownloads()
    var asked: [String] = []
    var titles: [String] = []
    var answer = true
    var phases: [AppState] = []
    var playing = false

    init() {
        continued.ask = { [unowned self] title, subtitle in
            titles.append(title)
            asked.append(subtitle)
            return answer
        }
        continued.send = { [unowned self] command in
            if case .appPhase(let phase) = command { phases.append(phase) }
        }
        continued.playing = { [unowned self] in playing }
    }
}

@MainActor
@Suite struct ContinuedDownloadsTests {
    @Test func aDownloadStartedAsksForAWindowOnce() {
        let h = Harness()
        h.continued.update([row(1, .done)])
        #expect(h.asked.isEmpty, "nothing to go on")
        h.continued.update([row(1, .done), row(2, .queued)])
        #expect(h.asked == ["One episode left"])
        #expect(h.continued.stage == .asked)
        #expect(h.continued.isRunning, "the Live Activity stays out of the way from the ask")
        h.continued.update([row(1, .done), row(2, .active, 0.1), row(3, .queued)])
        #expect(h.asked.count == 1, "not again while one is asked for")
    }

    @Test func aRefusalLetsTheNextDownloadAskAgain() {
        let h = Harness()
        h.answer = false
        h.continued.update([row(1, .queued)])
        #expect(h.continued.stage == .idle)
        h.continued.update([row(1, .active, 0.2)])
        #expect(h.asked.count == 1, "the same downloads ask nothing new")
        h.answer = true
        h.continued.update([row(1, .active, 0.2), row(2, .queued)])
        #expect(h.asked.count == 2)
        #expect(h.continued.stage == .asked)
    }

    @Test func anOpenWindowReportsAndClosesWhenEverythingIsDone() {
        let h = Harness()
        h.continued.update([row(1, .active, 0.5), row(2, .queued)])
        let window = FakeWindow()
        h.continued.began(window, rows: [row(1, .active, 0.5), row(2, .queued)])
        #expect(h.continued.stage == .running)
        #expect(h.phases.isEmpty, "in front, the engine is told nothing yet")
        #expect(window.reports.last?.completed == 500)
        #expect(window.reports.last?.total == 2_000)
        #expect(window.reports.last?.subtitle == "2 episodes left")

        // The app leaves: the downloads go on in the background.
        h.continued.phase(foreground: false, rows: [row(1, .active, 0.5), row(2, .queued)])
        #expect(h.phases == [.backgroundDownloads])

        h.continued.update([row(1, .done, 1), row(2, .active, 0.25)])
        #expect(window.reports.last?.completed == 1_250)
        #expect(window.reports.last?.subtitle == "One episode left")

        h.continued.update([row(1, .done, 1), row(2, .done, 1)])
        #expect(window.ended == true)
        #expect(h.continued.stage == .idle)
        #expect(h.phases == [.backgroundDownloads, .background(playing: false)], "everything waits again")
    }

    @Test func runningOutOrBeingStoppedPausesWhereTheyGotTo() {
        let h = Harness()
        h.playing = true
        h.continued.update([row(1, .active, 0.3)])
        let window = FakeWindow()
        h.continued.began(window, rows: [row(1, .active, 0.3)])
        h.continued.phase(foreground: false, rows: [row(1, .active, 0.3)])
        h.continued.expired()
        #expect(window.ended == false)
        #expect(h.continued.stage == .idle)
        #expect(h.phases.last == .background(playing: true), "what plays goes on; the downloads wait")
        // Nothing more is reported to a closed window.
        h.continued.update([row(1, .active, 0.4)])
        #expect(window.reports.count == 1)
    }

    @Test func leavingWithDownloadsQueuedAsksAndTheOpenedWindowLetsThemGoOn() {
        let h = Harness()
        let rows = [row(1, .queued)]
        // Queued at launch, before any window: the app leaves.
        h.continued.leaving(rows: rows)
        #expect(h.asked == ["One episode left"])
        h.continued.phase(foreground: false, rows: rows)
        #expect(h.phases == [.background(playing: false)], "held until the window opens")
        h.continued.began(FakeWindow(), rows: rows)
        #expect(h.phases == [.background(playing: false), .backgroundDownloads])
        // Back in front: everything runs.
        h.continued.phase(foreground: true, rows: rows)
        #expect(h.phases.last == .foreground)
        #expect(h.continued.stage == .running, "the window stays open until the downloads finish")
    }

    @Test func aWindowWithNothingLeftClosesAtOnce() {
        let h = Harness()
        h.continued.update([row(1, .queued)])
        let window = FakeWindow()
        h.continued.began(window, rows: [row(1, .done, 1)])
        #expect(window.ended == true)
        #expect(h.continued.stage == .idle)
    }

    @Test func theLiveActivityStaysAwayFromTheAskUntilTheWindowCloses() {
        let store = AppStore()
        store.continued.ask = { _, _ in true }
        #expect(!store.downloadsActivity.systemShowsProgress)
        store.continued.update([row(1, .queued)])
        #expect(store.downloadsActivity.systemShowsProgress, "asked for: the system's progress is about to show")
        let window = FakeWindow()
        store.continued.began(window, rows: [row(1, .active, 0.5)])
        #expect(store.downloadsActivity.systemShowsProgress)
        store.continued.update([row(1, .done, 1)])
        #expect(window.ended == true)
        #expect(!store.downloadsActivity.systemShowsProgress, "the app's own activity may show again")

        // A window the system would not open lets it show again too.
        store.continued.update([row(1, .done, 1), row(2, .queued)])
        #expect(store.downloadsActivity.systemShowsProgress)
        store.continued.refused()
        #expect(!store.downloadsActivity.systemShowsProgress)
    }

    @Test func aChapterDownloadStartedAsksForAWindow() {
        let h = Harness()
        h.continued.update(chapters: chapters(.done).rows)
        #expect(h.asked.isEmpty, "nothing to go on")
        h.continued.update(chapters: chapters(.done, .queued, .queued).rows)
        #expect(h.titles == ["Downloading chapters"])
        #expect(h.asked == ["2 chapters left"])
        #expect(h.continued.stage == .asked)
    }

    @Test func leavingWithOnlyChaptersQueuedAsksAndTellsTheEngineTheyGoOn() {
        let h = Harness()
        h.answer = false
        // Queued in front while the system would not open a window: the app then leaves.
        h.continued.update(chapters: chapters(.queued, .queued).rows)
        h.answer = true
        h.continued.leaving(rows: [])
        #expect(h.asked.last == "2 chapters left")
        h.continued.phase(foreground: false, rows: [])
        #expect(h.phases == [.background(playing: false)], "held until the window opens")
        let window = FakeWindow()
        h.continued.began(window, rows: [])
        #expect(h.continued.stage == .running)
        #expect(h.phases == [.background(playing: false), .backgroundDownloads], "the chapter queue runs away")
        #expect(window.reports.last?.total == 2_000)
        #expect(window.reports.last?.completed == 0)

        h.continued.update(chapters: chapters(.downloading(done: 5, total: 20), .queued).rows)
        #expect(window.reports.last?.completed == 250)
        h.continued.update(chapters: chapters(.done, .downloading(done: 0, total: 0)).rows)
        #expect(window.reports.last?.completed == 1_000, "a novel's chapter counts once done")
        #expect(window.reports.last?.subtitle == "One chapter left")
        h.continued.update(chapters: chapters(.done, .done).rows)
        #expect(window.ended == true)
        #expect(h.phases.last == .background(playing: false), "everything waits again")
    }

    @Test func episodesAndChaptersShareOneWindowAndCountTogether() {
        let h = Harness()
        h.continued.update([row(1, .active, 0.5)])
        #expect(h.titles == ["Downloading episodes"])
        let window = FakeWindow()
        h.continued.began(window, rows: [row(1, .active, 0.5)])
        h.continued.update(chapters: chapters(.downloading(done: 10, total: 20), .queued).rows)
        #expect(h.asked.count == 1, "one window for both")
        #expect(window.reports.last?.completed == 1_000)
        #expect(window.reports.last?.total == 3_000)
        #expect(window.reports.last?.title == "Downloading episodes and chapters")
        #expect(window.reports.last?.subtitle == "One episode left · 2 chapters left")

        // The episode finishes; the window stays open for the chapters.
        h.continued.update([row(1, .done, 1)])
        #expect(window.ended == nil)
        #expect(window.reports.last?.completed == 1_500)
        #expect(window.reports.last?.title == "Downloading chapters")
        h.continued.update(chapters: chapters(.done, .done).rows)
        #expect(window.ended == true)
    }

    @Test func chaptersWaitingForWifiOrFailedDoNotHoldAWindow() {
        let h = Harness()
        h.continued.update(chapters: chapters(.waitsForWifi, .failed(failure: .offline)).rows)
        #expect(h.asked.isEmpty, "nothing could go on in a window")
        let window = FakeWindow()
        h.continued.update(chapters: chapters(.queued).rows)
        h.continued.began(window, rows: [])
        h.continued.update(chapters: chapters(.failed(failure: .offline)).rows)
        #expect(window.ended == true)
        #expect(window.reports.isEmpty, "closed rather than reported")
    }

    @Test func theStoreHandsTheEnginesChapterDownloadsToTheWindow() {
        let store = AppStore()
        var asked: [String] = []
        store.continued.ask = { title, _ in
            asked.append(title)
            return true
        }
        store.apply(.reading(.downloads(chapters(.queued))))
        #expect(asked == ["Downloading chapters"])
        #expect(store.downloadsActivity.systemShowsProgress)
        #expect(ContinuedDownloads.goesOn([], chapters: chapters(.queued).rows), "the Downloads note says they go on")
        #expect(!ContinuedDownloads.goesOn([], chapters: chapters(.waitsForWifi).rows))
    }

    @Test func withNoWindowToAskForNothingIsAsked() {
        let continued = ContinuedDownloads()
        continued.update([row(1, .queued)])
        continued.leaving(rows: [row(1, .queued)])
        #expect(continued.stage == .idle)
        #expect(!continued.isRunning)
    }
}
