// Starting a watch party from Watch together on the Mac, laid out in a window as the app's is: the
// start sheet, the server step and the lobby, on the testing engine's fake Syncplay server. The
// window's toolbar stays the split view's throughout, search and all: a navigation stack in the
// detail column takes it over, and the two can pass it back and forth until AppKit stops the app.

#if os(macOS)
import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import AppKit
import SwiftUI
import Testing

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct PartyStartLayoutTests {
    /// A window that lays out and draws as an on-screen one does, out of sight.
    private final class Window: NSWindow {
        override var canBecomeKey: Bool { true }
        override var isKeyWindow: Bool { true }
        override var isVisible: Bool { true }
    }

    private static let search = "com.apple.SwiftUI.search"
    private static let stackBack = "com.apple.SwiftUI.navigationStack.back"

    private static func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    private func turn(_ seconds: Double = 0.02) async throws {
        Self.spin(seconds)
        try await Task.sleep(for: .milliseconds(1))
    }

    private func waitFor(_ what: String, seconds: Double = 15, _ done: () -> Bool, layout: () -> Void) async throws {
        let end = Date().addingTimeInterval(seconds)
        while !done() {
            try #require(Date() < end, "waited for \(what)")
            layout()
            try await turn()
        }
    }

    private func settle(_ layout: () -> Void, turns: Int = 20) async throws {
        for _ in 0..<turns { layout(); try await turn() }
    }

    private func toolbar(_ window: NSWindow) -> [String] {
        window.toolbar?.items.map(\.itemIdentifier.rawValue) ?? []
    }

    private func window(showing store: AppStore, _ navigator: Navigator) -> Window {
        let window = Window(
            contentRect: NSRect(x: 0, y: 0, width: 1_200, height: 800),
            styleMask: [.titled, .closable, .resizable, .miniaturizable, .fullSizeContentView],
            backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: RootView().environment(store).environment(navigator))
        window.alphaValue = 0
        window.setFrameOrigin(NSPoint(x: -20_000, y: -20_000))
        window.orderFrontRegardless()
        return window
    }

    @Test func startingAPartyFromWatchTogetherKeepsTheWindowsToolbar() async throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory
            .appending(path: "kaeri-ui-\(UUID().uuidString)", directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore()
        let launch = EngineLaunch(engine: .fixtures(.watching), home: .override(root: root), platform: .macOs, secrets: .host)
        await store.start(launch, host: AppHost(secrets: MemorySecrets()))
        defer { store.shutdown() }
        try #require(store.phase == .running, "\(store.phase)")
        let fake = try #require(store.startFakeParty())
        defer { fake.stop() }

        let navigator = Navigator()
        navigator.section = .together
        let window = window(showing: store, navigator)
        defer { window.close() }
        let layout = { window.layoutIfNeeded(); window.displayIfNeeded() }
        try await settle(layout)
        let party = store.party
        try await waitFor("the form", { party.form != nil }, layout: layout)
        let place = toolbar(window)
        #expect(place.contains(Self.search), "\(place)")

        // Start a party: nothing to pick first, so the first time the server and a name. Watch
        // together on a title page comes to the same step with its episode.
        party.beginStart()
        #expect(party.picking == .server(nil))
        try await settle(layout)
        store.loadContinue()
        try await waitFor("Continue", { !store.continuing.queue(store.titles).all.isEmpty }, layout: layout)
        let pick = try #require(store.continuing.queue(store.titles).all.first)
        let episode = PartyEpisode(id: pick.entry.id, episode: String(pick.episode))
        party.start(episode)
        #expect(party.picking == .server(episode))
        try await settle(layout)
        party.pickServer = fake.server()
        party.pickName = "Ada"
        party.startOnServer()
        #expect(party.lobbyShown)

        // The lobby, in the room, with the sources checked: Back joins the section's toolbar,
        // which keeps its search.
        try await waitFor("the room", { party.party != nil && party.members != nil }, layout: layout)
        try await waitFor("the sources", {
            if case .checking? = party.party?.sources { return false }
            return party.party?.sources != nil
        }, layout: layout)
        try await settle(layout, turns: 40)
        #expect(party.inLobbyRoom)
        let lobby = toolbar(window)
        #expect(lobby.contains(Self.search), "\(lobby)")
        #expect(!lobby.contains(Self.stackBack), "\(lobby)")
        #expect(lobby.count == place.count + 1, "\(place) then \(lobby)")

        // Back, and start again on the server used last: straight into a new lobby, with
        // nothing picked and no sheet.
        party.lobbyShown = false
        try await settle(layout)
        #expect(toolbar(window) == place)
        let form = try #require(party.form)
        store.apply(.partyForm(PartyForm(
            recentServers: [fake.server()], suggestedServer: form.suggestedServer,
            suggestedRunBy: form.suggestedRunBy, room: form.room, name: "Ada")))
        party.beginStart()
        #expect(party.picking == nil)
        #expect(party.lobbyShown)
        try await waitFor("the room again", { party.inLobbyRoom && party.members != nil }, layout: layout)
        try await settle(layout, turns: 40)
        #expect(toolbar(window).contains(Self.search))
    }

    /// Every Watch together state the boards draw, one after another in the same window, as the
    /// engine would bring them.
    @Test func everyWatchTogetherStateSettles() async throws {
        _ = NSApplication.shared
        let root = FileManager.default.temporaryDirectory
            .appending(path: "kaeri-ui-\(UUID().uuidString)", directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore()
        let launch = EngineLaunch(engine: .fixtures(.together), home: .override(root: root), platform: .macOs, secrets: .host)
        await store.start(launch, host: AppHost(secrets: MemorySecrets()))
        defer { store.shutdown() }
        try #require(store.phase == .running, "\(store.phase)")

        let navigator = Navigator()
        navigator.section = .together
        let window = window(showing: store, navigator)
        defer { window.close() }
        let layout = { window.layoutIfNeeded(); window.displayIfNeeded() }
        try await settle(layout)
        for scene in TogetherScene.allCases {
            let staged = await store.party.stage(scene, store: store)
            #expect(staged, "\(scene)")
            try await settle(layout, turns: 10)
            #expect(toolbar(window).contains(Self.search), "\(scene)")
        }
    }
}
#endif
