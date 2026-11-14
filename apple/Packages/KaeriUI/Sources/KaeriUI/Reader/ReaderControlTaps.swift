// A control in a page (a failed page's Try again, the end page's Continue) takes its own tap. The
// pager's and the strip's taps are simultaneous gestures over the whole page, so without this a tap
// on Try again also turned the page or brought the bar.

import SwiftUI

/// Where the controls in the reader's pages are, in the tap surface's space.
struct ReaderControlFrames: PreferenceKey {
    /// The coordinate space a tap surface names, which its controls report their frames in.
    static let space = "reader-tap-surface" // i18n-skip: a coordinate space's name

    static let defaultValue: [CGRect] = []

    static func reduce(value: inout [CGRect], nextValue: () -> [CGRect]) {
        value += nextValue()
    }

    /// Whether a tap at `point` is the page's: not on any control, nor a few points around one,
    /// where a finger that meant the control lands.
    static func pageTakes(_ point: CGPoint, controls: [CGRect]) -> Bool {
        !controls.contains { $0.insetBy(dx: -4, dy: -4).contains(point) }
    }
}

extension View {
    /// Marks a control in a page: the tap surface around it leaves taps on it to it.
    func takesItsOwnTaps() -> some View {
        background {
            GeometryReader { proxy in
                Color.clear.preference(
                    key: ReaderControlFrames.self, value: [proxy.frame(in: .named(ReaderControlFrames.space))])
            }
        }
    }

    /// The reader's taps over its pages: `single` with the tap's place, and on touch `double` for
    /// a double tap, which a single tap waits on. A tap on a control in a page goes to the control
    /// alone.
    func readerTaps(double: ((CGPoint) -> Void)? = nil, single: @escaping (CGPoint) -> Void) -> some View {
        modifier(ReaderTapSurface(double: double, single: single))
    }
}

/// Keeps the frames of the controls under it and lets through only the taps that miss them.
private struct ReaderTapSurface: ViewModifier {
    let double: ((CGPoint) -> Void)?
    let single: (CGPoint) -> Void
    @State private var controls = Controls()

    /// The frames as last laid out, read when a tap ends. Nothing is drawn from them, so a change
    /// redraws nothing. Set and read on the main thread only.
    private final class Controls: @unchecked Sendable {
        var frames: [CGRect] = []
    }

    func body(content: Content) -> some View {
        let surface = content
            .coordinateSpace(.named(ReaderControlFrames.space))
            .onPreferenceChange(ReaderControlFrames.self) { [controls] frames in controls.frames = frames }
        // A double tap is waited for only where one does something.
        if let double {
            surface.simultaneousGesture(
                tap(count: 2, then: double).exclusively(before: tap(count: 1, then: single)))
        } else {
            surface.simultaneousGesture(tap(count: 1, then: single))
        }
    }

    private func tap(count: Int, then act: @escaping (CGPoint) -> Void) -> some Gesture {
        SpatialTapGesture(count: count, coordinateSpace: .named(ReaderControlFrames.space)).onEnded { [controls] tap in
            if ReaderControlFrames.pageTakes(tap.location, controls: controls.frames) { act(tap.location) }
        }
    }
}
