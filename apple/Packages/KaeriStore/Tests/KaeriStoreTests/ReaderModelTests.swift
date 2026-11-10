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

    @Test func closingClearsTheRequest() {
        let (model, _) = model()
        model.open(work: "a:900301")
        model.close()
        #expect(model.request == nil)
        #expect(model.phase == .closed)
    }
}
