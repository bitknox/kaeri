// The reader model: when it opens, what it sends for the settings sheet, and what it does with a
// page that failed.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
struct ReaderModelTests {
    private func model() -> (ReaderModel, Box) {
        let model = ReaderModel()
        let box = Box()
        model.send = { box.commands.append($0) }
        return (model, box)
    }

    final class Box { var commands: [Command] = [] }

    private let surface = ReaderSurface(widthPx: 780, heightPx: 1688, scale: 2, eink: false, twoPages: false, dark: true)

    private func opens(_ commands: [Command]) -> Int {
        commands.filter { if case .reader(.open) = $0 { true } else { false } }.count
    }

    @Test func nothingOpensUntilTheScreenHasASize() {
        let (model, box) = model()
        model.open(work: "a:900301")
        #expect(opens(box.commands) == 0)
        model.surfaceChanged(surface)
        #expect(opens(box.commands) == 1)
        #expect(model.phase == .opening)
    }

    private func chapterAsks(_ commands: [Command]) -> Int {
        commands.filter { if case .reading(.loadChapters) = $0 { true } else { false } }.count
    }

    /// The open asks for the chapter list of nothing: the Chapters sheet asks when it shows
    /// without one, once.
    @Test func openingAsksForNoChapterList() {
        let (model, box) = model()
        model.open(work: "a:900301")
        model.surfaceChanged(surface)
        #expect(chapterAsks(box.commands) == 0)
        model.loadChapters()
        #expect(chapterAsks(box.commands) == 1)
        model.observe(.reading(.chapters(work: WorkId("a:900301"), edition: nil, volumes: [], failure: nil)))
        model.loadChapters()
        #expect(chapterAsks(box.commands) == 1)
    }

    @Test func aSecondSizeOnlyTellsTheEngineAboutTheSurface() {
        let (model, box) = model()
        model.open(work: "a:900301")
        model.surfaceChanged(surface)
        var wider = surface
        wider.widthPx = 1688
        wider.heightPx = 780
        model.surfaceChanged(wider)
        #expect(opens(box.commands) == 1)
    }

    /// The surfaces the engine heard of after the open, by session.
    private func surfaces(_ commands: [Command]) -> [(UInt64, ReaderSurface)] {
        commands.compactMap { if case .reader(.surface(let session, let surface)) = $0 { (session, surface) } else { nil } }
    }

    private func opened(_ model: ReaderModel, session: UInt64) {
        let work = WorkId("a:900301")
        let chapter = ChapterRow(
            id: ChapterId("c1"), label: "Chapter 1", state: .unread, number: nil, title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
        model.apply(.opened(ReaderView(
            session: session, work: work, title: "The Lantern Ferry", medium: .manga, chapter: chapter,
            mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: work, chapter: chapter.id, page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
    }

    @Test func aSizeChangedWhileTheBookOpensReachesTheEngineOnceItHasOpened() {
        let (model, box) = model()
        model.open(work: "a:900301")
        model.surfaceChanged(surface)
        var turned = surface
        turned.widthPx = 1688
        turned.heightPx = 780
        turned.twoPages = true
        // Turned while the engine is still opening: there is no session to send it to yet.
        model.surfaceChanged(turned)
        #expect(surfaces(box.commands).isEmpty)
        opened(model, session: 7)
        let sent = surfaces(box.commands)
        #expect(sent.count == 1)
        #expect(sent.first?.0 == 7)
        #expect(sent.first?.1 == turned)
        #expect(opens(box.commands) == 1)
    }

    @Test func aSizeThatStayedWhileTheBookOpenedIsNotSentAgain() {
        let (model, box) = model()
        model.open(work: "a:900301")
        model.surfaceChanged(surface)
        opened(model, session: 7)
        #expect(surfaces(box.commands).isEmpty)
        // Turned there and back while opening: the engine already has this size.
        let (other, otherBox) = self.model()
        other.open(work: "a:900301")
        other.surfaceChanged(surface)
        var turned = surface
        turned.widthPx = 1688
        other.surfaceChanged(turned)
        other.surfaceChanged(surface)
        opened(other, session: 8)
        #expect(surfaces(otherBox.commands).isEmpty)
    }

    /// The novel turns the engine heard, as (page, pages, how).
    private func novelTurns(_ commands: [Command]) -> [(UInt32, UInt32, TurnHow)] {
        commands.compactMap {
            if case .reader(.novelAt(_, _, _, let page, let pages, let how)) = $0 { (page, pages, how) } else { nil }
        }
    }

    @Test func aNovelsEndPageReachesTheEngineAsThePageAfterTheLast() {
        let (model, box) = model()
        model.open(work: "a:900302")
        model.surfaceChanged(surface)
        opened(model, session: 3)
        let pos = NovelPos(spine: 4, path: [2, 1], offset: 30)
        model.novelAt(pos, page: 6, pages: 7, how: .turn)
        model.novelEnded()
        let turns = novelTurns(box.commands)
        #expect(turns.count == 2)
        #expect(turns.last?.0 == 7)
        #expect(turns.last?.1 == 7)
        #expect(turns.last?.2 == .turn)
        let last = box.commands.last
        if case .reader(.novelAt(let session, let chapter, let at, _, _, _)) = last {
            #expect(session == 3)
            #expect(chapter == ChapterId("c1"))
            #expect(at == pos)
        } else {
            Issue.record("the end page was not sent: \(String(describing: last))")
        }
    }

    @Test func aNovelEndPageWithNoPageReportedSendsNothing() {
        let (model, box) = model()
        model.open(work: "a:900302")
        model.surfaceChanged(surface)
        opened(model, session: 3)
        model.novelEnded()
        #expect(novelTurns(box.commands).isEmpty)
    }

    @Test func theStripChoiceIsTheSeriesOwn() {
        let (model, box) = model()
        model.open(work: "a:900303")
        model.chooseMode(.strip)
        let set = box.commands.compactMap { command -> (ReaderSettingKey, SettingScope)? in
            if case .reader(.setSetting(_, _, _, let key, let scope, _)) = command { (key, scope) } else { nil }
        }
        #expect(set.contains { $0.0 == .mode && $0.1 == .series })
    }

    @Test func aFirstPageThatFailsFailsTheChapter() {
        let (model, _) = model()
        model.open(work: "a:900301")
        model.apply(.pageFailed(session: 0, chapter: ChapterId("c1"), page: 0, failure: .offline))
        #expect(model.phase == .failed(.offline))
        #expect(!ReadingFailure.offline.words.isEmpty)
    }

    @Test func theViewFollowsADirectionChangedInTheReader() {
        let (model, _) = model()
        let work = WorkId("a:900301")
        let chapter = ChapterRow(
            id: ChapterId("c3"), label: "Chapter 3", state: .unread, number: nil, title: nil, volume: nil,
            published: nil, pages: 3, newChapter: false, download: nil, versions: 1, bookmarked: false)
        model.apply(.opened(ReaderView(
            session: 1, work: work, title: "The Lantern Ferry", medium: .manga, chapter: chapter,
            mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: work, chapter: chapter.id, page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        func settings(_ work: WorkId?, direction: String) -> ReaderSettingsView {
            ReaderSettingsView(medium: .manga, work: work, rows: [
                ReaderSettingRow(key: .direction, value: .choice(name: direction), scope: .series,
                                 detected: false, overridden: true, choices: []),
            ])
        }
        // The Settings pane's defaults leave the open book alone.
        model.apply(.settings(settings(nil, direction: "ltr")))
        #expect(model.view?.direction == .rightToLeft)
        model.apply(.settings(settings(work, direction: "ltr")))
        #expect(model.view?.direction == .leftToRight)
        #expect(model.view?.mode == .paged)
    }

    @Test func aModeChangedInTheReaderKeepsThePageOnScreen() {
        let (model, _) = model()
        let work = WorkId("a:900301")
        let chapter = ChapterRow(
            id: ChapterId("c3"), label: "Chapter 3", state: .unread, number: nil, title: nil, volume: nil,
            published: nil, pages: 6, newChapter: false, download: nil, versions: 1, bookmarked: false)
        model.apply(.opened(ReaderView(
            session: 1, work: work, title: "The Lantern Ferry", medium: .manga, chapter: chapter,
            mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: work, chapter: chapter.id, page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        let leaves = (0..<6).map { page in
            Leaf(index: UInt32(page), parts: [
                LeafPart(page: UInt32(page), kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1),
                         rotate: false, half: nil),
            ], endPage: false)
        }
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: leaves))
        model.shown(4, how: .turn)
        func mode(_ name: String) -> ReaderSettingsView {
            ReaderSettingsView(medium: .manga, work: work, rows: [
                ReaderSettingRow(key: .mode, value: .choice(name: name), scope: .series,
                                 detected: false, overridden: true, choices: []),
            ])
        }
        // Pages to the strip: the strip opens at the page that was on screen.
        model.apply(.settings(mode("strip")))
        #expect(model.view?.mode == .strip)
        #expect(model.stripRestore?.image == 4)
        // The strip scrolled on to image 2; back to pages, that page is shown, with no way back
        // offered as if it had been a jump.
        model.position = (2, 6)
        model.apply(.settings(mode("paged")))
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: leaves))
        #expect(model.current == 2)
        #expect(model.goTo?.leaf == 2)
        #expect(model.goTo?.how == .placed)
    }

    /// A place by page alone, as Moved sends it, scrolls a strip to that page's image.
    @Test func aPlaceByPageAloneScrollsTheStripToItsImage() {
        let (model, _) = model()
        let work = WorkId("a:900302")
        let chapter = ChapterRow(
            id: ChapterId("c4"), label: "Chapter 4", state: .unread, number: nil, title: nil, volume: nil,
            published: nil, pages: 6, newChapter: false, download: nil, versions: 1, bookmarked: false)
        model.apply(.opened(ReaderView(
            session: 1, work: work, title: "The Lantern Ferry", medium: .manga, chapter: chapter,
            mode: .strip, direction: .leftToRight,
            start: ReadPlace(work: work, chapter: chapter.id, page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        let leaves = (0..<6).map { page in
            Leaf(index: UInt32(page), parts: [
                LeafPart(page: UInt32(page), kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1),
                         rotate: false, half: nil),
            ], endPage: false)
        } + [Leaf(index: 6, parts: [], endPage: true)]
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: leaves))
        model.apply(.goTo(session: 1, place: ReadPlace(work: work, chapter: chapter.id, page: 4, strip: nil, novel: nil)))
        #expect(model.stripRestore == StripPos(image: 4, offset: 0))
        // A place with its strip position keeps that position.
        model.apply(.goTo(session: 1, place: ReadPlace(
            work: work, chapter: chapter.id, page: 2, strip: StripPos(image: 2, offset: 0.5), novel: nil)))
        #expect(model.stripRestore == StripPos(image: 2, offset: 0.5))
    }

    /// Twenty pages as single leaves, or as a cover and spreads of two after it.
    private func layout(_ chapter: ChapterId, spreads: Bool) -> [Leaf] {
        let groups: [[UInt32]] = spreads
            ? [[0]] + stride(from: 1, to: 20, by: 2).map { $0 + 1 < 20 ? [$0, $0 + 1] : [$0] }
            : (0..<20).map { [UInt32($0)] }
        return groups.enumerated().map { index, pages in
            Leaf(index: UInt32(index), parts: pages.map { page in
                LeafPart(page: page, kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1),
                         rotate: false, half: nil)
            }, endPage: false)
        } + [Leaf(index: UInt32(groups.count), parts: [], endPage: true)]
    }

    @Test func aTurnBetweenPagesAndSpreadsKeepsThePage() {
        let (model, _) = model()
        let work = WorkId("a:900301")
        let chapter = ChapterRow(
            id: ChapterId("c13"), label: "Chapter 13", state: .unread, number: nil, title: nil, volume: nil,
            published: nil, pages: 20, newChapter: false, download: nil, versions: 1, bookmarked: false)
        // Opened at page 10, upright.
        model.apply(.opened(ReaderView(
            session: 1, work: work, title: "The Lantern Ferry", medium: .manga, chapter: chapter,
            mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: work, chapter: chapter.id, page: 9, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: false)))
        #expect(model.current == 9)
        // Turned on its side: the spread that holds page 10, then upright again at page 10.
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: true)))
        #expect(model.leaves[model.current].parts.map(\.page) == [9, 10])
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: false)))
        #expect(model.current == 9)
        // A page before the one it opened at stays where it is, rather than go forward to it.
        model.shown(3, how: .turn)
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: true)))
        #expect(model.leaves[model.current].parts.map(\.page) == [3, 4])
        model.shown(1, how: .turn)
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: false)))
        #expect(model.current == 1)
        // The end page stays the end page.
        model.shown(20, how: .turn)
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: true)))
        #expect(model.leaves[model.current].endPage)
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: false)))
        #expect(model.current == 20)
    }

    /// The second page of a spread, shown alone before the spread, comes back alone after it,
    /// however many times the screen turns.
    @Test func aRoundTripThroughSpreadsKeepsTheSecondPageOfTheSpread() {
        let (model, _) = model()
        let work = WorkId("a:900301")
        let chapter = ChapterRow(
            id: ChapterId("c13"), label: "Chapter 13", state: .unread, number: nil, title: nil, volume: nil,
            published: nil, pages: 20, newChapter: false, download: nil, versions: 1, bookmarked: false)
        model.apply(.opened(ReaderView(
            session: 1, work: work, title: "The Lantern Ferry", medium: .manga, chapter: chapter,
            mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: work, chapter: chapter.id, page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: false)))
        // Page 11 alone, then the spread of 10 and 11, then 11 alone again, twice over.
        model.shown(10, how: .turn)
        for _ in 0..<2 {
            model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: true)))
            #expect(model.leaves[model.current].parts.map(\.page) == [9, 10])
            model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: false)))
            #expect(model.current == 10)
        }
        // A turn in spreads goes by the spread turned to: its first page.
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: true)))
        model.shown(model.current + 1, how: .turn)
        #expect(model.leaves[model.current].parts.map(\.page) == [11, 12])
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: false)))
        #expect(model.current == 11)
        // Back to the spread with 10 and 11 by a turn: its first page, not the one held before.
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: true)))
        model.shown(model.current - 1, how: .turn)
        model.apply(.leaves(session: 1, chapter: chapter.id, leaves: layout(chapter.id, spreads: false)))
        #expect(model.current == 9)
    }

    @Test func closingClearsTheRequest() {
        let (model, _) = model()
        model.open(work: "a:900301")
        model.close()
        #expect(model.request == nil)
        #expect(model.phase == .closed)
    }
}
