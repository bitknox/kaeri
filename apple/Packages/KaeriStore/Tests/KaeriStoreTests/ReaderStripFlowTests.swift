// A strip flows into the next chapter only past the real end of the one open: a chapter whose
// last image has not landed is never passed, and its foot starts no read. A strip opens at the place
// kept, even one kept only as a page.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
private final class Sent {
    var all: [Command] = []

    var opened: [ChapterId] {
        all.compactMap { if case .reader(.openChapter(_, let chapter)) = $0 { chapter } else { nil } }
    }

    var feet: [Bool] {
        all.compactMap { if case .reader(.stripAt(_, _, _, let foot)) = $0 { foot } else { nil } }
    }
}

@MainActor
struct ReaderStripFlowTests {
    private func row(_ id: String) -> ChapterRow {
        ChapterRow(
            id: ChapterId(id), label: id, state: .unread, number: id, title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
    }

    private func file(_ page: UInt32) -> PageFile {
        PageFile(path: "/tmp/kaeri-test/\(page).png", encoding: .original(mime: "image/png"), width: 800,
                 height: 1200, crop: nil, colour: true, bytes: 1)
    }

    /// Chapter 1 open in a strip, chapter 2 after it, its three images with `ratio`.
    private func strip(_ sent: Sent, ratio: Float? = nil) -> ReaderModel {
        let model = ReaderModel()
        model.send = { sent.all.append($0) }
        model.open(work: "a:900301", chapter: "1")
        model.surfaceChanged(ReaderSurface(widthPx: 1170, heightPx: 2532, scale: 3, eink: false, twoPages: false, dark: true))
        sent.all = []
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: row("1"), mode: .strip, direction: .topToBottom,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("1"), page: 0, strip: nil, novel: nil),
            next: row("2"), previous: nil, settings: nil)))
        let parts = (0..<3).map { page in
            LeafPart(page: UInt32(page), kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1),
                     rotate: false, half: nil, ratio: ratio)
        }
        model.apply(.leaves(session: 1, chapter: ChapterId("1"), leaves: [
            Leaf(index: 0, parts: parts, endPage: false),
            Leaf(index: 1, parts: [], endPage: true),
        ]))
        return model
    }

    private func land(_ model: ReaderModel, _ pages: [UInt32]) {
        for page in pages {
            model.apply(.pageReady(session: 1, chapter: ChapterId("1"), page: page, half: nil, file: file(page)))
        }
    }

    /// Opening the next chapter counts this one as read, so a chapter whose last image is still
    /// coming, its foot a guess, is never passed: the next one opens once it has landed.
    @Test func aChapterStillLandingIsNotPassed() {
        let sent = Sent()
        let model = strip(sent)
        land(model, [0, 1])
        #expect(!model.stripEndIn)
        model.flowOn()
        #expect(model.flowing == nil)
        #expect(sent.opened.isEmpty)

        land(model, [2])
        #expect(model.stripEndIn)
        model.flowOn()
        #expect(model.flowing == ChapterId("2"))
        #expect(sent.opened == [ChapterId("2")])
    }

    /// Only the end has to be in: images far above, not fetched since the strip opened at its
    /// place near the end, do not hold the strip at its foot.
    @Test func imagesFarAboveStillComingDoNotHoldTheEnd() {
        let sent = Sent()
        let model = strip(sent)
        land(model, [2])
        #expect(model.stripEndIn)
        model.flowOn()
        #expect(sent.opened == [ChapterId("2")])
    }

    /// A chapter on the device knows its images' shapes before their files are made.
    @Test func aChapterWithKnownShapesIsPassed() {
        let sent = Sent()
        let model = strip(sent, ratio: 0.7)
        #expect(model.stripEndIn)
        model.flowOn()
        #expect(sent.opened == [ChapterId("2")])
    }

    /// An image that failed has said all it will: the strip can go on past it.
    @Test func anImageThatFailedDoesNotHoldTheStrip() {
        let sent = Sent()
        let model = strip(sent)
        land(model, [0, 1])
        model.apply(.pageFailed(session: 1, chapter: ChapterId("1"), page: 2, failure: .pageFetch(status: 503)))
        #expect(model.stripEndIn)
        model.flowOn()
        #expect(sent.opened == [ChapterId("2")])
    }

    /// Nothing laid out yet: there is no end to pass.
    @Test func aChapterWithNoImagesYetIsNotPassed() {
        let sent = Sent()
        let model = ReaderModel()
        model.send = { sent.all.append($0) }
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: row("1"), mode: .strip, direction: .topToBottom,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("1"), page: 0, strip: nil, novel: nil),
            next: row("2"), previous: nil, settings: nil)))
        model.flowOn()
        #expect(sent.opened.isEmpty)
    }

    /// The foot of a chapter still landing is only where a guess put it: the report says it is
    /// not in sight, so no read starts and no place at the end is kept as the end.
    @Test func theFootOfAChapterStillLandingStartsNoRead() {
        let sent = Sent()
        let model = strip(sent)
        land(model, [0])
        model.stripAt(StripPos(image: 2, offset: 0), footVisible: true)
        #expect(sent.feet == [false])

        land(model, [1, 2])
        model.stripAt(StripPos(image: 2, offset: 0.5), footVisible: true)
        #expect(sent.feet == [false, true])
    }

    /// Online but the source cannot be reached, or offline with the next chapter only a source
    /// has: the flow fails, and the chapter read stays open, every image where it was, with its
    /// end page naming the chapter that would not open and why.
    @Test(arguments: [ReadingFailure.sourceDown(source: "books.home.example"), .offline])
    func aStripThatCannotFlowOnKeepsTheChapterRead(_ failure: ReadingFailure) {
        let sent = Sent()
        let model = strip(sent, ratio: 0.7)
        model.flowOn()
        model.apply(.chapterRead(work: WorkId("a:900301"), chapter: ChapterId("1"), trackers: []))
        model.observe(.reading(.failed(about: .reader, failure: failure)))
        #expect(model.phase == .open)
        #expect(model.view?.chapter.id == ChapterId("1"))
        #expect(model.leaves.count == 2)
        #expect(model.trail.chapters.isEmpty, "the chapter read is the open one again, not a trail above nothing")
        #expect(model.flowing == nil)
        #expect(model.read)
        #expect(model.flowFailure == ReaderModel.FlowFailure(next: row("2"), failure: failure))

        // Scrolling past the end again asks nothing more: the end page's Try again does.
        model.flowOn()
        #expect(sent.opened == [ChapterId("2")])
    }

    /// Try again flows into the chapter under the one read, asking the engine, whose session is
    /// on that chapter, for it again; a second failure says so the same way.
    @Test func tryAgainFlowsOnIntoTheChapterThatFailed() {
        let sent = Sent()
        let model = strip(sent, ratio: 0.7)
        model.flowOn()
        model.observe(.reading(.failed(about: .reader, failure: .offline)))
        sent.all = []

        model.retry(page: nil)
        #expect(sent.all.count == 1)
        #expect(sent.all.contains { if case .reader(.retry(1, ChapterId("2"), nil)) = $0 { true } else { false } })
        #expect(model.phase == .open, "no opening screen over the chapter read")
        #expect(model.flowing == ChapterId("2"))
        #expect(model.flowFailure == nil)
        #expect(model.trail.chapters.map(\.id) == [ChapterId("1")])

        model.observe(.reading(.failed(about: .reader, failure: .pageFetch(status: 503))))
        #expect(model.phase == .open)
        #expect(model.flowFailure?.failure == .pageFetch(status: 503))
        #expect(model.trail.chapters.isEmpty)

        model.retryFlow()
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: row("2"), mode: .strip, direction: .topToBottom,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("2"), page: 0, strip: nil, novel: nil),
            next: nil, previous: row("1"), settings: nil)))
        #expect(model.view?.chapter.id == ChapterId("2"))
        #expect(model.trail.chapters.map(\.id) == [ChapterId("1")], "it opens under the chapter read")
        #expect(model.flowFailure == nil)
    }

    /// A chapter opened by hand after a failed flow starts clean.
    @Test func openingAChapterClearsAFailedFlow() {
        let sent = Sent()
        let model = strip(sent, ratio: 0.7)
        model.flowOn()
        model.observe(.reading(.failed(about: .reader, failure: .offline)))
        model.openChapter(ChapterId("3"))
        #expect(model.flowFailure == nil)
        #expect(model.phase == .opening)
    }

    /// A chapter opened in a strip at `page` and maybe `strip`, its images showing `pages`.
    private func resumed(page: UInt32, strip: StripPos?, pages: [UInt32]) -> ReaderModel {
        let model = ReaderModel()
        model.send = { _ in }
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: row("1"), mode: .strip, direction: .topToBottom,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("1"), page: page, strip: strip, novel: nil),
            next: row("2"), previous: nil, settings: nil)))
        let parts = pages.map { page in
            LeafPart(page: page, kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1),
                     rotate: false, half: nil, ratio: 0.7)
        }
        model.apply(.leaves(session: 1, chapter: ChapterId("1"), leaves: [
            Leaf(index: 0, parts: parts, endPage: false),
            Leaf(index: 1, parts: [], endPage: true),
        ]))
        return model
    }

    /// A chapter last read page by page keeps its place as a page only: the strip opens at the
    /// image that holds it, the one the Resumed notice names, not at the first.
    @Test func aPlaceKeptAsAPageOpensTheStripAtItsImage() {
        let model = resumed(page: 8, strip: nil, pages: Array(0..<12))
        #expect(model.stripStart == StripPos(image: 8, offset: 0))
    }

    /// Pages the strip leaves out, such as a credit page, do not shift the place: it is the
    /// image that holds the page, or the first after it.
    @Test func aPlaceKeptAsAPageFollowsThePagesTheStripShows() {
        let model = resumed(page: 8, strip: nil, pages: [0, 1, 3, 4, 5, 6, 7, 8, 9])
        #expect(model.stripStart == StripPos(image: 7, offset: 0))
        let past = resumed(page: 20, strip: nil, pages: Array(0..<12))
        #expect(past.stripStart == StripPos(image: 11, offset: 0))
    }

    /// A place kept in the strip itself, partway down an image, is used as it is, and a jump asked
    /// for later goes before either.
    @Test func aPlaceKeptInTheStripWins() {
        let model = resumed(page: 8, strip: StripPos(image: 5, offset: 0.4), pages: Array(0..<12))
        #expect(model.stripStart == StripPos(image: 5, offset: 0.4))
        model.apply(.goTo(session: 1, place: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId("1"),
                                                      page: nil, strip: StripPos(image: 2, offset: 0), novel: nil)))
        #expect(model.stripStart == StripPos(image: 2, offset: 0))
    }
}
