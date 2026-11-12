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

    func body(content: Content) -> some View {
        // Set here, in a body that follows the model: the app's scene closure does not, so a
        // value set there kept the book it saw first.
        hosted(content).focusedSceneValue(\.readerInFront, ReaderInFront.of(model))
    }

    @ViewBuilder private func hosted(_ content: Content) -> some View {
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
                        .transition(.opacity)
                }
            }
            .animation(reduceMotion ? nil : .standard, value: model.request != nil)
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

extension FocusedValues {
    /// The book open in a window, set on the window's scene by `hostsReader`, so the Go menu has it
    /// only while that window is in front, and draws again as it changes.
    @Entry public var readerInFront: ReaderInFront?
}

/// What the Go menu shows of the book in front.
public struct ReaderInFront: Equatable {
    /// Keys with no modifier go to the reader: no sheet and no Go to page field is up.
    var plain: Bool
    var backPage: Int?
    var keys: ReaderKeyContext

    /// The book open in `model`, or nil when none is.
    @MainActor public static func of(_ model: ReaderModel) -> ReaderInFront? {
        guard model.request != nil else { return nil }
        return ReaderInFront(plain: model.sheet == nil && !model.askingPage, backPage: model.backToPage,
                             keys: ReaderActions.keys(model))
    }
}

/// The Go menu's items, on the open reader's model. Dimmed when no reader is in front, and its
/// keys with no modifier are bound only while the reader takes them: AppKit stops at the first
/// menu item with a key, enabled or not, so a bound arrow or Space would take it from a sheet or
/// the Go to page field. The menu draws again with the focused value: the commands do not follow
/// the model's own changes.
private struct ReaderMenuItems: View {
    let model: ReaderModel
    @FocusedValue(\.readerInFront) private var front

    var body: some View {
        let on = front != nil && model.request != nil
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
