// The Mac's paged reader under a two-finger sideways swipe, as the trackpad sends it: scroll
// events with phases, sent through the app as the window server would send them. The page
// turns, and a swipe up and down turns nothing.

#if os(macOS)
import AppKit
import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import SwiftUI
import Testing

@MainActor
@Suite(.serialized, .timeLimit(.minutes(1)))
struct PagedTrackpadSwipeTests {
    private let session: UInt64 = 1
    private let size = CGSize(width: 800, height: 600)

    private func model() -> ReaderModel {
        let model = ReaderModel()
        model.send = { _ in }
        let work = WorkId("a:900301")
        let chapter = ChapterRow(
            id: ChapterId("13"), label: "Chapter 13", state: .unread, number: nil, title: nil, volume: nil,
            published: nil, pages: 4, newChapter: false, download: nil, versions: 1, bookmarked: false)
        model.apply(.opened(ReaderView(
            session: session, work: work, title: "The Lantern Ferry", medium: .manga, chapter: chapter,
            mode: .paged, direction: .leftToRight,
            start: ReadPlace(work: work, chapter: chapter.id, page: 0, strip: nil, novel: nil),
            next: nil, previous: nil, settings: nil)))
        let leaves = (0..<4).map { page in
            Leaf(index: UInt32(page), parts: [
                LeafPart(page: UInt32(page), kind: .story, file: nil, place: UnitRect(x: 0, y: 0, width: 1, height: 1), rotate: false, half: nil),
            ], endPage: false)
        }
        model.apply(.leaves(session: session, chapter: chapter.id, leaves: leaves))
        return model
    }

    private func window(_ model: ReaderModel) -> NSWindow {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: ReaderScreen(model: model) {}.frame(width: size.width, height: size.height)
            .environment(AppStore()).environment(Navigator()))
        window.setFrameOrigin(NSPoint(x: -20_000, y: -20_000))
        window.orderFrontRegardless()
        return window
    }

    private static func turn() {
        RunLoop.main.run(until: Date().addingTimeInterval(0.005))
    }

    /// The run loop turned.
    private func spin(_ seconds: Double) async {
        let end = Date().addingTimeInterval(seconds)
        while Date() < end {
            Self.turn()
            try? await Task.sleep(for: .milliseconds(1))
        }
    }

    private func until(_ seconds: Double, _ done: () -> Bool) async -> Bool {
        let end = Date().addingTimeInterval(seconds)
        while !done(), Date() < end { await spin(0.05) }
        return done()
    }

    /// One trackpad scroll event over the window's middle: `phase` as CoreGraphics numbers it
    /// (1 began, 2 changed, 4 ended), `momentum` likewise, deltas in points as the event gives them.
    private func scroll(_ window: NSWindow, phase: Int64, momentum: Int64 = 0, dx: Int32, dy: Int32 = 0) -> NSEvent? {
        guard let event = CGEvent(
            scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: dy, wheel2: dx, wheel3: 0)
        else { return nil }
        event.setIntegerValueField(.scrollWheelEventIsContinuous, value: 1)
        event.setIntegerValueField(.scrollWheelEventScrollPhase, value: phase)
        event.setIntegerValueField(.scrollWheelEventMomentumPhase, value: momentum)
        event.setIntegerValueField(.scrollWheelEventPointDeltaAxis1, value: Int64(dy))
        event.setIntegerValueField(.scrollWheelEventPointDeltaAxis2, value: Int64(dx))
        // The window's middle, in the window server's coordinates, which run down from the top
        // of the main screen.
        let frame = window.frame
        let top = NSScreen.screens.first?.frame.maxY ?? 0
        event.location = CGPoint(x: frame.midX, y: top - frame.midY)
        event.setIntegerValueField(.mouseEventWindowUnderMousePointer, value: Int64(window.windowNumber))
        event.setIntegerValueField(.mouseEventWindowUnderMousePointerThatCanHandleThisEvent, value: Int64(window.windowNumber))
        return NSEvent(cgEvent: event)
    }

    /// A swipe of eight moves of `dx` and `dy` points, then the fingers lift and two momentum
    /// events follow.
    private func swipe(_ window: NSWindow, dx: Int32, dy: Int32 = 0) async throws {
        var events: [NSEvent?] = [scroll(window, phase: 1, dx: dx, dy: dy)]
        events += (0..<7).map { _ in scroll(window, phase: 2, dx: dx, dy: dy) }
        events.append(scroll(window, phase: 4, dx: 0))
        events += [scroll(window, phase: 0, momentum: 1, dx: dx, dy: dy), scroll(window, phase: 0, momentum: 3, dx: 0)]
        for event in events {
            let event = try #require(event)
            #expect(TrackpadSwipe.place(of: event, in: window) != nil, "the event falls in the window: \(event.locationInWindow)")
            // Through the app's dispatch, where its local monitors see each event.
            NSApp.sendEvent(event)
            await spin(0.016)
        }
    }

    /// Fingers moving left, as the trackpad says them with natural scrolling off: the deltas run
    /// against the fingers. Left goes forward in a book read left to right.
    @Test func aTwoFingerSwipeTurnsThePage() async throws {
        let model = model()
        let window = window(model)
        defer {
            window.contentView = nil
            window.orderOut(nil)
        }
        await spin(0.3)
        #expect(model.current == 0)
        try await swipe(window, dx: 40)
        #expect(await until(3) { model.current == 1 }, "the next page: \(model.current)")
        // And back, the other way.
        try await swipe(window, dx: -40)
        #expect(await until(3) { model.current == 0 }, "back: \(model.current)")
    }

    @Test func upAndDownTurnsNothing() async throws {
        let model = model()
        let window = window(model)
        defer {
            window.contentView = nil
            window.orderOut(nil)
        }
        await spin(0.3)
        try await swipe(window, dx: 2, dy: 40)
        await spin(0.5)
        #expect(model.current == 0)
    }
}
#endif
