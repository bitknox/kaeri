// Where the reader lives. On iPhone and iPad it covers the app; on the Mac it is a window of its
// own, one at a time, with the Go menu's keys. The dev harness draws it over its window.

import KaeriStore
import KaeriStrings
import SwiftUI

extension View {
    /// Show the reader when the store has one open.
    public func hostsReader(_ model: ReaderModel, inline: Bool = false) -> some View {
        modifier(ReaderHost(model: model, inline: inline))
    }
}

private struct ReaderHost: ViewModifier {
    @Bindable var model: ReaderModel
    let inline: Bool

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
        if inline {
            // The real reader is a window with a hidden title bar; the harness has one window, so
            // the open reader takes the toolbar and title away, as that window has none.
            content
                .toolbarVisibility(model.request != nil ? .hidden : .automatic, for: .windowToolbar)
                .overlay {
                    if model.request != nil {
                        ReaderScreen(model: model) {}
                            .ignoresSafeArea()
                            .background(TrafficLights(visible: model.chromeShown))
                    }
                }
        } else {
            content.modifier(ReaderWindowOpener(model: model))
        }
        #endif
    }
}

#if os(macOS)
import AppKit

public enum ReaderWindow {
    public static let id = "reader"
}

private struct ReaderWindowOpener: ViewModifier {
    let model: ReaderModel
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow

    func body(content: Content) -> some View {
        content.onChange(of: model.request != nil) { _, open in
            if open { openWindow(id: ReaderWindow.id) } else { dismissWindow(id: ReaderWindow.id) }
        }
    }
}

/// The content of the reader's window.
public struct ReaderWindowView: View {
    let model: ReaderModel
    @Environment(\.dismissWindow) private var dismissWindow

    public init(model: ReaderModel) { self.model = model }

    public var body: some View {
        ReaderScreen(model: model) { dismissWindow(id: ReaderWindow.id) }
            .frame(minWidth: 480, minHeight: 600)
            .ignoresSafeArea()
            .background(TrafficLights(visible: model.chromeShown))
            .onDisappear { model.close() }
    }
}

/// The window's close, minimise and zoom buttons come and go with the chrome.
private struct TrafficLights: NSViewRepresentable {
    let visible: Bool

    func makeNSView(context: Context) -> NSView { NSView() }

    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async {
            guard let window = view.window else { return }
            window.titlebarAppearsTransparent = true
            window.titleVisibility = .hidden
            for kind in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
                window.standardWindowButton(kind)?.animator().alphaValue = visible ? 1 : 0
            }
        }
    }
}

/// The Go menu while a reader is in front. Every key of the reader is an item here.
public struct ReaderMenu: Commands {
    @FocusedValue(\.readerActions) private var reader

    public init() {}

    public var body: some Commands {
        CommandMenu(L.readerMenuGo) {
            if let reader {
                let next: KeyEquivalent = reader.rtl ? .leftArrow : .rightArrow
                let previous: KeyEquivalent = reader.rtl ? .rightArrow : .leftArrow
                Button(L.readerMenuNextPage) { reader.turn(true, false) }.keyboardShortcut(next, modifiers: [])
                Button(L.readerMenuPreviousPage) { reader.turn(false, false) }.keyboardShortcut(previous, modifiers: [])
                Button(L.readerMenuNextPage) { reader.turn(true, false) }.keyboardShortcut(.space, modifiers: [])
                Button(L.readerMenuPreviousPage) { reader.turn(false, false) }.keyboardShortcut(.space, modifiers: .shift)
                Button(L.readerMenuNextPage) { reader.turn(true, true) }.keyboardShortcut(.downArrow, modifiers: [])
                Button(L.readerMenuPreviousPage) { reader.turn(false, true) }.keyboardShortcut(.upArrow, modifiers: [])
                Divider()
                Button(L.readerMenuNextChapter) { reader.chapter(true) }.keyboardShortcut("]", modifiers: [])
                Button(L.readerMenuPreviousChapter) { reader.chapter(false) }.keyboardShortcut("[", modifiers: [])
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
            } else {
                Button(L.readerMenuNextPage) {}.disabled(true)
            }
        }
    }
}
#endif
