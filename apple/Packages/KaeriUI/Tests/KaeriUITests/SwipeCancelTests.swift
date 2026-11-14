#if os(macOS)
// A swipe the system cancels, with no end, leaves the pages at rest, not where the drag had them
// (iPad R7P-3: a window resized by its corner mid-swipe left a slice of the next page showing
// until the next swipe). Real mouse events through a window; the drag stops by the pages being
// disabled mid-drag, which cancels it.

@testable import KaeriStore
@testable import KaeriUI
import KaeriKit
import SwiftUI
import Testing

@MainActor @Observable
private final class Gate {
    var live = true
}

@MainActor
private final class Seen {
    var frames: [CGRect] = []
}

extension WindowEventTests {
    @MainActor
    @Suite(.serialized)
    struct SwipeCancelTests {
        private let size = CGSize(width: 700, height: 900)
        private let work = "a:900301"

        /// A paged chapter of five pages, all failed so each shows its Try again, whose frame says
        /// where its page is. The middle page is on screen.
        private func chapter() -> ReaderModel {
            let model = ReaderModel()
            model.open(work: work, chapter: "c1")
            model.surfaceChanged(ReaderSurface(widthPx: 1400, heightPx: 1800, scale: 2, eink: false, twoPages: false, dark: true))
            let row = ChapterRow(
                id: ChapterId("c1"), label: "Chapter 1", state: .unread, number: "1", title: nil, volume: nil,
                published: nil, pages: 5, newChapter: false, download: nil, versions: 1, bookmarked: false)
            model.apply(.opened(ReaderView(
                session: 1, work: WorkId(work), title: "The Lantern Ferry", medium: .manga, chapter: row,
                mode: .paged, direction: .leftToRight,
                start: ReadPlace(work: WorkId(work), chapter: ChapterId("c1"), page: 0, strip: nil, novel: nil),
                next: nil, previous: nil, settings: nil)))
            var leaves = (0..<5).map { page in
                Leaf(index: UInt32(page), parts: [
                    LeafPart(page: UInt32(page), kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1), rotate: false, half: nil),
                ], endPage: false)
            }
            leaves.append(Leaf(index: 5, parts: [], endPage: true))
            model.apply(.leaves(session: 1, chapter: ChapterId("c1"), leaves: leaves))
            for page in 0..<5 {
                model.apply(.pageFailed(session: 1, chapter: ChapterId("c1"), page: UInt32(page), failure: .offline))
            }
            model.shown(2, how: .turn)
            return model
        }

        /// How far the page nearest `rest` is from it.
        private func shift(_ seen: Seen, from rest: CGFloat) -> CGFloat {
            seen.frames.map { abs($0.midX - rest) }.min() ?? .infinity
        }

        @Test func aSwipeCancelledMidDragLeavesThePagesAtRest() async throws {
            let model = chapter()
            let gate = Gate()
            let seen = Seen()
            let view = PagedReader(model: model, theme: .dark, size: size, scale: 2, tone: PageTone(), paper: false) { _ in }
                .onPreferenceChange(ReaderControlFrames.self) { [seen] frames in
                    MainActor.assumeIsolated { seen.frames = frames }
                }
            let window = TapWindow(Wrapper(gate: gate) { view }, size: size)
            defer { window.close() }
            try await window.settle()
            #expect(model.current == 2)
            let rest = try #require(seen.frames.min { abs($0.midX - size.width / 2) < abs($1.midX - size.width / 2) }).midX

            // A drag toward the page before, held, from clear of the page's Try again: a press on
            // the button is the button's, and the pages never see it.
            let button = try #require(seen.frames.first { abs($0.midX - rest) < 1 })
            let start = CGPoint(x: 300, y: 120)
            #expect(!button.insetBy(dx: -30, dy: -30).contains(start))
            try await window.mouse(.leftMouseDown, at: start)
            for step in 1...8 {
                try await window.mouse(.leftMouseDragged, at: CGPoint(x: start.x + CGFloat(step) * 10, y: start.y))
            }
            try await window.wait(0.2)
            #expect(shift(seen, from: rest) > 40, "the pages follow the drag")

            // The system takes it: no end comes to the swipe.
            gate.live = false
            try await window.wait(0.3)
            try await window.mouse(.leftMouseUp, at: CGPoint(x: start.x + 80, y: start.y))
            try await window.wait(1.5)
            #expect(model.current == 2, "no turn")
            #expect(shift(seen, from: rest) < 1, "a slice of the next page shows: the pages stayed where the drag had them")
        }
    }
}

/// Holds the gate in the body, so turning it off redraws the pages disabled and without hit
/// testing, which cancels a drag under way: its `onEnded` never comes.
private struct Wrapper<Content: View>: View {
    let gate: Gate
    @ViewBuilder let content: () -> Content

    var body: some View {
        content().allowsHitTesting(gate.live).disabled(!gate.live)
    }
}
#endif
