// The cast drawer's motion on iPhone and iPad: how far the controller is open, from the mini
// (0) to full screen (1), as one value that a finger moves one to one, that a release hands its
// speed to, and that a zero-bounce spring settles. The spring can be caught at any point, so the
// drawer can be grabbed while it opens or closes.

import CoreGraphics
import Foundation
#if os(iOS)
import Observation
import QuartzCore
import UIKit
#endif

/// The zero-bounce spring of the standard motion token (`.spring(duration: 0.3, bounce: 0)`),
/// stepped by hand: a critically damped spring, solved exactly for each frame's time, toward 0
/// or 1. It stops on its target and never passes it.
struct DrawerSpring: Equatable {
    /// 0 on the mini, 1 open.
    var value: CGFloat
    /// In units of `value` per second.
    var velocity: CGFloat = 0
    var target: CGFloat

    /// The standard token's 0.3 s, as an angular frequency.
    static let omega: CGFloat = 2 * .pi / 0.3
    /// Close enough to call it there.
    static let rest: CGFloat = 0.0005

    var atRest: Bool { value == target && velocity == 0 }

    /// Move on by `dt` seconds. True once it rests on its target.
    @discardableResult
    mutating func step(_ dt: CGFloat) -> Bool {
        guard !atRest else { return true }
        let w = Self.omega
        let c1 = value - target
        let c2 = velocity + w * c1
        let decay = exp(-w * dt)
        let next = target + (c1 + c2 * dt) * decay
        let speed = (c2 - w * (c1 + c2 * dt)) * decay
        // Through the target is as far as it goes: no overshoot, so no bounce.
        if (next - target) * c1 < 0 || (abs(next - target) < Self.rest && abs(speed) < Self.rest * 20) {
            value = target
            velocity = 0
            return true
        }
        value = next
        velocity = speed
        return false
    }

    /// Where a release settles: the way a fling goes, past a slow speed; otherwise the nearer
    /// end. `speed` is in units of `value` per second.
    static func settles(at value: CGFloat, speed: CGFloat) -> CGFloat {
        if abs(speed) >= flick { return speed > 0 ? 1 : 0 }
        return value >= 0.5 ? 1 : 0
    }

    /// A fling, in units of `value` per second: about 300 pt/s over a screen's height.
    static let flick: CGFloat = 0.4
}

/// Which way a drag goes, read once it has moved far enough to tell.
enum DrawerDrag: Equatable {
    case up, down, across

    init(_ translation: CGSize) {
        if abs(translation.width) > abs(translation.height) {
            self = .across
        } else {
            self = translation.height < 0 ? .up : .down
        }
    }
}

#if os(iOS)
/// The drawer between the mini and the controller: one per tab shell, shared by the mini (which
/// a drag up opens it from), the cover, and the controller (which a drag down closes).
@Observable @MainActor
final class CastDrawer {
    /// 0 on the mini, 1 open, between the two as the finger or the spring has it.
    private(set) var progress: CGFloat = 0
    /// The controller's controls show: past a third of the way open. They fade on their own
    /// clock as this changes, as a fade is for what only appears.
    private(set) var revealed = false
    /// A finger holds the drawer.
    private(set) var held = false
    /// It rests closed on the mini, with no finger on it: the cover can go.
    private(set) var shut = true

    /// Points the box's top travels between the mini and the top of the screen, for one to one.
    @ObservationIgnored var travel: CGFloat = 600
    /// Where the mini is, for the travel of a drag up from it.
    @ObservationIgnored weak var anchor: CastMiniAnchor?
    /// A slider in the controller is being dragged: the drawer leaves that drag alone.
    @ObservationIgnored var sliding = false
    /// The controller's scroll is at its top, where a drag down closes the drawer rather than
    /// scrolling.
    @ObservationIgnored var atTop = true
    @ObservationIgnored private var spring = DrawerSpring(value: 0, target: 0)
    @ObservationIgnored private var grabbedAt: CGFloat = 0
    /// How far the finger had risen when it took hold, so the drawer moves from where it was.
    @ObservationIgnored private var grabbedRise: CGFloat = 0
    @ObservationIgnored private var ticker: DrawerTicker?

    /// Whether this drag moves the drawer: decided once per drag, at its first move.
    @ObservationIgnored private(set) var owner: Bool?

    /// Settle open or closed, from wherever it is and at the speed it has. A finger holding it
    /// keeps it: the release decides.
    func settle(open: Bool) {
        guard !held else { return }
        spring.target = open ? 1 : 0
        run()
    }

    /// Put it at an end at once: Reduce Motion's crossfade has no travel to make, and a drawer
    /// that comes up with nothing to grow from.
    func jump(open: Bool) {
        stop()
        spring = DrawerSpring(value: open ? 1 : 0, target: open ? 1 : 0)
        show(spring.value)
    }

    /// A drag's first move, `rise` points up from where it touched: whether the drawer takes it.
    /// From the mini, a drag up; on the controller, a drag down from the top of its scroll, or
    /// any drag up or down while it is still on its way, which catches it. Never a drag across,
    /// or one a slider has.
    func begin(_ drag: DrawerDrag, fromMini: Bool, rise: CGFloat) -> Bool {
        if let owner { return owner }
        let moving = !spring.atRest || (progress > 0 && progress < 1)
        let takes: Bool
        switch drag {
        case .across: takes = false
        case .up: takes = fromMini || moving
        case .down: takes = !fromMini && (moving || atTop)
        }
        owner = takes && !sliding
        if owner == true { grab(rise: rise) }
        return owner == true
    }

    /// The finger moved: `rise` points up from where it took hold, down when negative.
    func drag(rise: CGFloat) {
        guard held else { return }
        let value = min(max(grabbedAt + (rise - grabbedRise) / max(travel, 1), 0), 1)
        spring.value = value
        show(value)
    }

    /// The finger let go at `speed` points per second, up when positive. Where it settles:
    /// true open.
    @discardableResult
    func release(speed: CGFloat) -> Bool {
        // The drag is over; so is any slider's under the same finger.
        defer {
            owner = nil
            sliding = false
        }
        guard held else { return spring.target == 1 }
        held = false
        let velocity = speed / max(travel, 1)
        let open = DrawerSpring.settles(at: spring.value, speed: velocity) == 1
        spring.velocity = velocity
        spring.target = open ? 1 : 0
        run()
        return open
    }

    /// The gesture ended without a release (the system took it): settle where it is nearer.
    func cancel() {
        guard held else {
            owner = nil
            sliding = false
            return
        }
        release(speed: 0)
    }

    private func grab(rise: CGFloat) {
        stop()
        held = true
        grabbedAt = spring.value
        grabbedRise = rise
        spring.velocity = 0
        show(spring.value)
    }

    private func run() {
        guard !spring.atRest else {
            show(spring.value)
            return
        }
        if ticker == nil {
            ticker = DrawerTicker { [weak self] dt in self?.tick(dt) }
        }
        show(spring.value)
    }

    private func tick(_ dt: CFTimeInterval) {
        // A long frame (the app was away) is cut short so the step stays a step.
        if spring.step(CGFloat(min(dt, 1.0 / 30))) { stop() }
        show(spring.value)
    }

    private func stop() {
        ticker?.invalidate()
        ticker = nil
    }

    private func show(_ value: CGFloat) {
        if progress != value { progress = value }
        let reveal = value > 0.33
        if revealed != reveal { revealed = reveal }
        let closed = value == 0 && !held && ticker == nil
        if shut != closed { shut = closed }
    }
}

/// A display link that calls back with each frame's time since the last.
@MainActor
private final class DrawerTicker: NSObject {
    private var link: CADisplayLink?
    private var last: CFTimeInterval?
    private let tick: @MainActor (CFTimeInterval) -> Void

    init(tick: @escaping @MainActor (CFTimeInterval) -> Void) {
        self.tick = tick
        super.init()
        let link = CADisplayLink(target: self, selector: #selector(frame(_:)))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 60, maximum: 120, preferred: 120)
        link.add(to: .main, forMode: .common)
        self.link = link
    }

    @objc private func frame(_ link: CADisplayLink) {
        let now = link.targetTimestamp
        let dt = last.map { now - $0 } ?? link.duration
        last = now
        tick(max(dt, 0))
    }

    func invalidate() {
        link?.invalidate()
        link = nil
    }
}
#endif
