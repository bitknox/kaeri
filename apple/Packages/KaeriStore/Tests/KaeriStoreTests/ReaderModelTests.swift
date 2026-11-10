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
        #expect(model.goTo?.how == .scroll)
    }

    @Test func closingClearsTheRequest() {
        let (model, _) = model()
        model.open(work: "a:900301")
        model.close()
        #expect(model.request == nil)
        #expect(model.phase == .closed)
    }
}
