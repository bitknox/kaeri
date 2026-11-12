// Reading hunt round 5, Apple code audit: each test shows one bug and fails until it is fixed.
// They are off by default; KAERI_R5_AUDIT=1 runs them. A fixer turns a test on for good with
// the fix.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

private let audit = ProcessInfo.processInfo.environment["KAERI_R5_AUDIT"] != nil

@MainActor
private final class Sent {
    var all: [Command] = []
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

    /// R5-A1 (stuck): Continue from the end page into a chapter whose open fails. The engine says
    /// so with `ReadingEvent.failed(.reader)` and no `opened` (session.rs `laid_out`), but
    /// `observe` never settles the held end page, so it stays over the failure with its spinner
    /// and eats every tap; on iPhone and iPad the hidden chrome can't come back to close.
    @Test(.disabled(if: !audit, "R5-A1: a failed chapter open leaves the held end page up for good"))
    func a1_aChapterOpenThatFailsLetsTheHeldPageGo() {
        let model = onEndPage()
        model.openChapter(ChapterId("14"))
        #expect(model.held != nil)
        model.observe(.reading(.failed(about: .reader, failure: .notFound)))
        #expect(model.phase == .failed(.notFound))
        #expect(model.held?.landed == true, "the held end page must go so the failure and its buttons show")
    }

    /// R5-A2 (wrong state): the same failure names the chapter that was left, not the one that
    /// failed: the engine sends no `opened` for it, so `view` is still chapter 13 and
    /// ReaderChapterFailed reads "Chapter 13 didn't open".
    @Test(.disabled(if: !audit, "R5-A2: a failed chapter open names the chapter before it"))
    func a2_aFailedChapterOpenNamesTheChapterThatFailed() {
        let model = onEndPage()
        model.shown(1, how: .turn)
        model.openChapter(ChapterId("14"))
        model.observe(.reading(.failed(about: .reader, failure: .notFound)))
        #expect(model.view?.chapter.id != ChapterId("13"), "the failure screen names chapter 13, which opened fine")
    }

    /// R5-A3 (lost place): a strip report held back by the 500 ms pacing is dropped when the
    /// reader closes (`reset` cancels `pendingStrip`), so the engine keeps the place from up to
    /// half a second before: a fling or Space then Esc resumes images earlier.
    @Test(.disabled(if: !audit, "R5-A3: closing within 500 ms of a strip scroll loses the last place"))
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

    /// R5-A4 (data loss, minor): Not a credit page waits out its 8 s Undo before the engine
    /// hears, but the countdown stops while the app is away (`countdown` skips while
    /// `noticesPaused`, which includes `away`). Kept, then the app leaves and is ended in the
    /// background: the keep is never sent.
    @Test(.disabled(if: !audit, "R5-A4: a page kept as not a credit page is lost when the app leaves"))
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

    /// R5-A5 (wrong state): closed while opening, then another book opened before the engine
    /// answered the first. `begin` clears `closedOpening`, so the first book's late `opened`
    /// is taken as the second: its chapter shows under the second book's request.
    @Test(.disabled(if: !audit, "R5-A5: a book closed while opening shows up as the next book"))
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
    }
}
