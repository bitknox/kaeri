// The reader's screen: the pages, the chrome over them, the notices and the sheets. One view for
// iPhone, iPad and Mac; the Mac differences are the pointer-driven chrome and the keys.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

public struct ReaderScreen: View {
    @Bindable var model: ReaderModel
    let onClose: () -> Void
    @Environment(\.displayScale) private var displayScale
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.accessibilityVoiceOverEnabled) private var voiceOver
    @State private var size = CGSize(width: 390, height: 844)
    @State private var hideTask: Task<Void, Never>?
    @AppStorage(ReaderDefaults.background) private var background = "theme"
    @AppStorage(ReaderDefaults.system) private var system = false
    @Environment(\.colorScheme) private var colorScheme

    public init(model: ReaderModel, onClose: @escaping () -> Void) {
        self.model = model
        self.onClose = onClose
    }

    var mac: Bool {
        #if os(macOS)
        true
        #else
        false
        #endif
    }

    var look: String { model.choice(.look) ?? "dark" }
    var theme: ReaderTheme { .forLook(system ? (colorScheme == .dark ? "dark" : "light") : look) }
    var paper: Bool { model.isOn(.paperMode) || model.focusPaper }
    var tone: PageTone { PageTone(dim: paper, grey: model.isOn(.greyscalePages)) }
    var rtl: Bool { model.view?.direction == .rightToLeft }

    public var body: some View {
        // The whole screen in every state: the pages and the chrome are laid out in a box that
        // ignores the safe area, so the status bar coming and going moves nothing.
        GeometryReader { proxy in
            ZStack {
                (model.medium == .manga ? theme.backdrop(background) : theme.page)
                content
                ReaderChrome(model: model, theme: theme, mac: mac, size: size, onClose: close)
                ReaderNotices(model: model, theme: theme, mac: mac)
            }
            .frame(width: proxy.size.width, height: proxy.size.height)
            .onAppear { resize(proxy.size) }
            .onChange(of: proxy.size) { _, new in resize(new) }
        }
        .ignoresSafeArea()
        .onChange(of: voiceOver, initial: true) { _, on in model.pinChrome = on }
        // The book's decoded pages go with the reader: a page is drawn again from its file.
        .onDisappear { PageImages.removeAll() }
        .preferredColorScheme(system ? nil : theme.scheme)
        .environment(\.readerPaper, paper)
        .environment(\.readerClose, close)
        .animation(paper ? nil : (model.chromeShown ? .fadeIn : .fadeOut), value: model.chromeShown)
        #if os(macOS)
        .onContinuousHover { phase in
            if case .active = phase { pointerMoved() }
        }
        .onExitCommand { escape() }
        #endif
        .onChange(of: model.sheet) { _, sheet in if sheet != nil { model.chromeShown = true } }
        .focusedSceneValue(\.readerActions, actions)
        #if os(iOS)
        .statusBarHidden(!model.chromeShown)
        .persistentSystemOverlays(model.chromeShown ? .automatic : .hidden)
        #endif
    }

    @ViewBuilder private var content: some View {
        switch model.phase {
        case .closed, .opening:
            ReaderOpening(model: model, theme: theme)
        case .failed(let failure):
            ReaderChapterFailed(model: model, theme: theme, failure: failure, close: close)
        case .open:
            if model.novel != nil {
                NovelReader(model: model, theme: theme, mac: mac, size: size, rtl: rtl) { tap($0) }
            } else if model.view?.mode == .strip {
                StripReader(model: model, theme: theme, mac: mac, size: size, tone: tone) { tap(0.5) }
            } else {
                PagedReader(model: model, theme: theme, size: size, scale: displayScale, tone: tone, paper: paper) { tap($0) }
            }
        }
    }

    // MARK: Size

    private func resize(_ new: CGSize) {
        guard new.width > 0, new.height > 0 else { return }
        size = new
        let surface = ReaderSurface(
            widthPx: UInt32(new.width * displayScale), heightPx: UInt32(new.height * displayScale),
            scale: Float(displayScale), eink: false, twoPages: new.width > new.height && new.width >= 700,
            dark: !theme.isLight)
        model.surfaceChanged(surface)
    }

    // MARK: Chrome

    /// A tap at the horizontal fraction `x`. The middle third shows the chrome; the next tap hides it.
    private func tap(_ x: Double) {
        if model.sheet != nil { return }
        if model.chromeShown { model.hideChrome() } else if x > 0.33 && x < 0.67 { model.chromeShown = true }
    }

    #if os(macOS)
    private func pointerMoved() {
        model.chromeShown = true
        hideTask?.cancel()
        hideTask = Task {
            try? await Task.sleep(for: .seconds(2))
            guard !Task.isCancelled, model.sheet == nil, !model.pinChrome else { return }
            model.chromeShown = false
            NSCursor.setHiddenUntilMouseMoves(true)
        }
    }

    /// Esc steps out: a sheet first, then the chrome, then full screen, then the reader.
    private func escape() {
        if model.sheet != nil { model.sheet = nil; return }
        if model.chromeShown { model.chromeShown = false; return }
        if let window = NSApp.keyWindow, window.styleMask.contains(.fullScreen) { window.toggleFullScreen(nil); return }
        close()
    }
    #else
    private func pointerMoved() {}
    private func escape() { close() }
    #endif

    private func close() {
        model.close()
        onClose()
    }

    var actions: ReaderActions {
        ReaderActions(
            rtl: rtl, canGoBack: model.backTo != nil, backPage: model.backTo == nil ? nil : model.ring.map { model.pageNumber(of: $0) },
            turn: { forward, line in model.requestTurn(forward: forward, line: line) },
            chapter: { next in
                if let row = next ? model.view?.next : model.view?.previous { model.openChapter(row.id) }
            },
            back: { model.act(.backTo) },
            askPage: { model.chromeShown = true; model.askingPage = true },
            showChapters: { model.sheet = .chapters },
            showSettings: { model.sheet = .settings },
            zoom: { step in zoom(step) })
    }

    private func zoom(_ step: Int) {
        if model.medium == .novel {
            let now = model.amount(.textSize) ?? 1
            let next = step == 0 ? 1 : min(3, max(0.5, now + Double(step) * 0.1))
            model.set(.textSize, .amount(value: next))
        } else if model.view?.mode == .strip {
            let now = model.amount(.stripWidth) ?? 0.6
            let next = step == 0 ? 0.6 : min(1, max(0.3, now + Double(step) * 0.1))
            model.set(.stripWidth, .amount(value: next))
        }
    }
}

// MARK: Environment and menu actions

private struct ReaderCloseKey: EnvironmentKey { static let defaultValue: @MainActor @Sendable () -> Void = {} }

extension EnvironmentValues {
    var readerClose: @MainActor @Sendable () -> Void {
        get { self[ReaderCloseKey.self] }
        set { self[ReaderCloseKey.self] = newValue }
    }
}

private struct ReaderPaperKey: EnvironmentKey { static let defaultValue = false }

extension EnvironmentValues {
    var readerPaper: Bool {
        get { self[ReaderPaperKey.self] }
        set { self[ReaderPaperKey.self] = newValue }
    }
}

/// What the Go menu can ask of the reader that is open.
public struct ReaderActions {
    public var rtl: Bool
    public var canGoBack: Bool
    public var backPage: Int?
    public var turn: (_ forward: Bool, _ line: Bool) -> Void
    public var chapter: (_ next: Bool) -> Void
    public var back: () -> Void
    public var askPage: () -> Void
    public var showChapters: () -> Void
    public var showSettings: () -> Void
    public var zoom: (_ step: Int) -> Void
}

public struct ReaderActionsKey: FocusedValueKey {
    public typealias Value = ReaderActions
}

extension FocusedValues {
    public var readerActions: ReaderActions? {
        get { self[ReaderActionsKey.self] }
        set { self[ReaderActionsKey.self] = newValue }
    }
}

// MARK: Opening and failing

struct ReaderOpening: View {
    let model: ReaderModel
    let theme: ReaderTheme
    @State private var seconds = 0
    @State private var shown = false

    var body: some View {
        let chapter = model.view?.chapter.label ?? ""
        VStack(spacing: 14) {
            if shown {
                ProgressView().controlSize(.regular).tint(theme.haze)
                Text(seconds >= 4 ? L.readerOpeningSlow(seconds: seconds) : L.readerOpening(chapter: chapter))
                    .font(.ui(15)).foregroundStyle(theme.haze).monospacedDigit()
                    .contentTransition(.opacity)
            }
        }
        .transition(.opacity)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .task {
            // Nothing for 600 ms, then the spinner and its line; the slow line comes at 4 s.
            seconds = 0
            try? await Task.sleep(for: .milliseconds(600))
            withAnimation(.fadeIn) { shown = true }
            seconds = 1
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                seconds += 1
            }
        }
    }
}

struct ReaderChapterFailed: View {
    let model: ReaderModel
    let theme: ReaderTheme
    let failure: ReadingFailure
    let close: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Image(systemName: "exclamationmark.triangle").font(.system(size: 24)).foregroundStyle(theme.vermilion)
            Text(L.readerChapterFailed(chapter: model.view?.chapter.label ?? "")).font(.ui(20, weight: .semibold))
                .foregroundStyle(theme.silk)
            Text(failure.words).font(.ui(15)).foregroundStyle(theme.haze)
            HStack(spacing: 10) {
                Button(L.readerTryAgain) { model.retry(page: nil) }.buttonStyle(ReaderAmberStyle(theme: theme, height: 44).fixed())
                Button(L.readerChooseSource, action: close).buttonStyle(ReaderPillStyle(theme: theme, height: 44))
            }
            .padding(.top, 6)
            Button(L.readerClose, action: close).buttonStyle(.plain)
                .font(.ui(15, weight: .semibold)).foregroundStyle(theme.haze).frame(minHeight: 44)
        }
        .frame(maxWidth: 360, alignment: .leading)
        .padding(28)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

extension ReaderAmberStyle {
    /// The same button at its own width.
    func fixed() -> ReaderFixedAmber { ReaderFixedAmber(base: self) }
}

struct ReaderFixedAmber: ButtonStyle {
    let base: ReaderAmberStyle

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.ui(15, weight: .semibold))
            .foregroundStyle(base.theme.onAmber)
            .padding(.horizontal, 18)
            .frame(minHeight: base.height)
            .background(base.theme.amber.opacity(configuration.isPressed ? 0.85 : 1), in: .capsule)
            .contentShape(.capsule)
    }
}
