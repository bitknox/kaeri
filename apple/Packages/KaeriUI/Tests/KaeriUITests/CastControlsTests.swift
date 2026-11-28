// The Mac's cast controls as VoiceOver reads them: while the TV reconnects, everything in the panel
// but Stop casting is dimmed and off, and says so; and the Connected dialog's Unmute keeps its
// whole label beside the TV's volume, fixed or a slider.

#if os(macOS)
import KaeriKit
@testable import KaeriStore
import KaeriStrings
@testable import KaeriUI
import AppKit
import SwiftUI
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct CastControlsTests {
    let store = AppStore()
    let navigator = Navigator()

    struct Element {
        let name: String
        let role: String
        let enabled: Bool
        let frame: NSRect
    }

    /// Every element in the accessibility tree of `view`, hosted in a window of `size`.
    func elements(_ view: some View, size: NSSize) -> [Element] {
        _ = NSApplication.shared
        // SwiftUI builds its accessibility tree only for a client; this asks for it in this
        // process, as VoiceOver's presence would.
        NSApplication.shared.accessibilitySetValue(
            true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
        let window = NSWindow(
            contentRect: NSRect(x: -20_000, y: -20_000, width: size.width, height: size.height), styleMask: [.borderless],
            backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: view
            .frame(width: size.width, height: size.height, alignment: .topLeading)
            .environment(store)
            .environment(navigator))
        window.alphaValue = 0
        window.orderFrontRegardless()
        defer { window.close() }
        runAwake(0.5)
        var found: [Element] = []
        func walk(_ node: Any, depth: Int) {
            // Read by name: the protocol's accessors are optional, and Swift can't pick between
            // their overloads on an element of an unknown class.
            func read(_ key: String) -> Any? {
                guard let node = node as? NSObject, node.responds(to: Selector(key)) else { return nil }
                return node.value(forKey: key)
            }
            guard depth < 40 else { return }
            let label = read("accessibilityLabel") as? String ?? ""
            let title = read("accessibilityTitle") as? String ?? ""
            // A static text carries its words as its value.
            let value = read("accessibilityValue") as? String ?? ""
            let name = [label, title, value].first { !$0.isEmpty } ?? ""
            if !name.isEmpty {
                // The getter of `accessibilityEnabled` is `isAccessibilityEnabled`.
                let enabled = (node as? NSObject).map {
                    $0.responds(to: Selector("isAccessibilityEnabled")) ? ($0.value(forKey: "accessibilityEnabled") as? Bool ?? true) : true
                } ?? true
                found.append(Element(
                    name: name,
                    role: read("accessibilityRole") as? String ?? "",
                    enabled: enabled,
                    frame: (read("accessibilityFrame") as? NSValue)?.rectValue ?? .zero))
            }
            for child in read("accessibilityChildren") as? [Any] ?? [] { walk(child, depth: depth + 1) }
        }
        walk(window.contentView as Any, depth: 0)
        return found
    }

    /// The cast held on Living room, playing episode 3 at 600 s, with the TV muted.
    func hold(volume: FfiVolumeControl, canMute: Bool = true, reconnecting: Bool = false) {
        let cast = store.tv.cast
        cast.link = StillLink()
        var view = FfiCastSessionView(
            state: .playing, button: .connected, rev: 1, deviceKey: "cast:living", deviceName: "Living room",
            item: FfiCastItem(
                id: 900_001, title: "Lanterns", episode: "3", episodeTitle: nil, image: nil, // i18n-skip
                position: 600, duration: 1420, paused: false, waiting: nil, loading: false),
            queue: [], notice: nil, note: nil, level: volume == .level ? 0.4 : nil, muted: true,
            volumeControl: volume, ownReceiver: true, canMute: canMute)
        view.reconnecting = reconnecting
        cast.take(view: view)
    }

    @Test func whileTheTvReconnectsEverythingButStopCastingIsOff() {
        hold(volume: .level, reconnecting: true)
        #expect(store.tv.cast.reconnecting)
        let all = elements(MacCastPanel(artHeight: 60), size: NSSize(width: MacCastMetrics.panelWidth, height: 720))
            .filter { $0.role != NSAccessibility.Role.staticText.rawValue }
        let dimmed = [
            L.castPlayHere, L.playerBack10, L.actionPause, L.playerForward10, L.playerNextEpisode,
            L.playerPosition, L.playerUnmute,
        ]
        for name in dimmed {
            let shown = all.filter { $0.name == name }
            #expect(!shown.isEmpty, "\(name) is in the tree: \(all.map { "\($0.name) \($0.role)" })")
            #expect(shown.allSatisfy { !$0.enabled }, "\(name) is off: \(shown.map { $0.role })")
        }
        let stop = all.filter { $0.name == L.castStop }
        #expect(!stop.isEmpty)
        #expect(stop.allSatisfy { $0.enabled }, "Stop casting stays live")
    }

    @Test func theConnectedDialogsUnmuteKeepsItsWholeLabel() throws {
        // The pill alone, at its own width, read the same way.
        let alone = elements(CastMuteButton(muted: true, touch: false) {}.fixedSize(), size: NSSize(width: 400, height: 60))
        let ideal = try #require(alone.first { $0.name == L.playerUnmute }?.frame.width)
        // A TV that refused a level but takes a mute, and one that takes both.
        for volume in [FfiVolumeControl.fixed, .level] {
            hold(volume: volume)
            let all = elements(CastConnectedDialog(), size: NSSize(width: CastMetrics.popoverWidth ?? 280, height: 240))
            let unmute = try #require(all.first { $0.name == L.playerUnmute && $0.role == NSAccessibility.Role.button.rawValue },
                                      "\(all.map { "\($0.name) \($0.role)" })")
            #expect(unmute.frame.width >= ideal - 0.5,
                    "\(volume): Unmute is \(unmute.frame.width) wide of its \(ideal): \(all.map { "\($0.name) \($0.frame.width)" })")
        }
    }

    @Test func aTvWhoseOwnVolumeIsInChargeOffersNoMute() {
        hold(volume: .fixed, canMute: false)
        let all = elements(CastConnectedDialog(), size: NSSize(width: CastMetrics.popoverWidth ?? 280, height: 240))
        let names = all.map(\.name)
        #expect(names.contains(L.castVolumeFixed), "\(names)")
        #expect(!names.contains(L.playerUnmute) && !names.contains(L.playerMute), "\(names)")
        #expect(!names.contains(L.playerMuted), "the TV's muted flag means nothing here: \(names)")
    }
}

/// A session link that tells the engine nothing and hears nothing back.
private final class StillLink: CastSessionLink, @unchecked Sendable {
    func state() -> FfiCastSessionView {
        FfiCastSessionView(
            state: .idle, button: .connected, rev: 0, deviceKey: "", deviceName: "", item: nil, queue: [],
            notice: nil, note: nil, level: nil, muted: nil, volumeControl: .fixed, ownReceiver: true, canMute: false)
    }
    func next() async -> FfiCastSessionView? { nil }
    func connect(key: String, from: FfiCastFrom?) throws {}
    func disconnect() {}
    func playHere() -> FfiCastFrom? { nil }
    func command(_ command: FfiTvCommand) {}
    func card() -> (any FfiCastSessionProtocol)? { nil }
    func away(_ away: Bool) {}
    func played(_ request: TVPlayRequest) {}
}
#endif
