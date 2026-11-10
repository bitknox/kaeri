// The reader says when an open fails, and it always closes: from opening, from a failure, and
// while a second book waits on the first to close.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

/// What the model sent the engine.
@MainActor
private final class Sent {
    var all: [Command] = []
    var opens: Int { all.filter { if case .reader(.open) = $0 { true } else { false } }.count }
}

@MainActor
struct ReaderAlwaysClosesTests {
    private let surface = ReaderSurface(widthPx: 1600, heightPx: 1000, scale: 2, eink: false, twoPages: false, dark: true)

    private func row(_ id: String) -> ChapterRow {
        ChapterRow(
            id: ChapterId(id), label: id, state: .unread, number: id, title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
    }

    private func view(_ chapter: String, session: UInt64, next: String? = nil, mode: ReaderMode = .paged) -> ReaderView {
        ReaderView(
            session: session, work: WorkId("a:900301"), title: "The Lantern Ferry", medium: .manga,
            chapter: row(chapter), mode: mode, direction: .rightToLeft,
            start: ReadPlace(work: WorkId("a:900301"), chapter: ChapterId(chapter), page: 0, strip: nil, novel: nil),
            next: next.map(row), previous: nil, settings: nil)
    }

    private func opening(_ sent: Sent = Sent()) -> ReaderModel {
        let model = ReaderModel()
        model.send = { sent.all.append($0) }
        model.open(work: "a:900301", chapter: "676")
        model.surfaceChanged(surface)
        return model
    }

    /// A source without the chapter: the open fails, and the reader says so instead of
    /// "Finding a source" for good.
    @Test func aFailedOpenShowsTheFailure() {
        let model = opening()
        model.observe(.reading(.failed(about: .reader, failure: .notFound)))
        #expect(model.phase == .failed(.notFound))
    }

    @Test func aFailureForAnotherScreenLeavesTheReaderAlone() {
        let model = opening()
        model.observe(.reading(.failed(about: .series, failure: .notFound)))
        #expect(model.phase == .opening)
    }

    @Test func tryAgainAfterAFailedOpenAsksForTheOpenAgain() {
        let sent = Sent()
        let model = opening(sent)
        model.observe(.reading(.failed(about: .reader, failure: .offline)))
        model.retry(page: nil)
        #expect(sent.opens == 2)
        #expect(model.phase == .opening)
    }

    @Test func closingFromAFailureCloses() {
        let model = opening()
        model.observe(.reading(.failed(about: .reader, failure: .notFound)))
        model.close()
        #expect(model.request == nil)
        #expect(model.phase == .closed)
    }

    /// Closed while it opened: the session the engine opens after is closed at once, and the
    /// reader stays closed.
    @Test func aSessionThatOpensAfterTheCloseIsClosed() {
        let sent = Sent()
        let model = opening(sent)
        model.close()
        model.apply(.opened(view("676", session: 7)))
        #expect(sent.all.contains { if case .reader(.close(7)) = $0 { true } else { false } })
        #expect(model.request == nil)
        #expect(model.view == nil)
    }

    /// A second book replaces the first without the reader going away in between, and closing
    /// before the first has closed drops the second as well.
    @Test func aSecondBookKeepsTheReaderUpAndClosingDropsIt() {
        let sent = Sent()
        let model = opening(sent)
        model.apply(.opened(view("676", session: 1)))
        model.open(work: "a:900302")
        #expect(model.request?.work == "a:900302")
        model.apply(.closed(session: 1))
        #expect(sent.opens == 2, "the second book opens once the first has closed")

        model.apply(.opened(view("1", session: 2)))
        model.open(work: "a:900303")
        model.close()
        model.apply(.closed(session: 2))
        #expect(model.request == nil)
        #expect(sent.opens == 2, "the book waiting on the close is dropped with it")
    }

    /// A strip that fails to flow into the next chapter says so, and Try again asks for that
    /// chapter, not the one above it.
    @Test func aStripThatFailsToFlowOnSaysSoAndRetriesTheNextChapter() {
        let sent = Sent()
        let model = opening(sent)
        model.apply(.opened(view("1", session: 1, next: "2", mode: .strip)))
        model.apply(.leaves(session: 1, chapter: ChapterId("1"), leaves: [
            Leaf(index: 0, parts: [
                LeafPart(page: 0, kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1), rotate: false, half: nil),
            ], endPage: false),
        ]))
        model.flowOn()
        model.observe(.reading(.failed(about: .reader, failure: .pageFetch(status: 503))))
        #expect(model.phase == .failed(.pageFetch(status: 503)))
        model.retry(page: nil)
        #expect(sent.all.contains { if case .reader(.retry(1, ChapterId("2"), nil)) = $0 { true } else { false } })
    }
}
