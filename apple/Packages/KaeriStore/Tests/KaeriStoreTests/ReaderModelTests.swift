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

    @Test func closingClearsTheRequest() {
        let (model, _) = model()
        model.open(work: "a:900301")
        model.close()
        #expect(model.request == nil)
        #expect(model.phase == .closed)
    }
}
