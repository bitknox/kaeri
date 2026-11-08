// The app: one window over the store, on the Mac, iPad and iPhone.
//
// Built twice. Kaeri starts the app's own engine on the real sources and AniList. Kaeri
// Fixtures, compiled with FIXTURES against the testing engine, starts on a scene of the test kit's
// fixture titles in a new temporary directory, for tests and screenshots only.

import KaeriDesign
import KaeriKit
import KaeriPlayerUI
import KaeriStore
import KaeriStrings
import KaeriUI
import SwiftUI

@main
struct KaeriApp: App {
    #if os(macOS)
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    #else
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    #endif
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        #if os(macOS)
        // The one window: browsing, and the player in its place while it plays (M4).
        Window("Kaeri", id: "main") {
            content
                .hostsReader(delegate.store.reader)
                // The reader's end page reads the store and opens Add a source: the cover does
                // not see what `content` sets inside.
                .environment(delegate.store)
                .environment(delegate.navigator)
                .focusedSceneValue(\.playerInFront, delegate.player.inMainWindow)
        }
        // The Fixtures app keeps the exact 1280 by 800 the shot scripts rely on. The real app opens
        // at 80% of the display's usable area, centred; a remembered frame still wins over this.
        #if FIXTURES
        .defaultSize(width: 1280, height: 800)
        #else
        .defaultWindowPlacement { _, context in
            let area = context.defaultDisplay.visibleRect
            let width = min(max(area.width * 0.8, 1280), area.width)
            let height = min(max(area.height * 0.8, 800), area.height)
            return WindowPlacement(
                CGPoint(x: area.midX - width / 2, y: area.midY - height / 2),
                size: CGSize(width: width, height: height))
        }
        #endif
        .windowResizability(.contentMinSize)
        // Up at every launch, whatever the launch restored or however it came about.
        .defaultLaunchBehavior(.presented)
        .commands {
            KaeriCommands(
                store: delegate.store, navigator: delegate.navigator,
                playerBack: .init(
                    isUp: { [player = delegate.player] in player.inMainWindow },
                    back: { [player = delegate.player] in player.requestBack() })
            ) {
                PlaybackMenu(model: delegate.player)
            } playerWindowItems: {
                PictureInPictureMenuItem(model: delegate.player)
            }
        }
        // The reader: one window, one book at a time, with its own Go menu.
        Window(L.readerWindow, id: ReaderWindow.id) {
            ReaderWindowView(model: delegate.store.reader)
                .environment(delegate.store)
                .environment(delegate.navigator)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 760, height: 1000)
        .restorationBehavior(.disabled)
        .defaultLaunchBehavior(.suppressed)
        .commands { ReaderMenu() }
        // Play trailer: the trailer watched properly, in a window of its own.
        WindowGroup(L.titlePlayTrailer, id: TrailerRequest.windowId, for: TrailerRequest.self) { $request in
            if let request {
                TrailerWindowView(request: request)
                    .environment(delegate.store)
                    .preferredColorScheme(.dark)
                    .frame(minWidth: 480, minHeight: 270)
            }
        }
        .defaultSize(width: 960, height: 540)
        .windowResizability(.contentMinSize)
        .restorationBehavior(.disabled)
        .defaultLaunchBehavior(.suppressed)
        // An episode's discussion: one window beside the player, which another episode swaps.
        Window(L.discussionOpen, id: DiscussionKey.windowId) {
            DiscussionWindowHost()
                .environment(delegate.store)
                .preferredColorScheme(.dark)
        }
        .defaultSize(width: 480, height: 720)
        .restorationBehavior(.disabled)
        .defaultLaunchBehavior(.suppressed)
        #else
        WindowGroup {
            content
                .hostsReader(delegate.store.reader)
                // The reader's end page reads the store and opens Add a source: the cover does
                // not see what `content` sets inside.
                .environment(delegate.store)
                .environment(delegate.navigator)
                // The player covers the window it plays in, so it is in front while it shows.
                .focusedSceneValue(\.playerInFront, delegate.player.isPresented)
        }
        .commands {
            KaeriCommands(store: delegate.store, navigator: delegate.navigator) {
                PlaybackMenu(model: delegate.player)
            } playerWindowItems: {
                EmptyView()
            }
            // The reader's keys, with a hardware keyboard on iPad and iPhone.
            ReaderMenu()
        }
        #endif
    }

    private var content: some View {
        RootView()
            // Handoff: what another device hands over opens here.
            .continuesHandoff()
            .hostsPlayer(delegate.player, over: PlayerCoverSheets())
            .environment(\.playerUp, delegate.player.isPresented)
            .environment(delegate.store)
            .environment(delegate.navigator)
            #if os(macOS)
            .frame(minWidth: 960, minHeight: 600)
            #endif
            .task { await delegate.start() }
            // Where the app is, for the engine: in the background its torrents wait.
            .onChange(of: scenePhase, initial: true) { _, phase in
                switch phase {
                case .active: delegate.phases.scene(.active)
                case .background: delegate.phases.scene(.background)
                default: delegate.phases.scene(.inactive)
                }
            }
            // Notifications: their permission and the week's schedule looked at again on the way
            // back, and the next background refresh asked for on the way out. Downloads: a window
            // to go on in after the app leaves.
            .onChange(of: scenePhase) { old, phase in
                switch phase {
                case .active:
                    delegate.notices.cameForward()
                    // A TV driven tries again at once rather than at the end of its wait.
                    delegate.store.tv.retryNow()
                    // Back from a licensed service after long enough to have watched an episode:
                    // the engine says so, and the notice asks.
                    delegate.store.returnedToFront()
                    #if os(iOS)
                    delegate.store.continued.phase(foreground: true, rows: delegate.store.downloads.rows)
                    #endif
                case .inactive where old == .active:
                    #if os(iOS)
                    // Still in front: the last moment the system opens a downloads window.
                    delegate.store.continued.leaving(rows: delegate.store.downloads.rows)
                    #endif
                case .background:
                    delegate.notices.wentAway()
                    #if os(iOS)
                    delegate.store.continued.phase(foreground: false, rows: delegate.store.downloads.rows)
                    #endif
                default: break
                }
            }
    }
}
