// Where the reader lives. On iPhone and iPad it covers the app. On the Mac it takes the main
// window over, one book at a time: the sidebar and the toolbar go, the window keeps its size,
// place and full screen, and closing it shows the window as it was, scrolled where it was. The
// Go menu's keys work on both, with a hardware keyboard on iPad.

import KaeriStore
import KaeriStrings
import SwiftUI

extension View {
    /// Show the reader when the store has one open.
    public func hostsReader(_ model: ReaderModel) -> some View {
        modifier(ReaderHost(model: model))
    }
}

private struct ReaderHost: ViewModifier {
    @Bindable var model: ReaderModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var isOpen: Binding<Bool> {
        Binding(get: { model.request != nil }, set: { if !$0 { model.close() } })
    }

    @ViewBuilder func body(content: Content) -> some View {
        #if os(iOS)
        content.fullScreenCover(isPresented: isOpen) {
            ReaderScreen(model: model) {}
                .ignoresSafeArea()
                .background(Color.black)
        }
        #else
        // The window under the reader stays as it is, so going back finds it where it was. The
        // reader crossfades in over it and out again.
        content
            .toolbarVisibility(model.request != nil ? .hidden : .automatic, for: .windowToolbar)
            // In full screen the toolbar's strip stays at the top, drawn over the page in the
            // window's own colours even with nothing in it. While a book is open it comes with the
            // menu bar only, when the pointer goes to the top of the screen.
            .windowToolbarFullScreenVisibility(model.request != nil ? .onHover : .automatic)
            .overlay {
                if model.request != nil {
                    ReaderScreen(model: model) {}
                        .ignoresSafeArea()
                        .background(TrafficLights(visible: model.chromeShown))
                        .background(CloseKey { model.close() })
                        .background(WindowFront(model: model))
                        .transition(.opacity)
                }
            }
            .animation(reduceMotion ? nil : .standard, value: model.request != nil)
            .background(MenuKeys(keys: MenuKeys.Keys(reading: model.request != nil, reader: ReaderInFront.of(model))))
        #endif
    }
}

#if os(macOS)
import AppKit

/// ⌘W closes the book, not the window under it: the menu bar's Close would take the window
/// away with the reader still open in it. Answered before the menu, in the reader's window only.
private struct CloseKey: NSViewRepresentable {
    let close: () -> Void

    @MainActor final class Coordinator {
        var monitor: Any?
        var close: () -> Void = {}
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        let coordinator = context.coordinator
        coordinator.close = close
        coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak view] event in
            let closed = MainActor.assumeIsolated {
                guard let window = view?.window, event.window === window,
                      event.modifierFlags.intersection(.deviceIndependentFlagsMask) == .command,
                      event.charactersIgnoringModifiers?.lowercased() == "w"
                else { return false }
                coordinator.close()
                return true
            }
            return closed ? nil : event
        }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        context.coordinator.close = close
    }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        if let monitor = coordinator.monitor { NSEvent.removeMonitor(monitor) }
        coordinator.monitor = nil
    }
}

/// The menu bar's keys as they are now. SwiftUI hands a changed item to AppKit's menu only as
/// that menu opens, so a key that moves from one item to another, as ⌘L does from View to Go
/// when a book opens, stays with the old item until the menu is opened, and AppKit gives the
/// key to that item, dimmed or not. So after the reader's keys change, the next key brings every
/// menu up to date first, as opening it would.
private struct MenuKeys: NSViewRepresentable {
    /// What the menus' keys hang on: the View menu's while no book is open, the Go menu's.
    struct Keys: Equatable {
        var reading: Bool
        var reader: ReaderInFront?
    }

    let keys: Keys

    @MainActor final class Coordinator {
        var monitor: Any?
        var keys: Keys?
        /// The keys as the menu bar last had them.
        var held: Keys?
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView {
        let coordinator = context.coordinator
        coordinator.keys = keys
        coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            MainActor.assumeIsolated {
                if coordinator.held != coordinator.keys {
                    coordinator.held = coordinator.keys
                    Self.update()
                }
            }
            return event
        }
        return NSView()
    }

    func updateNSView(_ view: NSView, context: Context) {
        context.coordinator.keys = keys
    }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        if let monitor = coordinator.monitor { NSEvent.removeMonitor(monitor) }
        coordinator.monitor = nil
    }

    /// Every menu as opening it makes it.
    static func update() {
        for item in NSApp.mainMenu?.items ?? [] {
            guard let menu = item.submenu else { continue }
            menu.delegate?.menuNeedsUpdate?(menu)
            menu.update()
        }
    }
}

/// Whether the reader's window has the keys, for the Go menu. AppKit hands a key to the menu
/// bar before the window in front, so while another of the app's windows is in front (a
/// trailer, a discussion) the Go menu binds none of the reader's keys and Space or an arrow stays
/// with that window. The reader's own sheets and popovers, and the app in the background, keep
/// the reader in front.
private struct WindowFront: NSViewRepresentable {
    let model: ReaderModel

    @MainActor final class Coordinator {
        weak var view: NSView?
        var model: ReaderModel?
        var observers: [NSObjectProtocol] = []

        /// Asked once the change is done, as NSApp names the new key window only after the
        /// notification of the old one going.
        func follow() {
            DispatchQueue.main.async { [weak self] in
                guard let self, let model, let window = view?.window else { return }
                let front = WindowFront.inFront(key: NSApp.keyWindow, reader: window)
                if model.windowInFront != front { model.windowInFront = front }
            }
        }
    }

    /// The reader's window is in front unless another window of the app has the keys. A window
    /// on the reader's, such as a sheet or a popover, counts as the reader's.
    static func inFront(key: NSWindow?, reader: NSWindow) -> Bool {
        var window = key
        while let current = window {
            if current === reader { return true }
            window = current.sheetParent ?? current.parent
        }
        return key == nil
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    /// Asks again once it is in a window, as the reader may open over a window not in front.
    final class Spot: NSView {
        var moved: () -> Void = {}

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            moved()
        }
    }

    func makeNSView(context: Context) -> Spot {
        let view = Spot()
        let coordinator = context.coordinator
        coordinator.view = view
        coordinator.model = model
        view.moved = { [weak coordinator] in coordinator?.follow() }
        let names: [Notification.Name] = [
            NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification,
            NSApplication.didBecomeActiveNotification, NSApplication.didResignActiveNotification,
        ]
        coordinator.observers = names.map { name in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { _ in
                MainActor.assumeIsolated { coordinator.follow() }
            }
        }
        coordinator.follow()
        return view
    }

    func updateNSView(_ view: Spot, context: Context) {
        context.coordinator.model = model
    }

    static func dismantleNSView(_ view: Spot, coordinator: Coordinator) {
        coordinator.observers.forEach(NotificationCenter.default.removeObserver)
        coordinator.observers = []
        // The next book starts in front, as its window is when it opens.
        coordinator.model?.windowInFront = true
    }
}

/// The window's close, minimise and zoom buttons come and go with the reader's chrome, over a
/// title bar the reader's pages show through. The window has them back as they were once the
/// reader closes. The title bar's transparency is left to the toolbar's: hiding the toolbar makes
/// it clear before this view is in the window, and showing the toolbar again makes it solid, which
/// the toolbar's scroll edge effect needs.
private struct TrafficLights: NSViewRepresentable {
    let visible: Bool

    final class Coordinator {
        weak var window: NSWindow?
        var title: NSWindow.TitleVisibility?
        /// Set once the reader is gone, so a change still queued does not hide the title again.
        var closed = false
    }

    private static let buttons: [NSWindow.ButtonType] = [.closeButton, .miniaturizeButton, .zoomButton]

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView { NSView() }

    func updateNSView(_ view: NSView, context: Context) {
        let coordinator = context.coordinator
        DispatchQueue.main.async {
            guard !coordinator.closed, let window = view.window else { return }
            if coordinator.window == nil {
                coordinator.window = window
                coordinator.title = window.titleVisibility
            }
            window.titleVisibility = .hidden
            for kind in Self.buttons {
                window.standardWindowButton(kind)?.animator().alphaValue = visible ? 1 : Self.gone(kind)
            }
        }
    }

    /// How far a button fades while the chrome is hidden. A minimise button at no opacity at all
    /// turns Window › Minimize and ⌘M off, so it stays at a trace no one sees. The title bar
    /// that holds them is left alone: it holds the toolbar too, which comes back as the book
    /// closes.
    private static func gone(_ kind: NSWindow.ButtonType) -> CGFloat {
        kind == .miniaturizeButton ? 0.01 : 0
    }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        coordinator.closed = true
        guard let window = coordinator.window, let title = coordinator.title else { return }
        window.titleVisibility = title
        for kind in buttons {
            window.standardWindowButton(kind)?.alphaValue = 1
        }
    }
}

#endif

/// The Go menu while a reader is in front. Every key of the reader is an item here.
public struct ReaderMenu: Commands {
    let model: ReaderModel

    public init(model: ReaderModel) {
        self.model = model
    }

    public var body: some Commands {
        CommandMenu(L.readerMenuGo) {
            ReaderMenuItems(model: model)
        }
    }
}

/// What the Go menu shows of the book in front.
struct ReaderInFront: Equatable {
    /// Keys with no modifier go to the reader: no sheet and no Go to page field is up.
    var plain: Bool
    var backPage: Int?
    var keys: ReaderKeyContext

    /// The book open in `model` while its window has the keys, or nil.
    @MainActor static func of(_ model: ReaderModel) -> ReaderInFront? {
        guard model.request != nil, model.windowInFront else { return nil }
        return ReaderInFront(plain: model.sheet == nil && !model.askingPage, backPage: model.backToPage,
                             keys: ReaderActions.keys(model))
    }
}

/// The Go menu's items, on the open reader's model. Dimmed when no reader is in front, and its
/// keys with no modifier are bound only while the reader takes them: AppKit stops at the first
/// menu item with a key, enabled or not, so a bound arrow or Space would take it from a sheet or
/// the Go to page field. The items follow the model itself, which the app holds for every
/// window, so they do not depend on where focus is in the window.
private struct ReaderMenuItems: View {
    let model: ReaderModel

    var body: some View {
        let front = ReaderInFront.of(model)
        let on = front != nil
        let plain = on && front?.plain == true
        let reader = ReaderActions(model: model) { model.close() }
        let keys = front?.keys ?? reader.keys
        // A key with no modifier is the reader's only while no sheet or Go to page is up. The
        // menu binds those keys by `plain`, but iPadOS can keep the commands it built before the
        // prompt came up, so each checks again as it comes.
        let free = { model.sheet == nil && !model.askingPage }
        Group {
            // The same keys as ReaderKeys, which the reader also takes itself.
            if let next = ReaderKeys.arrow(forward: true, in: keys),
               let previous = ReaderKeys.arrow(forward: false, in: keys) {
                Button(L.readerMenuNextPage) { if free() { reader.perform(.page(forward: true)) } }.plainKey(next, when: plain)
                Button(L.readerMenuPreviousPage) { if free() { reader.perform(.page(forward: false)) } }.plainKey(previous, when: plain)
            }
            Button(L.readerMenuNextPage) { if free() { reader.perform(.page(forward: true)) } }.plainKey(.space, when: plain)
            Button(L.readerMenuPreviousPage) { if free() { reader.perform(.page(forward: false)) } }
                .key(.space, .shift, when: plain)
            Button(L.readerMenuNextPage) { if free() { reader.perform(.page(forward: true)) } }.plainKey(.pageDown, when: plain)
            Button(L.readerMenuPreviousPage) { if free() { reader.perform(.page(forward: false)) } }.plainKey(.pageUp, when: plain)
            Button(L.readerMenuNextPage) { if free() { reader.perform(.line(forward: true)) } }.plainKey(.downArrow, when: plain)
            Button(L.readerMenuPreviousPage) { if free() { reader.perform(.line(forward: false)) } }.plainKey(.upArrow, when: plain)
            Button(L.readerMenuFirstPage) { if free() { reader.perform(.edge(first: true)) } }.plainKey(.home, when: plain)
            Button(L.readerMenuLastPage) { if free() { reader.perform(.edge(first: false)) } }.plainKey(.end, when: plain)
            Divider()
            Button(L.readerMenuNextChapter) { if free() { reader.perform(.chapter(next: true)) } }.plainKey("]", when: plain)
            Button(L.readerMenuPreviousChapter) { if free() { reader.perform(.chapter(next: false)) } }.plainKey("[", when: plain)
            Button(L.readerMenuGoToPage) { if free() { reader.askPage() } }.plainKey("g", when: plain)
            if let page = front?.backPage {
                Button(L.readerMenuBackToPage(page: page)) { reader.back() }.key("[", .command, when: on)
            } else {
                Button(L.readerMenuBack) { reader.back() }.key("[", .command, when: on).disabled(true)
            }
            Divider()
            Button(L.readerMenuChapters) { reader.showChapters() }.key("l", .command, when: on)
            Button(L.readerMenuSettings) { reader.showSettings() }.key("t", .command, when: on)
            Divider()
            Button(L.readerMenuZoomIn) { reader.zoom(1) }.key("+", .command, when: on)
            Button(L.readerMenuZoomOut) { reader.zoom(-1) }.key("-", .command, when: on)
            Button(L.readerMenuZoomFit) { reader.zoom(0) }.key("0", .command, when: on)
            #if os(iOS)
            // The Mac's reader takes Esc itself; on iPad it is an item of its own.
            Divider()
            Button(L.readerClose) { reader.escape() }.plainKey(.escape, when: on)
            #endif
        }
        .disabled(!on)
    }
}

private extension View {
    /// A key, bound only while `on`.
    func key(_ key: KeyEquivalent, _ modifiers: EventModifiers, when on: Bool) -> some View {
        keyboardShortcut(on ? KeyboardShortcut(key, modifiers: modifiers) : nil)
    }

    /// A key with no modifier, bound only while `on`.
    func plainKey(_ key: KeyEquivalent, when on: Bool) -> some View {
        self.key(key, [], when: on)
    }
}
