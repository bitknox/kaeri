// A two-finger sideways swipe on a Mac's trackpad, read from its scroll events as a drag: the
// fingers' way across, then a let-go with their speed. The momentum after the fingers lift is
// the swipe's own and goes nowhere else. A mouse's sideways wheel is a step a notch.

import CoreGraphics
import Foundation

/// The scroll events of one gesture, as a drag. Cross-platform, so its tests run anywhere.
struct SwipeScroll {
    enum Phase: Equatable { case began, changed, ended, cancelled, wheel }

    enum Out: Equatable {
        /// Not a sideways swipe: the event goes on to whatever else scrolls.
        case pass
        /// The swipe's own event, with nothing to do.
        case take
        /// The fingers this far across, the way a drag's translation goes.
        case follow(CGFloat)
        /// The fingers lifted this far across, moving at this many points a second.
        case letGo(CGFloat, velocity: CGFloat)
        /// A mouse wheel's notch sideways, the way a drag would go.
        case step(CGFloat)
    }

    /// Points the fingers move before the swipe takes a way, sideways or not.
    static let slop: CGFloat = 6

    private enum Way { case undecided, sideways, other }
    private var way: Way = .undecided
    private var x: CGFloat = 0
    private var y: CGFloat = 0
    /// The last moves, for the speed at the let-go.
    private var recent: [(at: TimeInterval, dx: CGFloat)] = []
    /// The last gesture was a swipe: its momentum is its own.
    private var swiped = false

    /// One scroll event: its phase, whether it is the momentum after the fingers lifted, and its
    /// move the way the fingers went, at `at` seconds.
    mutating func feed(phase: Phase, momentum: Bool, dx: CGFloat, dy: CGFloat, at: TimeInterval) -> Out {
        if momentum { return swiped ? .take : .pass }
        switch phase {
        case .wheel:
            // A mouse wheel: no gesture, a notch at a time.
            guard abs(dx) > abs(dy), dx != 0 else { return .pass }
            return .step(dx)
        case .began:
            way = .undecided
            x = 0
            y = 0
            recent = []
            swiped = false
            return add(dx, dy, at)
        case .changed:
            return add(dx, dy, at)
        case .ended, .cancelled:
            defer { way = .undecided }
            guard way == .sideways else { return .pass }
            swiped = true
            let window = recent.filter { at - $0.at < 0.1 }
            let span = (window.last?.at ?? at) - (window.first?.at ?? at)
            let speed = span > 0 ? window.dropFirst().reduce(0) { $0 + $1.dx } / CGFloat(span) : 0
            return .letGo(phase == .cancelled ? 0 : x, velocity: phase == .cancelled ? 0 : speed)
        }
    }

    private mutating func add(_ dx: CGFloat, _ dy: CGFloat, _ at: TimeInterval) -> Out {
        x += dx
        y += dy
        recent.append((at, dx))
        if recent.count > 8 { recent.removeFirst() }
        if way == .undecided, max(abs(x), abs(y)) >= Self.slop {
            way = abs(x) > abs(y) ? .sideways : .other
        }
        switch way {
        case .undecided: return .take
        case .other: return .pass
        case .sideways: return .follow(x)
        }
    }
}

#if os(macOS)
import AppKit
import SwiftUI

/// The reader's page reading the trackpad: scroll events over it, in its window, go to
/// `handle`, which says whether it took them.
struct TrackpadSwipe: NSViewRepresentable {
    let active: Bool
    let handle: (SwipeScroll.Out) -> Bool

    @MainActor final class Coordinator {
        var monitor: Any?
        var swipe = SwipeScroll()
        var active = false
        var handle: (SwipeScroll.Out) -> Bool = { _ in false }
        /// The gesture began over the page: it stays the page's wherever the pointer goes.
        var tracking = false
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        let coordinator = context.coordinator
        coordinator.active = active
        coordinator.handle = handle
        coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak view] event in
            let taken = MainActor.assumeIsolated { () -> Bool in
                guard let view, let window = view.window, event.window === window else { return false }
                return coordinator.scrolled(event, over: view)
            }
            return taken ? nil : event
        }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        context.coordinator.active = active
        context.coordinator.handle = handle
    }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        if let monitor = coordinator.monitor { NSEvent.removeMonitor(monitor) }
        coordinator.monitor = nil
    }
}

extension TrackpadSwipe.Coordinator {
    func scrolled(_ event: NSEvent, over view: NSView) -> Bool {
        let phase: SwipeScroll.Phase = switch event.phase {
        case .began, .mayBegin: .began
        case .changed, .stationary: .changed
        case .ended: .ended
        case .cancelled: .cancelled
        default: .wheel
        }
        let momentum = !event.momentumPhase.isEmpty
        let inside = view.bounds.contains(view.convert(event.locationInWindow, from: nil))
        if phase == .began, !momentum {
            tracking = active && inside
            // A gesture that began off the page is none of the page's, its momentum included.
            if !tracking { swipe = SwipeScroll() }
        }
        // The momentum after a swipe is the swipe's; a wheel's notch counts only over the page;
        // the rest of a gesture only if it began there.
        if !momentum, phase == .wheel ? !(active && inside) : !tracking { return false }
        // The fingers' way: with natural scrolling the deltas follow them, otherwise they run
        // against them. A wheel's deltas are the page's way, as the system scrolls it.
        let flip: CGFloat = !event.hasPreciseScrollingDeltas || event.isDirectionInvertedFromDevice ? 1 : -1
        let out = swipe.feed(
            phase: phase, momentum: momentum, dx: event.scrollingDeltaX * flip, dy: event.scrollingDeltaY * flip,
            at: event.timestamp)
        if phase == .ended || phase == .cancelled { tracking = false }
        return handle(out)
    }
}
#endif
