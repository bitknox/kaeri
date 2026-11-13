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
