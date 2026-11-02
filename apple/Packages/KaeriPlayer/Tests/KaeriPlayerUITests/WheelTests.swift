// The scroll wheel over the Mac player: what is left to a menu or panel over the video, and how
// small trackpad deltas add up to volume steps.

#if os(macOS)
import AppKit
import SwiftUI
import Testing

@testable import KaeriPlayerUI

@MainActor
struct WheelTests {
    @Test func smallTrackpadDeltasAddUpToSteps() {
        var steps = WheelSteps()
        // Eight deltas of 1.25 points: each alone is less than a step.
        let points = (0..<8).map { _ in steps.volume(1.25, precise: true) }
        #expect(points.reduce(0, +) == 2)
        #expect(points.allSatisfy { $0 == 0 || $0 == 1 })
        #expect(steps.volume(-5, precise: true) == -1)
    }

    @Test func aWheelNotchIsTwoPoints() {
        var steps = WheelSteps()
        #expect(steps.volume(1, precise: false) == 2)
        #expect(steps.volume(-3, precise: false) == -6)
    }

    @Test func horizontalScrollsGatherIntoSeeks() {
        var steps = WheelSteps()
        #expect(steps.seek(3, ended: false) == nil)
        #expect(steps.seek(5, ended: false) == -2)
        #expect(steps.seek(-2, ended: true) == 0.5)
        #expect(steps.seek(0, ended: true) == nil)
    }

    /// A player-sized window with a menu's list hanging over the video, laid out by SwiftUI.
    @Test func aScrollOverAMenuIsTheMenus() throws {
        let watcher = NSView()
        let root = ZStack {
            Color.black
            Color.clear.contentShape(.rect).onTapGesture {}
            MenuScroll { VStack { ForEach(0..<3) { Text("Track \($0)") } } }
                .frame(width: 200, height: 120)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
        }
        .background(Holder(view: watcher))
        .frame(width: 800, height: 450)
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 800, height: 450), styleMask: [.borderless],
            backing: .buffered, defer: false)
        let host = NSHostingView(rootView: root)
        window.contentView = host
        host.layoutSubtreeIfNeeded()
        try #require(watcher.window === window, "the watcher is in the window")
        func claimed(_ x: CGFloat, _ y: CGFloat, stops: [CGRect] = []) -> Bool {
            // AppKit's points, from the bottom left; the stops, from the top left.
            ScrollMonitor.claimed(
                at: CGPoint(x: x, y: 450 - y), stops: stops, hit: host.hitTest(NSPoint(x: x, y: y)), player: watcher)
        }
        // The list, short enough to fit, is still a scroll view.
        #expect(claimed(700, 400))
        #expect(!claimed(400, 225), "the video")
        #expect(claimed(100, 400, stops: [CGRect(x: 0, y: 0, width: 300, height: 200)]), "a panel")
        #expect(!claimed(400, 225, stops: [CGRect(x: 0, y: 0, width: 300, height: 200)]))
    }

    struct Holder: NSViewRepresentable {
        let view: NSView
        func makeNSView(context: Context) -> NSView { view }
        func updateNSView(_ view: NSView, context: Context) {}
    }

    @Test func theVolumeLineReadsThePointer() {
        #expect(VolumeLine.level(at: 36, width: 72) == 50)
        #expect(VolumeLine.level(at: -10, width: 72) == 0)
        #expect(VolumeLine.level(at: 90, width: 72) == 100)
        #expect(VolumeLine.level(at: 10, width: 0) == 0)
    }
}
#endif

/// The volume line moves on the step, before mpv says it has the new volume, and steps build
/// on the one asked for. The engine still sets it.
@MainActor
struct VolumeStepTests {
    func volume(_ rig: Rig, _ value: Double) async throws {
        rig.mpv.emit(.property(name: "volume", value: .double(value)))
        try await until("volume \(value)") { rig.model.volume == value }
    }

    @Test func theLineMovesBeforeMpvAnswers() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.stepVolume(by: -5)
        rig.model.stepVolume(by: -5)
        #expect(rig.model.shownVolume == 90)
        #expect(rig.controls.suffix(2) == [.volume(-5), .volume(-5)])
        // The first step's report: the second is still on its way.
        try await volume(rig, 95)
        #expect(rig.model.shownVolume == 90)
        try await volume(rig, 90)
        #expect(rig.model.shownVolume == 90)
        #expect(rig.model.volumeAsked == nil)
    }

    @Test func stepsStopAtMpvsLimits() async throws {
        let rig = Rig()
        try await rig.opened()
        try await volume(rig, 128)
        rig.model.stepVolume(by: 5)
        #expect(rig.controls.last == .volume(2))
        #expect(rig.model.shownVolume == PlayerModel.volumeMax)
        try await volume(rig, 130)
        let sent = rig.controls.count
        rig.model.stepVolume(by: 5)
        #expect(rig.controls.count == sent, "nothing past the top")
    }

    @Test func aDragSetsTheLevel() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.setVolume(40.4)
        #expect(rig.controls.last == .volume(-60))
        let sent = rig.controls.count
        rig.model.setVolume(39.8)
        #expect(rig.controls.count == sent, "the same whole percent")
        #expect(rig.model.shownVolume == 40)
    }

    @Test func mpvsWordWinsOnceTheStepIsOld() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.stepVolume(by: 5)
        // mpv settles elsewhere, say a volume-max set lower, and says so a second on.
        rig.step(.seconds(1))
        try await volume(rig, 101)
        #expect(rig.model.shownVolume == 101)
    }
}
