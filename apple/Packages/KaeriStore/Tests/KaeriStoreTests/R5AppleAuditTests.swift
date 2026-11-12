// The reader through a failed chapter open, a close, the app leaving and a second book: what
// the round 5 Apple audit of reading checked.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
private final class Sent {
    var all: [Command] = []
    var opens: Int { all.filter { if case .reader(.open) = $0 { true } else { false } }.count }
}

@MainActor
struct R5AppleAuditTests {
    private let session: UInt64 = 1
    private let work = WorkId("a:900301")
    private let surface = ReaderSurface(widthPx: 1600, heightPx: 1000, scale: 2, eink: false, twoPages: false, dark: true)

    private func row(_ id: String) -> ChapterRow {
        ChapterRow(
            id: ChapterId(id), label: "Chapter \(id)", state: .unread, number: id, title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
    }

    private func view(_ chapter: String, next: String?, mode: ReaderMode = .paged, session: UInt64 = 1,
                      work: String = "a:900301") -> ReaderView {
        ReaderView(
            session: session, work: WorkId(work), title: "The Lantern Ferry", medium: .manga,
            chapter: row(chapter), mode: mode, direction: .rightToLeft,
            start: ReadPlace(work: WorkId(work), chapter: ChapterId(chapter), page: 0, strip: nil, novel: nil),
            next: next.map(row), previous: nil, settings: nil)
    }

    private func leaves(_ pages: Int) -> [Leaf] {
        var all = (0..<pages).map { page in
            Leaf(index: UInt32(page), parts: [
                LeafPart(page: UInt32(page), kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1), rotate: false, half: nil),
            ], endPage: false)
        }
        all.append(Leaf(index: UInt32(pages), parts: [], endPage: true))
        return all
    }

    /// A reader asked for chapter 13 of the book, as the app opens one, with its surface known.
    private func opening(_ sent: Sent, chapter: String = "13") -> ReaderModel {
        let model = ReaderModel()
        model.send = { sent.all.append($0) }
        model.open(work: "a:900301", chapter: chapter)
        model.surfaceChanged(surface)
        return model
    }

    /// Chapter 13 open on its end page.
    private func onEndPage(_ sent: Sent = Sent()) -> ReaderModel {
        let model = opening(sent)
        model.heldCue = .milliseconds(30)
        model.apply(.opened(view("13", next: "14")))
        model.apply(.leaves(session: session, chapter: ChapterId("13"), leaves: leaves(3)))
        model.shown(3, how: .turn)
        return model
    }

    /// Continue from the end page into a chapter whose open fails: the engine says so with
    /// `ReadingEvent.failed(.reader)` and no `opened`, and the held end page goes, so the failure
    /// and its buttons show and take taps.
    @Test
    func a1_aChapterOpenThatFailsLetsTheHeldPageGo() {
        let model = onEndPage()
        model.openChapter(ChapterId("14"))
        #expect(model.held != nil)
        model.observe(.reading(.failed(about: .reader, failure: .notFound)))
        #expect(model.phase == .failed(.notFound))
        #expect(model.held?.landed == true, "the held end page must go so the failure and its buttons show")
    }

    /// The same failure names the chapter that would not open, not the one it was left from.
    @Test
    func a2_aFailedChapterOpenNamesTheChapterThatFailed() {
        let model = onEndPage()
        model.shown(1, how: .turn)
        model.openChapter(ChapterId("14"))
        model.observe(.reading(.failed(about: .reader, failure: .notFound)))
        #expect(model.view?.chapter.id != ChapterId("13"), "the failure screen names chapter 13, which opened fine")
    }

    /// A strip report held back by the 500 ms pacing goes to the engine as the reader closes, so
    /// the place kept is the one shown last.
    @Test
    func a3_closingSendsTheStripPlaceHeldBack() {
        let sent = Sent()
        let model = opening(sent)
        model.apply(.opened(view("13", next: "14", mode: .strip)))
        model.apply(.leaves(session: session, chapter: ChapterId("13"), leaves: leaves(8)))
        model.stripAt(StripPos(image: 1, offset: 0), footVisible: false)
        let later = StripPos(image: 5, offset: 0.5)
        model.stripAt(later, footVisible: false)
        model.close()
        let places = sent.all.compactMap { command -> StripPos? in
            if case .reader(.stripAt(_, _, let at, _)) = command { at } else { nil }
        }
        #expect(places.last == later, "the engine last heard \(String(describing: places.last))")
    }

    /// Not a credit page waits out its 8 s Undo before the engine hears, and goes at once when
    /// the app leaves, which may end it in the background.
    @Test
    func a4_aKeepGoesToTheEngineWhileTheAppIsAway() async throws {
        let sent = Sent()
        let model = opening(sent)
        model.apply(.opened(view("13", next: "14")))
        model.apply(.leaves(session: session, chapter: ChapterId("13"), leaves: leaves(3)))
        let file = PageFile(path: "/tmp/credit.png", encoding: .original(mime: "image/png"), width: 800,
                            height: 1200, crop: nil, colour: true, bytes: 1)
        let credit = CreditPage(page: 0, atEnd: false, file: file, fingerprint: 11)
        model.keepCredit(credit, chapter: "13")
        model.away = true
        let deadline = ContinuousClock.now + .seconds(10)
        func kept() -> Bool {
            sent.all.contains { if case .reader(.notACreditPage) = $0 { true } else { false } }
        }
        while !kept(), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(200))
        }
        #expect(kept(), "the keep waits for the app to come back, and is lost if it never does")
    }

    /// Closed while opening, then another book opened before the engine answered the first: the
    /// first book's late `opened` is closed, never shown as the second.
    @Test
    func a5_aBookClosedWhileOpeningNeverShowsAsTheNext() {
        let sent = Sent()
        let model = ReaderModel()
        model.send = { sent.all.append($0) }
        model.open(work: "a:900301", chapter: "676")
        model.surfaceChanged(surface)
        model.close()
        model.open(work: "a:900302")
        model.apply(.opened(view("676", next: nil, session: 7, work: "a:900301")))
        #expect(model.request?.work == "a:900302")
        #expect(model.view?.work != WorkId("a:900301"), "the closed book's chapter shows as the new one")
        #expect(sent.all.contains { if case .reader(.close(7)) = $0 { true } else { false } })
        // The engine answers the close sent while the first book opened, then the second opens.
        model.apply(.closed(session: 7))
        model.apply(.closed(session: 0))
        model.apply(.opened(view("1", next: nil, session: 8, work: "a:900302")))
        #expect(model.phase == .open)
        #expect(model.view?.work == WorkId("a:900302"))
        #expect(sent.opens == 2, "the second book is asked for once")
    }

    /// The same book opened again after a close while it opened, and the engine dropped the
    /// first open: the one open that comes, after the close is answered, is shown.
    @Test
    func theSameBookReopenedOpensOnceTheCloseIsAnswered() {
        let sent = Sent()
        let model = opening(sent)
        model.close()
        #expect(sent.all.contains { if case .reader(.close(0)) = $0 { true } else { false } })
        model.open(work: "a:900301")
        model.apply(.closed(session: 0))
        model.apply(.opened(view("13", next: "14", session: 2)))
        #expect(model.phase == .open)
        #expect(model.view?.chapter.id == ChapterId("13"))
    }

    /// A failure that answers the closed book's open is that book's: the next book keeps opening.
    @Test
    func aFailureOfABookClosedWhileOpeningLeavesTheNextOpening() {
        let model = opening()
        model.close()
        model.open(work: "a:900302")
        model.observe(.reading(.failed(about: .reader, failure: .notFound)))
        #expect(model.phase == .opening)
        model.apply(.closed(session: 0))
        model.observe(.reading(.failed(about: .reader, failure: .offline)))
        #expect(model.phase == .failed(.offline))
    }

    /// Picking a chapter in a strip sends the place held back by the pacing before it opens.
    @Test
    func leavingAStripChapterSendsThePlaceHeldBackFirst() {
        let sent = Sent()
        let model = opening(sent)
        model.apply(.opened(view("13", next: "14", mode: .strip)))
        model.apply(.leaves(session: session, chapter: ChapterId("13"), leaves: leaves(8)))
        model.stripAt(StripPos(image: 1, offset: 0), footVisible: false)
        let later = StripPos(image: 4, offset: 0.25)
        model.stripAt(later, footVisible: false)
        model.openChapter(ChapterId("14"))
        let last = sent.all.lastIndex { if case .reader(.stripAt(_, _, let at, _)) = $0 { at == later } else { false } }
        let open = sent.all.lastIndex { if case .reader(.openChapter) = $0 { true } else { false } }
        #expect(last != nil && open != nil && last! < open!)
    }
}
