// Cmd-0 in a long strip fits it back to its column, and undoes a pinch's zoom as well: a pinch the
// system cut off settles at the scale the fingers left, and Fit is the way back from there.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct StripFitTests {
    private func model(_ mode: ReaderMode) -> ReaderModel {
        let model = ReaderModel()
        let work = WorkId("a:900303")
        let chapter = ChapterId("c1")
        model.apply(.opened(ReaderView(
            session: 1, work: work, title: "The Lantern Ferry", medium: .manga,
            chapter: ChapterRow(id: chapter, label: "1", state: .unread), mode: mode,
            direction: .leftToRight, start: ReadPlace(work: work, chapter: chapter))))
        return model
    }

    @Test func fitAsksTheStripToUndoItsZoom() {
        let model = model(.strip)
        ReaderActions.zoom(model, 0)
        #expect(model.stripFitAsked == 1)
    }

    @Test func zoomingInOrOutLeavesThePinchAlone() {
        let model = model(.strip)
        ReaderActions.zoom(model, 1)
        ReaderActions.zoom(model, -1)
        #expect(model.stripFitAsked == 0)
    }
}
