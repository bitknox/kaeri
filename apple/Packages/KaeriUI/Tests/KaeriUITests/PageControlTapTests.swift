#if os(macOS)
// A failed page's Try again takes its own tap: in the pager it retries without turning the page
// (iPad R7P-1), in the strip without bringing the bar (iPhone P12). Real clicks through a window,
// on the page the readers draw, under the tap surface both readers put over their pages.

import KaeriKit
@testable import KaeriUI
import AppKit
import SwiftUI
import Testing

/// Clicks a view through an offscreen window, as HitAreaTests does. SwipeCancelTests drags in it.
@MainActor
final class TapWindow {
    private final class Window: NSWindow {
        override var canBecomeKey: Bool { true }
        override var isKeyWindow: Bool { true }
        override var isVisible: Bool { true }
    }

    private let window: Window
    let size: CGSize

    init(_ view: some View, size: CGSize) {
        _ = NSApplication.shared
        self.size = size
        window = Window(
            contentRect: NSRect(origin: .zero, size: size), styleMask: [.borderless],
            backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(
            rootView: view.frame(width: size.width, height: size.height, alignment: .topLeading))
        window.alphaValue = 0
        window.setFrameOrigin(NSPoint(x: -20_000, y: -20_000))
        window.orderFrontRegardless()
    }

    func settle() async throws {
        for _ in 0..<20 { try await turn(0.01) }
    }

    /// A click at a point in the view's space, top left at zero, and time for a tap to end.
    func click(_ point: CGPoint) async throws {
        let location = NSPoint(x: point.x, y: size.height - point.y)
        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
            let event = NSEvent.mouseEvent(
                with: type, location: location, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1,
                pressure: type == .leftMouseDown ? 1 : 0)!
            window.sendEvent(event)
            try await turn(0.005)
        }
        for _ in 0..<10 { try await turn(0.01) }
    }

    /// A mouse button event at a point in the view's space.
    func mouse(_ type: NSEvent.EventType, at point: CGPoint) async throws {
        let event = NSEvent.mouseEvent(
            with: type, location: NSPoint(x: point.x, y: size.height - point.y), modifierFlags: [],
            timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
            eventNumber: 0, clickCount: 1, pressure: type == .leftMouseUp ? 0 : 1)!
        window.sendEvent(event)
        try await turn(0.01)
    }

    /// Time for a gesture's change, or an animation, to land.
    func wait(_ seconds: Double) async throws {
        let end = Date().addingTimeInterval(seconds)
        while Date() < end { try await turn(0.02) }
    }

    /// A key pressed and let go at the window, as the keyboard sends it.
    func key(_ characters: String, code: UInt16) async throws {
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            let event = NSEvent.keyEvent(
                with: type, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber, context: nil, characters: characters,
                charactersIgnoringModifiers: characters, isARepeat: false, keyCode: code)!
            window.sendEvent(event)
            try await turn(0.01)
        }
        for _ in 0..<10 { try await turn(0.01) }
    }

    var firstResponder: NSResponder? { window.firstResponder }

    /// The window's accessibility tree, as VoiceOver walks it.
    func accessibilityElements() -> [NSObject] {
        var found: [NSObject] = []
        func walk(_ element: Any, depth: Int) {
            guard depth < 60, found.count < 50_000, let element = element as? NSObject else { return }
            found.append(element)
            for child in element.accessibilityChildren() ?? [] { walk(child, depth: depth + 1) }
        }
        if let root = window.contentView { walk(root, depth: 0) }
        return found
    }

    /// Every label, title and value in the window's accessibility tree.
    func accessibilityText() -> [String] {
        accessibilityElements().flatMap { Self.text(of: $0) }
    }

    static func text(of element: NSObject) -> [String] {
        [element.accessibilityLabel(), element.accessibilityTitle(), element.accessibilityValue() as? String]
            .compactMap { $0 }.filter { !$0.isEmpty }
    }

    func close() { window.close() }

    private func turn(_ seconds: Double) async throws {
        Self.spin(seconds)
        try await Task.sleep(for: .milliseconds(1))
    }

    /// The run loop for a while, out of the async context it cannot be run from.
    private static func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
}

@MainActor
private final class Heard {
    var retries = 0
    var pageTaps = 0
    var controls: [CGRect] = []
}

extension WindowEventTests {
    @MainActor
    @Suite(.serialized)
    struct PageControlTapTests {
        private let size = CGSize(width: 700, height: 900)

        private func failed(_ heard: Heard) -> some View {
            PageFailed(page: 9, failure: .offline, theme: .dark) { heard.retries += 1 }
                .frame(width: size.width, height: size.height)
        }
}

    /// The pager: the page under the surface, as PagedReader lays a leaf out.
    private func pager(_ heard: Heard) -> some View {
        ZStack { failed(heard) }
            .frame(width: size.width, height: size.height)
            .contentShape(.rect)
            .readerTaps { _ in heard.pageTaps += 1 }
            .onPreferenceChange(ReaderControlFrames.self) { [heard] frames in
                MainActor.assumeIsolated { heard.controls = frames }
            }
    }

    /// The strip: the page in a scroll view under the surface, as StripReader lays its parts out.
    private func strip(_ heard: Heard) -> some View {
        ScrollView(.vertical) {
            VStack(spacing: 0) {
                Color.gray.frame(height: 300)
                failed(heard)
                Color.gray.frame(height: 300)
            }
        }
        .frame(width: size.width, height: size.height)
        .readerTaps { _ in heard.pageTaps += 1 }
        .onPreferenceChange(ReaderControlFrames.self) { [heard] frames in
            MainActor.assumeIsolated { heard.controls = frames }
        }
    }

    private func check(_ view: some View, _ heard: Heard) async throws {
        let window = TapWindow(view, size: size)
        defer { window.close() }
        try await window.settle()
        let button = try #require(heard.controls.first, "Try again is marked as the page's control")
        #expect(heard.controls.count == 1)

        try await window.click(CGPoint(x: button.midX, y: button.midY))
        #expect(heard.retries == 1, "Try again retried")
        #expect(heard.pageTaps == 0, "and the page heard nothing: no turn, no bar")

        // Anywhere else on the page is still the page's.
        try await window.click(CGPoint(x: size.width - 40, y: size.height - 40))
        #expect(heard.retries == 1)
        #expect(heard.pageTaps == 1, "a tap off the button turns or brings the bar as before")
    }

    @Test func aPagesTryAgainDoesNotTurnThePage() async throws {
        let heard = Heard()
        try await check(pager(heard), heard)
    }

    @Test func aStripPagesTryAgainDoesNotBringTheBar() async throws {
        let heard = Heard()
        try await check(strip(heard), heard)
    }

    @Test func aTapNearAControlIsTheControlsAndOneFarOffIsThePages() {
        let button = CGRect(x: 100, y: 200, width: 120, height: 40)
        #expect(!ReaderControlFrames.pageTakes(CGPoint(x: 160, y: 220), controls: [button]))
        #expect(!ReaderControlFrames.pageTakes(CGPoint(x: 98, y: 220), controls: [button]))
        #expect(ReaderControlFrames.pageTakes(CGPoint(x: 160, y: 300), controls: [button]))
        #expect(ReaderControlFrames.pageTakes(CGPoint(x: 160, y: 220), controls: []))
    }
}
#endif
