// A send follows the engine's words: chapters done of total, a failure that names its job and
// chapter, and the e-reader the engine kept.

@testable import KaeriStore
import KaeriKit
import Testing

@MainActor
struct SendModelTests {
    private func kobo(chosen: Bool = false) -> ExportTarget {
        ExportTarget(id: "kobo", name: "Kobo", widthPx: 1264, heightPx: 1680, colour: false,
                     formats: [.cbz], mangaFormats: [.cbz], novelFormats: [], split: false, chosen: chosen)
    }

    @Test func progressCarriesTheChaptersDone() {
        let model = SendModel()
        model.apply(.exportTargets([kobo()]))
        let id = model.begin(work: "w", chapters: ["1", "2", "3"], target: kobo(), novel: false)
        model.apply(.exportProgress(job: 7, done: 150, total: 600, chaptersDone: 1, chapters: 3))
        #expect(model.send(id!)?.phase == .preparing(fraction: 0.25, done: 1, total: 3))
        #expect(model.send(id!)?.job == 7)
    }

    @Test func aFailureNamesItsJobAndChapter() {
        let model = SendModel()
        let first = model.begin(work: "w", chapters: ["1"], target: kobo(), novel: false)
        let second = model.begin(work: "w", chapters: ["2"], target: kobo(), novel: false)
        model.apply(.exportProgress(job: 1, done: 0, total: 10, chaptersDone: 0, chapters: 1))
        model.apply(.exportProgress(job: 2, done: 0, total: 10, chaptersDone: 0, chapters: 1))
        // The second send fails while the first goes on: it is the second that says so.
        model.apply(.exportFailed(job: 2, chapter: "12", failure: .offline))
        guard case .failed(let words)? = model.send(second!)?.phase else {
            Issue.record("the second send did not fail")
            return
        }
        #expect(words.contains("12"))
        #expect(model.send(first!)?.isRunning == true)
    }

    /// The engine clears ready files after a day: Share then says they are gone, offers Send
    /// again, and nothing is counted as sent.
    @Test func shareChecksTheFilesAreStillThere() {
        let model = SendModel()
        var sent: [Command] = []
        model.command = { sent.append($0) }
        let id = model.begin(work: "w", chapters: ["1", "2"], target: kobo(), novel: false)!
        model.apply(.exportProgress(job: 4, done: 0, total: 10, chaptersDone: 0, chapters: 2))
        let files = [
            ExportFile(path: "/sends/one.cbz", name: "one.cbz", bytes: 10, part: nil),
            ExportFile(path: "/sends/two.cbz", name: "two.cbz", bytes: 10, part: nil),
        ]
        model.apply(.exportReady(job: 4, files: files))

        #expect(model.shareable(id) { _ in true }?.map(\.path) == ["/sends/one.cbz", "/sends/two.cbz"])
        #expect(model.shareable(id) { $0 != "/sends/two.cbz" } == nil)
        #expect(model.send(id)?.phase == .gone)

        let before = sent.count
        model.shared(id)
        #expect(sent.count == before, "files that are gone are never counted as sent")

        let again = model.sendAgain(id)
        #expect(again != nil)
        #expect(model.send(id) == nil)
        #expect(model.send(again!)?.isRunning == true)
        guard case .reading(.prepare(let work, .chapters(let ids), let target, _))? = sent.last else {
            Issue.record("Send again did not ask for the files")
            return
        }
        #expect(work == "w")
        #expect(ids == ["1", "2"])
        #expect(target == "kobo")
    }

    /// A send the engine started again at launch is one this screen never asked for: it learns
    /// its chapters from the waiting word, so once ready, sharing it says which chapters went.
    @Test func aSendHeldAcrossARestartKnowsItsChapters() {
        let model = SendModel()
        var sent: [Command] = []
        model.command = { sent.append($0) }
        model.apply(.exportWaiting(job: 41, work: "w", target: "kobo", line: .verbatim(text: "Waiting for Wi-Fi"), chapters: ["c3", "c4"]))
        guard let send = model.sends.first(where: { $0.job == 41 }) else {
            Issue.record("the restarted send did not show")
            return
        }
        #expect(send.chapters == ["c3", "c4"])
        #expect(send.isWaiting)

        model.apply(.exportReady(job: 41, files: [ExportFile(path: "/sends/b.kepub.epub", name: "b.kepub.epub", bytes: 10, part: nil)]))
        model.shared(send.id)
        let shared = sent.compactMap { command -> [String]? in
            if case .reading(.shared(let work, let chapters, let target)) = command, work == "w", target == "kobo" { chapters } else { nil }
        }
        #expect(shared == [["c3", "c4"]])
    }

    /// The Send sheet opened again on the same chapters finds the send under way, and Prepare
    /// there starts no second one. Other chapters, or the same ones once the first has ended,
    /// are a send of their own.
    @Test func theSameChaptersUnderWayAreOneSend() {
        let model = SendModel()
        var sent: [Command] = []
        model.command = { sent.append($0) }
        let prepares = { sent.filter { if case .reading(.prepare) = $0 { true } else { false } }.count }
        let first = model.begin(work: "w", chapters: ["23"], target: kobo(), novel: false)
        model.apply(.exportProgress(job: 9, done: 1, total: 25, chaptersDone: 0, chapters: 1))
        #expect(model.running(work: "w", chapters: ["23"])?.id == first)

        #expect(model.begin(work: "w", chapters: ["23"], target: kobo(), novel: false) == first)
        #expect(model.sends.count == 1)
        #expect(prepares() == 1)

        #expect(model.running(work: "w", chapters: ["24"]) == nil)
        #expect(model.running(work: "w", chapters: ["23"], target: "kindle") == nil)
        model.apply(.exportFailed(job: 9, chapter: nil, failure: .offline))
        #expect(model.running(work: "w", chapters: ["23"]) == nil)
        #expect(model.begin(work: "w", chapters: ["23"], target: kobo(), novel: false) != first)
        #expect(prepares() == 2)
    }

    /// A send this screen asked for keeps the chapters it asked for.
    @Test func aSendAskedForKeepsItsOwnChapters() {
        let model = SendModel()
        let id = model.begin(work: "w", chapters: ["1", "2"], target: kobo(), novel: false)!
        model.apply(.exportWaiting(job: 5, work: "w", target: "kobo", line: .verbatim(text: "Waiting for Wi-Fi"), chapters: ["2"]))
        #expect(model.send(id)?.chapters == ["1", "2"])
    }

    @Test func theEngineKeepsTheDefaultEReader() {
        let model = SendModel()
        var sent: [Command] = []
        model.command = { sent.append($0) }
        model.apply(.exportTargets([kobo(chosen: true)]))
        #expect(model.defaultTarget?.id == "kobo")
        model.choose(target: "kindle")
        #expect(sent.count == 1)
    }
}
