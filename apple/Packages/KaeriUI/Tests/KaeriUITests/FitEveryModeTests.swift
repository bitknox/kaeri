// Fit to Window (Cmd-0) undoes a pinch in every way of reading manga: the page on screen and a
// spread share the paged zoom; the long strip and pages one under another, which flow as it
// does, have the strip's.

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
        let model = model(.paged)
        ReaderActions.zoom(model, 0)
        #expect(model.pageZoomAsked?.step == 0)
        #expect(model.stripFitAsked == 0)
    }

    @Test func fitAsksTheStripToUndoItsPinch() {
        for mode in [ReaderMode.strip, .vertical] {
            let model = model(mode)
            ReaderActions.zoom(model, 0)
            #expect(model.stripFitAsked == 1, "\(mode)")
            #expect(model.pageZoomAsked == nil, "\(mode)")
        }
    }
}
