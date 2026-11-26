// Watch together outside the player: its screen (the room's place), the people button that opens
// it on iPhone, a room's invite links, and on the Mac the ask before the player's window closes
// while in a room.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The Watch together section on the Mac and a wide iPad: the room's place.
struct TogetherScreen: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        RoomPlace()
            .task { store.room.loadRecent() }
    }
}

#if os(iOS)
/// The people button in Home's toolbar, before the account button.
struct TogetherButton: ToolbarContent {
    @Environment(AppStore.self) private var store

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Button(L.navWatchTogether, systemImage: store.room.inRoom ? "person.2.fill" : "person.2") {
                store.room.placeShown = true
            }
        }
    }
}
#endif

/// A room's invite links, which the engine reads: `kaeri://room#…`, the way the system hands one
/// to the app, and `https://kaeri.app/room#…`, as people paste and share them.
enum RoomLinks {
    static func isInvite(_ url: URL) -> Bool {
        switch url.scheme?.lowercased() {
        case "kaeri": url.host?.lowercased() == "room"
        case "https": url.host?.lowercased() == "kaeri.app" && url.path.lowercased() == "/room"
        default: false
        }
    }
}

extension Navigator {
    /// Watch together on screen: its section on the Mac and a wide iPad, Home with the place
    /// pushed on iPhone and in a narrow window.
    func showTogether(store: AppStore) {
        #if os(iOS)
        if UITraitCollection.current.horizontalSizeClass == .compact || UIDevice.current.userInterfaceIdiom == .phone {
            if section != .home { section = .home }
            paths[.home] = []
            store.room.placeShown = true
            return
        }
        #endif
        store.room.placeShown = false
        if section != .together { section = .together }
    }
}

/// The window's part in the room: an opened invite shows the room in Watch together, and on the
/// Mac the red button and ⌘W ask first while in a room.
public struct RoomWindow: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    public init() {}

    public func body(content: Content) -> some View {
        content
            .onOpenURL { url in
                guard RoomLinks.isInvite(url) else { return }
                store.room.open(invite: url.absoluteString)
                navigator.showTogether(store: store)
            }
            .onChange(of: store.room.placeShown) { _, shown in
                if shown { navigator.showTogether(store: store) }
            }
            #if os(macOS)
            .background { RoomCloseGuard(asks: { store.room.asksBeforeClosing() }) { _ in } }
            #endif
    }
}

#if os(macOS)
/// Asks before the window closes while in a room: its delegate stands behind one that asks
/// first, and answers everything else as it did. SwiftUI keeps the window's delegate; this one
/// only wraps it.
struct RoomCloseGuard: NSViewRepresentable {
    let asks: @MainActor () -> Bool
    let found: (NSWindow?) -> Void

    func makeNSView(context: Context) -> Holder { Holder(asks: asks, found: found) }
    func updateNSView(_ view: Holder, context: Context) {
        view.asker.asks = asks
        view.install()
    }

    final class Holder: NSView {
        let asker: CloseAsker
        let found: (NSWindow?) -> Void

        init(asks: @escaping @MainActor () -> Bool, found: @escaping (NSWindow?) -> Void) {
            asker = CloseAsker(asks: asks)
            self.found = found
            super.init(frame: .zero)
        }

        required init?(coder: NSCoder) { fatalError("not used") }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            install()
            found(window)
        }

        /// In front of the window's own delegate, once, and again should SwiftUI set another.
        func install() {
            guard let window, window.delegate !== asker else { return }
            asker.original = window.delegate
            window.delegate = asker
        }
    }
}

/// The window delegate that asks: `windowShouldClose` asks while in a room, and every other
/// message goes to the delegate it stands in front of.
final class CloseAsker: NSObject, NSWindowDelegate {
    var asks: @MainActor () -> Bool
    weak var original: NSWindowDelegate?

    init(asks: @escaping @MainActor () -> Bool) {
        self.asks = asks
    }

    @MainActor
    func windowShouldClose(_ sender: NSWindow) -> Bool {
        if asks() { return false }
        return original?.windowShouldClose?(sender) ?? true
    }

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (original?.responds(to: selector) ?? false)
    }

    override func forwardingTarget(for selector: Selector!) -> Any? {
        original?.responds(to: selector) == true ? original : super.forwardingTarget(for: selector)
    }
}
#endif
