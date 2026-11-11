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
/// reader closes.
private struct TrafficLights: NSViewRepresentable {
    let visible: Bool

    final class Coordinator {
        weak var window: NSWindow?
        var before: (transparent: Bool, title: NSWindow.TitleVisibility)?
    }

    private static let buttons: [NSWindow.ButtonType] = [.closeButton, .miniaturizeButton, .zoomButton]

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView { NSView() }

    func updateNSView(_ view: NSView, context: Context) {
        let coordinator = context.coordinator
        DispatchQueue.main.async {
            guard let window = view.window else { return }
            if coordinator.window == nil {
                coordinator.window = window
                coordinator.before = (window.titlebarAppearsTransparent, window.titleVisibility)
            }
            window.titlebarAppearsTransparent = true
            window.titleVisibility = .hidden
            for kind in Self.buttons {
                window.standardWindowButton(kind)?.animator().alphaValue = visible ? 1 : 0
            }
        }
    }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        guard let window = coordinator.window, let before = coordinator.before else { return }
        window.titlebarAppearsTransparent = before.transparent
        window.titleVisibility = before.title
        for kind in buttons {
            window.standardWindowButton(kind)?.alphaValue = 1
        }
    }
}

#endif

/// The Go menu while a reader is in front. Every key of the reader is an item here.
public struct ReaderMenu: Commands {
    @FocusedValue(\.readerActions) private var reader

    public init() {}

    public var body: some Commands {
        CommandMenu(L.readerMenuGo) {
            if let reader {
                // The same keys as ReaderKeys, which the reader also takes itself.
                if let next = ReaderKeys.arrow(forward: true, in: reader.keys),
                   let previous = ReaderKeys.arrow(forward: false, in: reader.keys) {
                    Button(L.readerMenuNextPage) { reader.perform(.page(forward: true)) }.keyboardShortcut(next, modifiers: [])
                    Button(L.readerMenuPreviousPage) { reader.perform(.page(forward: false)) }.keyboardShortcut(previous, modifiers: [])
                }
                Button(L.readerMenuNextPage) { reader.perform(.page(forward: true)) }.keyboardShortcut(.space, modifiers: [])
                Button(L.readerMenuPreviousPage) { reader.perform(.page(forward: false)) }.keyboardShortcut(.space, modifiers: .shift)
                Button(L.readerMenuNextPage) { reader.perform(.page(forward: true)) }.keyboardShortcut(.pageDown, modifiers: [])
                Button(L.readerMenuPreviousPage) { reader.perform(.page(forward: false)) }.keyboardShortcut(.pageUp, modifiers: [])
                Button(L.readerMenuNextPage) { reader.perform(.line(forward: true)) }.keyboardShortcut(.downArrow, modifiers: [])
                Button(L.readerMenuPreviousPage) { reader.perform(.line(forward: false)) }.keyboardShortcut(.upArrow, modifiers: [])
                Button(L.readerMenuFirstPage) { reader.perform(.edge(first: true)) }.keyboardShortcut(.home, modifiers: [])
                Button(L.readerMenuLastPage) { reader.perform(.edge(first: false)) }.keyboardShortcut(.end, modifiers: [])
                Divider()
                Button(L.readerMenuNextChapter) { reader.perform(.chapter(next: true)) }.keyboardShortcut("]", modifiers: [])
                Button(L.readerMenuPreviousChapter) { reader.perform(.chapter(next: false)) }.keyboardShortcut("[", modifiers: [])
                Button(L.readerMenuGoToPage) { reader.askPage() }.keyboardShortcut("g", modifiers: [])
                if let page = reader.backPage {
                    Button(L.readerMenuBackToPage(page: page)) { reader.back() }.keyboardShortcut("[", modifiers: .command)
                } else {
                    Button(L.readerMenuBack) { reader.back() }.keyboardShortcut("[", modifiers: .command).disabled(true)
                }
                Divider()
                Button(L.readerMenuChapters) { reader.showChapters() }.keyboardShortcut("l", modifiers: .command)
                Button(L.readerMenuSettings) { reader.showSettings() }.keyboardShortcut("t", modifiers: .command)
                Divider()
                Button(L.readerMenuZoomIn) { reader.zoom(1) }.keyboardShortcut("+", modifiers: .command)
                Button(L.readerMenuZoomOut) { reader.zoom(-1) }.keyboardShortcut("-", modifiers: .command)
                Button(L.readerMenuZoomFit) { reader.zoom(0) }.keyboardShortcut("0", modifiers: .command)
                #if os(iOS)
                // The Mac's window takes Esc itself; on iPad it is an item of its own.
                Divider()
                Button(L.readerClose) { reader.escape() }.keyboardShortcut(.escape, modifiers: [])
                #endif
            } else {
                Button(L.readerMenuNextPage) {}.disabled(true)
            }
        }
    }
}
