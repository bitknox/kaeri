// Fit to Window (Cmd-0) undoes a pinch in every way of reading manga: the page on screen, a
// spread and pages one under another share the paged zoom, the long strip has its own.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct FitEveryModeTests {
    private func model(_ mode: ReaderMode) -> ReaderModel {
        let model = ReaderModel()
        let work = WorkId("a:900301")
        let chapter = ChapterId("c1")
        model.apply(.opened(ReaderView(
            session: 1, work: work, title: "The Lantern Ferry", medium: .manga,
            chapter: ChapterRow(id: chapter, label: "1", state: .unread), mode: mode,
            direction: .rightToLeft, start: ReadPlace(work: work, chapter: chapter))))
        return model
    }

    @Test func fitAsksThePagesToUndoTheirPinch() {
        for mode in [ReaderMode.paged, .vertical] {
            let model = model(mode)
            ReaderActions.zoom(model, 0)
            #expect(model.pageZoomAsked?.step == 0, "\(mode)")
            #expect(model.stripFitAsked == 0, "\(mode)")
        }
    }

    @Test func fitAsksTheStripToUndoItsPinch() {
        let model = model(.strip)
        ReaderActions.zoom(model, 0)
        #expect(model.stripFitAsked == 1)
        #expect(model.pageZoomAsked == nil)
    }
}
