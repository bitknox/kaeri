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
    @Environment(\.scenePhase) private var scenePhase
    @Environment(Navigator.self) private var navigator: Navigator?
    @State private var spoken: Task<Void, Never>?
    @State private var groupHeight: CGFloat = 84
    /// The novel's web view, kept from chapter to chapter while the reader is open.
    @State private var novelWeb = NovelWebSession()
    /// The held end page's opacity: 1 while it waits, faded to 0 once the next page has drawn.
    @State private var heldShown = 1.0
    /// The reader holds the keyboard from the moment it shows, so its keys need no click first.
    @FocusState private var holdsKeys: Bool

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
                    .environment(\.readerHeld, model.held != nil)
                heldEnd
                ReaderChrome(model: model, theme: theme, mac: mac, size: size, onClose: close, groupHeight: $groupHeight)
                ReaderNotices(model: model, theme: theme, mac: mac, size: size, groupHeight: groupHeight)
            }
            .frame(width: proxy.size.width, height: proxy.size.height)
            .onAppear { resize(proxy.size) }
            .onChange(of: proxy.size) { _, new in resize(new) }
        }
        .ignoresSafeArea()
        // The reader closed: its web view and everything the page held go with it, as soon as
        // the book closes, whether or not the window it was in has gone yet.
        .onDisappear { novelWeb.close() }
        .onChange(of: model.request == nil) { _, closed in if closed { novelWeb.close() } }
        .onChange(of: voiceOver, initial: true) { _, on in model.pinChrome = on }
        // The book's decoded pages go with the reader: a page is drawn again from its file.
        .onDisappear { PageImages.removeAll() }
        // Notices wait while the reader is out of sight.
        .onChange(of: scenePhase, initial: true) { _, phase in model.away = phase != .active }
        // A settled turn is said, as the chrome's line says it.
        .onChange(of: spokenPlace) { _, _ in announcePlace() }
        .preferredColorScheme(system ? nil : theme.scheme)
        .environment(\.readerPaper, paper)
        .environment(\.readerClose, close)
        .animation(paper ? nil : (model.chromeShown ? .fadeIn : .fadeOut), value: model.chromeShown)
        #if os(macOS)
        .onContinuousHover { phase in
            if case .active = phase { pointerMoved() }
        }
        // The Go menu carries the keys; the reader also takes them itself, for a key the menu
        // has not bound. Esc is the reader's own, on the view that holds the keys.
        .focusable()
        .focusEffectDisabled()
        .focused($holdsKeys)
        .onExitCommand { escape() }
        .onKeyPress(phases: [.down, .repeat]) { press in
            take(press.key, press.modifiers, repeated: press.phase == .repeat) ? .handled : .ignored
        }
        .onAppear { holdsKeys = true }
        .onChange(of: model.sheet == nil) { _, closed in if closed { holdsKeys = true } }
        .onChange(of: model.askingPage) { _, asking in if !asking { holdsKeys = true } }
        .onChange(of: scenePhase) { _, phase in if phase == .active { holdsKeys = true } }
        #else
        // On iPad the reader holds the keyboard in a view of its own while nothing is over it.
        .background {
            ReaderKeyCatcher(holds: model.sheet == nil && !model.askingPage && scenePhase == .active) { key, modifiers in
                take(key, modifiers, repeated: false)
            }
        }
        #endif
        .onChange(of: model.sheet) { _, sheet in if sheet != nil { model.chromeShown = true } }
        .onChange(of: model.held?.tick) { _, _ in heldShown = 1 }
        .onChange(of: model.held?.landed == true) { _, landed in if landed { letHeldGo() } }
        #if os(iOS)
        .statusBarHidden(!model.chromeShown)
        .persistentSystemOverlays(model.chromeShown ? .automatic : .hidden)
        #endif
    }

    @ViewBuilder private var content: some View {
        switch model.phase {
        case .closed, .opening:
            ReaderOpening(model: model, theme: theme, size: size, chooseSource: chooseSource, close: close)
        case .failed(let failure):
            ReaderChapterFailed(model: model, theme: theme, failure: failure, chooseSource: chooseSource, close: close)
        case .open:
            if model.novel != nil {
                // The web view keeps the tap for as long as the reader is open, so it holds the
                // model alone, never this screen.
                NovelReader(model: model, theme: theme, mac: mac, size: size, rtl: rtl, web: novelWeb) { [model] x in
                    Self.tap(model, x)
                }
            } else if model.view?.mode == .strip {
                StripReader(model: model, theme: theme, mac: mac, size: size, tone: tone) { tap(0.5) }
            } else {
                PagedReader(model: model, theme: theme, size: size, scale: displayScale, tone: tone, paper: paper) { tap($0) }
            }
        }
    }

    // MARK: The held end page

    /// Leaving a chapter from its end page: that page stays over the next chapter, taking the
    /// touches, until the next chapter's first page has drawn under it.
    @ViewBuilder private var heldEnd: some View {
        if let held = model.held {
            ZStack(alignment: .bottom) {
                ReaderEndPage(model: model, theme: theme, size: size, held: held)
                    .allowsHitTesting(false)
                // A chapter picked from the list has no Continue on this page to wait in: the
                // cue sits at its foot instead.
                if held.slow, held.to != (held.end?.next ?? held.view.next)?.id {
                    ProgressView().controlSize(.small).tint(theme.haze)
                        .padding(.bottom, 48)
                        .transition(.opacity)
                }
            }
            .animation(.fadeIn, value: held.slow)
            .frame(width: size.width, height: size.height)
            .contentShape(.rect)
            .onTapGesture {}
            .opacity(heldShown)
            .accessibilityHidden(true)
        }
    }

    /// The first page has drawn under the held end page: one crossfade to it, with the same
    /// spring as a page turn. Paper mode and Reduce Motion swap at once; the page under it is
    /// already drawn, so no frame is empty either way.
    private func letHeldGo() {
        guard let tick = model.held?.tick else { return }
        if paper || reduceMotion {
            model.releaseHeld(tick: tick)
            return
        }
        withAnimation(.quick, completionCriteria: .logicallyComplete) {
            heldShown = 0
        } completion: {
            var transaction = Transaction()
            transaction.disablesAnimations = true
            withTransaction(transaction) {
                model.releaseHeld(tick: tick)
                heldShown = 1
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
    private func tap(_ x: Double) { Self.tap(model, x) }

    private static func tap(_ model: ReaderModel, _ x: Double) {
        if model.sheet != nil { return }
        if model.chromeShown { model.hideChrome() } else if x > 0.33 && x < 0.67 { model.chromeShown = true }
    }

    #if os(macOS)
    private func pointerMoved() {
        model.chromeShown = true
        hideTask?.cancel()
        hideTask = Task {
            try? await Task.sleep(for: .seconds(2))
            // A shot holding the scrubber keeps the group up under a still pointer.
            guard !Task.isCancelled, model.sheet == nil, !model.pinChrome, model.shotScrub == nil else { return }
            model.chromeShown = false
            NSCursor.setHiddenUntilMouseMoves(true)
        }
    }

    #else
    private func pointerMoved() {}
    #endif

    private func escape() { ReaderActions.escape(model, close: close) }

    /// A key for the reader: Esc steps out, and the reader's table turns while nothing is over
    /// the pages. True when the reader took it.
    private func take(_ key: KeyEquivalent, _ modifiers: EventModifiers, repeated: Bool) -> Bool {
        if key == .escape, !repeated, modifiers.isEmpty {
            escape()
            return true
        }
        guard model.sheet == nil, !model.askingPage,
              let action = ReaderKeys.action(key, modifiers: modifiers, in: keyContext)
        else { return false }
        actions.perform(action)
        return true
    }

    /// The place as VoiceOver hears it, once a turn has settled.
    private var spokenPlace: String {
        guard voiceOver, model.phase == .open else { return "" }
        // A credit page shown or left is a place of its own: "Chapter 87, credit page".
        return "\(model.view?.chapter.id ?? "")|\(model.inTrail?.id ?? "")|\(model.position.index)|\(model.position.count)|\(model.onCredit.map(String.init) ?? "")"
    }

    private func announcePlace() {
        spoken?.cancel()
        guard voiceOver, model.phase == .open else { return }
        spoken = Task {
            try? await Task.sleep(for: .milliseconds(450))
            guard !Task.isCancelled else { return }
            let line = ReaderChrome.line(model: model, index: model.position.index)
            AccessibilityNotification.PageScrolled(line.replacingOccurrences(of: " · ", with: ", ")).post() // i18n-skip: a separator
        }
    }

    /// Choose source: the reader goes and the series' version and source picker comes up over
    /// its page.
    private func chooseSource() {
        guard let work = model.request?.work ?? model.view?.work else { close(); return }
        close()
        #if os(iOS)
        // The picker is a sheet of the page under the reader: it comes once the cover has gone.
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(450))
            navigator?.chooseSource(for: work)
        }
        #else
        navigator?.chooseSource(for: work)
        #endif
    }

    private func close() {
        model.close()
        onClose()
    }

    var actions: ReaderActions { ReaderActions(model: model, close: close) }

    var keyContext: ReaderKeyContext { ReaderActions.keys(model) }
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

private struct ReaderHeldKey: EnvironmentKey { static let defaultValue = false }

extension EnvironmentValues {
    /// The end page is held over these pages: they draw at once under it, with no fade of their own.
    var readerHeld: Bool {
        get { self[ReaderHeldKey.self] }
        set { self[ReaderHeldKey.self] = newValue }
    }
}

extension EnvironmentValues {
    var readerPaper: Bool {
        get { self[ReaderPaperKey.self] }
        set { self[ReaderPaperKey.self] = newValue }
    }
}

/// What the Go menu can ask of the reader that is open.
public struct ReaderActions {
    public var rtl: Bool
    /// How the reader reads, for its keys.
    var keys: ReaderKeyContext
    public var canGoBack: Bool
    public var backPage: Int?
    public var turn: (_ forward: Bool, _ line: Bool) -> Void
    public var chapter: (_ next: Bool) -> Void
    public var back: () -> Void
    public var askPage: () -> Void
    /// Home and End.
    public var edge: (_ first: Bool) -> Void
    /// Esc on iPad: out of a sheet, the chrome, then the reader.
    public var escape: () -> Void
    public var showChapters: () -> Void
    public var showSettings: () -> Void
    public var zoom: (_ step: Int) -> Void
}

extension ReaderActions {
    /// The open reader's actions, for its keys and the Go menu.
    @MainActor init(model: ReaderModel, close: @escaping () -> Void) {
        self.init(
            rtl: model.view?.direction == .rightToLeft, keys: Self.keys(model),
            canGoBack: model.backToPage != nil, backPage: model.backToPage,
            turn: { forward, line in model.requestTurn(forward: forward, line: line) },
            chapter: { next in
                if let row = next ? model.view?.next : model.view?.previous { model.openChapter(row.id) }
            },
            back: { model.back() },
            askPage: { model.chromeShown = true; model.askingPage = true },
            edge: { first in Self.edge(model, first: first) },
            escape: { Self.escape(model, close: close) },
            showChapters: { model.sheet = .chapters },
            showSettings: { model.sheet = .settings },
            zoom: { step in Self.zoom(model, step) })
    }

    @MainActor static func keys(_ model: ReaderModel) -> ReaderKeyContext {
        ReaderKeyContext(novel: model.novel != nil, mode: model.view?.mode, direction: model.view?.direction)
    }

    #if os(macOS)
    /// Esc steps out: a sheet first, then Go to page, then the reader, back to where the book
    /// was opened. Full screen is the window's, and stays.
    @MainActor static func escape(_ model: ReaderModel, close: () -> Void) {
        if model.sheet != nil { model.sheet = nil; return }
        if model.askingPage { model.askingPage = false; return }
        close()
    }
    #else
    /// Esc steps out: a sheet first, then Go to page, then the chrome, then the reader.
    @MainActor static func escape(_ model: ReaderModel, close: () -> Void) {
        if model.sheet != nil { model.sheet = nil; return }
        if model.askingPage { model.askingPage = false; return }
        if model.chromeShown { model.chromeShown = false; return }
        close()
    }
    #endif

    /// Home and End: the first page or image, or the last one before the end page.
    @MainActor private static func edge(_ model: ReaderModel, first: Bool) {
        model.hideChrome()
        if model.novel != nil || model.view?.mode == .strip {
            model.requestJump(first ? 0 : max(model.position.count - 1, 0))
            return
        }
        let last = model.leaves.lastIndex { !$0.endPage } ?? 0
        model.requestJump(first ? 0 : last)
    }

    @MainActor static func zoom(_ model: ReaderModel, _ step: Int) {
        if model.medium == .novel {
            let now = model.amount(.textSize) ?? 1
            let next = step == 0 ? 1 : min(3, max(0.5, now + Double(step) * 0.1))
            model.set(.textSize, .amount(value: next))
        } else if model.view?.mode == .strip {
            let now = model.amount(.stripWidth) ?? 0.6
            let next = step == 0 ? 0.6 : min(1, max(0.3, now + Double(step) * 0.1))
            model.set(.stripWidth, .amount(value: next))
            // Fit undoes a pinch's zoom too, whatever cut the pinch off.
            if step == 0 { model.fitStrip() }
        }
    }

    /// Carry out what a key or its menu item asks.
    func perform(_ key: ReaderKey) {
        switch key {
        case .page(let forward): turn(forward, false)
        case .line(let forward): turn(forward, true)
        case .edge(let first): edge(first)
        case .chapter(let next): chapter(next)
        case .zoom(let step): zoom(step)
        }
    }
}

// MARK: Opening and failing

struct ReaderOpening: View {
    let model: ReaderModel
    let theme: ReaderTheme
    let size: CGSize
    let chooseSource: () -> Void
    let close: () -> Void
    @State private var seconds = 0
    @State private var shown = false

    /// How long a stage goes on before the screen offers another way: the player's own wait
    /// before it offers another source.
    static let longWait = 15

    /// The page's place as the opening holds it: the column of a strip, a page fitted to the
    /// screen otherwise. The first page fades in on the same plate.
    private var plate: CGSize {
        if model.view?.mode == .strip { return CGSize(width: min(size.width, 720), height: size.height) }
        let width = min(size.width, size.height * 0.7)
        return CGSize(width: width, height: min(size.height, width / 0.7))
    }

    /// What the open waits on, in words: the stage the engine named, else the chapter.
    static func stage(_ wait: ReaderWait?, chapter label: String) -> String {
        switch wait {
        case .chapterList(let source)?: L.readerWaitChapterList(source: source)
        case .chapterPages(let chapter, let source)?: L.readerWaitChapterPages(chapter: chapter, source: source)
        case .findingSource?, nil: L.readerOpening(chapter: label)
        }
    }

    var body: some View {
        let chapter = model.view?.chapter
        let label = chapter.map { $0.number ?? $0.label } ?? ""
        let stage = Self.stage(model.openingWait, chapter: label)
        ZStack {
            if model.medium != .novel {
                Rectangle().fill(theme.raised).frame(width: plate.width, height: plate.height)
            }
            VStack(spacing: 6) {
                if shown {
                    Text(seconds >= 3 ? L.readerWaitSeconds(line: stage, seconds: seconds) : stage)
                        .font(.ui(15)).foregroundStyle(theme.haze).monospacedDigit()
                        .multilineTextAlignment(.center)
                        .contentTransition(.opacity)
                        .transition(.opacity)
                }
                if seconds >= Self.longWait {
                    // A stage that goes on offers the way on, as a failed open does.
                    VStack(spacing: 12) {
                        Text(L.readerWaitLong).font(.ui(13)).foregroundStyle(theme.haze)
                        HStack(spacing: 10) {
                            Button(L.seriesReadFromAnother, action: chooseSource)
                                .buttonStyle(SecondaryCapsuleStyle(height: Device.isMac ? 28 : 40, fontSize: Device.isMac ? 13 : 15))
                            Button(L.readerGoBack, action: close)
                                .buttonStyle(.plain)
                                .font(.plat(mac: 13, touch: 15, weight: .semibold))
                                .foregroundStyle(theme.haze)
                                .pointerHover()
                        }
                    }
                    .padding(.top, 10)
                    .transition(.opacity)
                }
            }
            .padding(.horizontal, 28)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityElement(children: .contain)
        .onChange(of: model.openingWait) { _, wait in
            // A new stage is said aloud, as the first one was.
            guard shown, let wait else { return }
            AccessibilityNotification.Announcement(Self.stage(wait, chapter: label)).post()
        }
        .task {
            // The plate at once; its line from 600 ms, and the wait admitted from 3 s.
            seconds = 0
            try? await Task.sleep(for: .milliseconds(600))
            guard !Task.isCancelled else { return }
            withAnimation(.fadeIn) { shown = true }
            AccessibilityNotification.Announcement(Self.stage(model.openingWait, chapter: label)).post()
            seconds = 1
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                withAnimation(.fadeIn) { seconds += 1 }
            }
        }
    }
}

/// A chapter that would not open: the reason in plain words, Choose source as the one action,
/// Try again and Close quiet under it.
struct ReaderChapterFailed: View {
    let model: ReaderModel
    let theme: ReaderTheme
    let failure: ReadingFailure
    let chooseSource: () -> Void
    let close: () -> Void
    @Environment(\.dynamicTypeSize) private var type

    var body: some View {
        let chapter = model.view?.chapter
        VStack(alignment: .leading, spacing: 0) {
            Spacer(minLength: 120).frame(maxHeight: 240)
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(theme.vermilion)
                    .accessibilityHidden(true)
                Text(L.readerChapterFailed(chapter: chapter.map { $0.number ?? $0.label } ?? ""))
                    .foregroundStyle(theme.silk)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .font(.ui(22, weight: .semibold))
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.isHeader)
            Text(failure.words).font(.ui(15)).foregroundStyle(theme.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 10)
            Spacer(minLength: 32)
            Button(L.readerChooseSource, action: chooseSource)
                .buttonStyle(ReaderAmberStyle(theme: theme))
            let quiet = Group {
                Button(L.readerTryAgain) { model.retry(page: nil) }
                Button(L.readerClose, action: close)
            }
            .buttonStyle(.plain)
            .font(.ui(15, weight: .semibold)).foregroundStyle(theme.silk)
            .frame(maxWidth: .infinity, minHeight: 44)
            .contentShape(.rect)
            if type.isAccessibilitySize {
                VStack(spacing: 0) { quiet }.padding(.top, 6)
            } else {
                HStack(spacing: 0) { quiet }.padding(.top, 6)
            }
        }
        .frame(maxWidth: 440, alignment: .leading)
        .padding(.horizontal, 28)
        .padding(.bottom, 26)
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
