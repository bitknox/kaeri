// Go to episode: a number typed into a long list lands its row at the top. From the range menu's
// last item, ⌘L, or on a desktop digits typed into the focused list (BrowsingExtras board, 3).

#if os(macOS)
import AppKit
#else
import UIKit
#endif
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// A row Go to episode landed: shown with the selection fill until another is, or the page goes.
struct GoToLanding: Equatable {
    var number: String
    /// The range the row is in, for a long series: the list switches to it as it lands.
    var range: Int?
    var token: Int
}

/// How a landed row comes to the top of its list (Browsing extras board, motion): a row a screen
/// or so away in the range shown scrolls there; another range, a long jump and Reduce Motion
/// crossfade to the list already there, so nothing streaks past.
@MainActor
enum GoToMove {
    case scroll
    case crossfade

    /// The scroll's curve: `standard`, but on the Mac, where a scroll view asked to scroll on a
    /// spring stays where it is, the same length on a timing curve that starts as fast and
    /// settles alike.
    static var scrolling: Animation {
        #if os(macOS)
        .timingCurve(0.3, 0, 0, 1, duration: 0.3)
        #else
        .standard
        #endif
    }

    /// `index` is the row's place in the range, `pickersTop` where the list's heading stands from
    /// the top of the visible area, `rowHeight` a row's height on average.
    static func to(index: Int, switching: Bool, reduced: Bool, pickersTop: CGFloat, rowHeight: CGFloat) -> GoToMove {
        if switching || reduced { return .crossfade }
        let y = pickersTop + 34 + CGFloat(index) * rowHeight
        let screen = WindowCrossfade.visibleHeight
        return y > -screen * 0.5 && y < screen * 1.5 ? .scroll : .crossfade
    }

    /// The same for a lazy list, measured in rows: the row scrolls when it is within a screen's
    /// worth of rows of those on screen (`onScreen`, indexes in the range), else it crossfades.
    /// With no row on screen yet, the pickers' place says, as above.
    static func to(
        index: Int, switching: Bool, reduced: Bool, onScreen: [Int], pickersTop: CGFloat, rowHeight: CGFloat
    ) -> GoToMove {
        if switching || reduced { return .crossfade }
        guard let low = onScreen.min(), let high = onScreen.max() else {
            return to(index: index, switching: false, reduced: false, pickersTop: pickersTop, rowHeight: rowHeight)
        }
        let span = high - low + 1
        return index >= low - span && index <= high + span ? .scroll : .crossfade
    }
}

/// A crossfade of the window from what it shows now to what it shows once `change` has run: the
/// window as it is covers it, the change happens under the cover without motion, and the cover
/// fades out over 0.3 s, so both sides are drawn throughout and a long scroll never streaks.
@MainActor
enum WindowCrossfade {
    static func run(_ change: @escaping @MainActor () async -> Void) {
        #if os(macOS)
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow ?? NSApp.windows.first(where: \.isVisible),
              let view = window.contentView,
              let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds)
        else { Task { await change() }; return }
        view.cacheDisplay(in: view.bounds, to: bitmap)
        let image = NSImage(size: view.bounds.size)
        image.addRepresentation(bitmap)
        let cover = NSImageView(frame: view.bounds)
        cover.image = image
        cover.imageScaling = .scaleAxesIndependently
        cover.autoresizingMask = [.width, .height]
        cover.wantsLayer = true
        view.addSubview(cover, positioned: .above, relativeTo: nil)
        Task {
            await change()
            try? await Task.sleep(for: .milliseconds(50))
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.3
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                cover.animator().alphaValue = 0
            } completionHandler: {
                MainActor.assumeIsolated { cover.removeFromSuperview() }
            }
        }
        #else
        guard let window = UIApplication.shared.connectedScenes
                .compactMap({ ($0 as? UIWindowScene)?.keyWindow }).first,
              let cover = window.snapshotView(afterScreenUpdates: false)
        else { Task { await change() }; return }
        cover.isUserInteractionEnabled = false
        window.addSubview(cover)
        Task {
            await change()
            try? await Task.sleep(for: .milliseconds(50))
            UIView.animate(withDuration: 0.3, delay: 0, options: [.curveEaseInOut, .allowUserInteraction]) {
                cover.alpha = 0
            } completion: { _ in
                cover.removeFromSuperview()
            }
        }
        #endif
    }

    /// The height of the window's content, about what a list shows at once.
    static var visibleHeight: CGFloat {
        #if os(macOS)
        (NSApp.keyWindow ?? NSApp.mainWindow)?.contentLayoutRect.height ?? 800
        #else
        UIApplication.shared.connectedScenes.compactMap { ($0 as? UIWindowScene)?.keyWindow }.first?.bounds.height ?? 800
        #endif
    }

    /// Runs `change` with no animation, not even the ones views attach to their values.
    static func still(_ change: () -> Void) {
        var transaction = Transaction()
        transaction.disablesAnimations = true
        withTransaction(transaction, change)
    }
}

/// Digits typed into an episode list, waiting for a pause or Return.
struct TypedEpisode: Equatable {
    var title: AnilistId
    var text: String
    var failed = false
}

extension Navigator {
    /// Lands row `index` of `rows` (in range `range`, for a long series): switches range if it is
    /// not the one open, opens the watched run the row is folded into, and has the list scroll
    /// the row to its top.
    func land(on title: AnilistId, rows: [EpisodeRow], index: Int, range: Int?) {
        guard rows.indices.contains(index) else { return }
        goToRow[title] = GoToLanding(number: rows[index].number, range: range, token: (goToRow[title]?.token ?? 0) + 1)
    }
}

extension Navigator {
    /// One more digit typed into `title`'s focused list: the capsule says the number so far, and
    /// the jump comes after 0.6 s of quiet, or at Return (`jumpToTyped`).
    func typeEpisode(_ digits: String, title: AnilistId, rows: [EpisodeRow]) {
        let text = (typedEpisode?.title == title ? typedEpisode?.text ?? "" : "") + digits
        withAnimation(.fadeIn) { typedEpisode = TypedEpisode(title: title, text: text) }
        typingPause?.cancel()
        typingPause = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(600))
            if !Task.isCancelled { self?.jumpToTyped(title: title, rows: rows) }
        }
    }

    /// The typed number's row lands, or the capsule turns vermilion; either way it fades out a
    /// second later.
    func jumpToTyped(title: AnilistId, rows: [EpisodeRow]) {
        typingPause?.cancel()
        guard let typed = typedEpisode, typed.title == title else { return }
        let result = goToResult(rows: rows, typed: typed.text)
        if case .row(let index, let range) = result {
            land(on: title, rows: rows, index: Int(index), range: range.map(Int.init))
        } else {
            typedEpisode?.failed = true
        }
        typingPause = Task { [weak self] in
            try? await Task.sleep(for: .seconds(1))
            if !Task.isCancelled { withAnimation(.fadeOut) { self?.typedEpisode = nil } }
        }
    }
}

/// What Go to episode said of what was typed, in the engine's words.
@MainActor
func goToResult(rows: [EpisodeRow], typed: String) -> GoTo {
    goToEpisode(rows: rows, typed: typed, now: Int64(Date.now.timeIntervalSince1970))
}

#if os(macOS) || os(iOS)
/// The popover of Go to episode, on the Mac and iPad: a number field with the show's span as a
/// hint, Cancel and Go. Return goes, Escape cancels. Go is the default button in the selected
/// fill, not amber. A number the table does not have says so and keeps the popover open with the
/// number selected.
struct GoToEpisodeForm: View {
    let rows: [EpisodeRow]
    let land: (GoTo) -> Void
    let cancel: () -> Void
    @State private var typed = ""
    @State private var message: String?
    @FocusState private var focused: Bool

    var body: some View {
        let result = goToResult(rows: rows, typed: typed)
        VStack(alignment: .leading, spacing: 8) {
            Text(L.episodeGoToTitle)
                .font(.mac(13, weight: .semibold))
                .foregroundStyle(.silk)
            TextField("", text: $typed)
                .textFieldStyle(.plain)
                .font(.mac(13))
                .monospacedDigit()
                .foregroundStyle(.silk)
                .padding(.horizontal, 8)
                .frame(height: 28)
                .background(Palette.ground, in: .rect(cornerRadius: 6))
                .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Palette.water, lineWidth: 2))
                .focused($focused)
                .accessibilityLabel(goToLabel(rows: rows))
                .onSubmit { go(result) }
                .onChange(of: typed) { _, now in
                    let digits = now.filter { $0.isNumber || $0 == "." }
                    if digits != now { typed = digits }
                    message = nil
                }
                #if os(iOS)
                .keyboardType(.decimalPad)
                #endif
            if let message {
                Label(message, systemImage: "exclamationmark.triangle")
                    .font(.mac(12))
                    .foregroundStyle(.vermilion)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                Text(goToHint(rows: rows))
                    .font(.mac(12))
                    .foregroundStyle(.haze)
            }
            HStack(spacing: 8) {
                Spacer()
                Button(L.actionCancel, action: cancel)
                    .buttonStyle(SecondaryCapsuleStyle(height: 24, fontSize: 12))
                    .keyboardShortcut(.cancelAction)
                Button(L.episodeGoToGo) { go(result) }
                    .buttonStyle(SelectedCapsuleStyle(height: 24, fontSize: 12))
                    .disabled(result == .empty)
            }
        }
        .padding(12)
        .frame(width: 220)
        .onAppear { focused = true }
    }

    private func go(_ result: GoTo) {
        switch result {
        case .empty: break
        case .row: land(result)
        case .noSuch, .notOut:
            message = goToMessage(result: result)
            focused = true
        }
    }
}

/// Go in the selected fill, the default button without amber.
struct SelectedCapsuleStyle: ButtonStyle {
    var height: CGFloat = 28
    var fontSize: CGFloat = 13
    @Environment(\.isEnabled) private var enabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.mac(fontSize, weight: .semibold))
            .foregroundStyle(enabled ? .silk : .haze)
            .padding(.horizontal, 14)
            .frame(height: height)
            .background(Palette.raisedSelected.opacity(configuration.isPressed ? 0.7 : 1), in: .capsule)
            .opacity(enabled ? 1 : 0.5)
            .contentShape(.capsule)
    }
}
#endif

extension View {
    /// Go to episode's popover hanging from this view: the range menu, or the heading of a list
    /// with no ranges.
    func goToPopover(isPresented: Binding<Bool>, rows: [EpisodeRow], land: @escaping (GoTo) -> Void) -> some View {
        popover(isPresented: isPresented, arrowEdge: .bottom) {
            GoToEpisodeForm(rows: rows, land: land) { isPresented.wrappedValue = false }
                #if os(iOS)
                .presentationCompactAdaptation(.popover)
                #endif
        }
    }
}

/// Typed digits on a desktop episode list: a capsule at the top right ("Episode 382") that updates
/// in place and fades out a second after the jump.
/// Where the capsule stands over a title page: its top trailing corner, `top` down.
struct TypedCapsuleSlot: View {
    @Environment(Navigator.self) private var navigator
    let title: AnilistId
    let top: CGFloat

    var body: some View {
        if let typed = navigator.typedEpisode, typed.title == title {
            TypedEpisodeCapsule(text: typed.text, failed: typed.failed)
                .padding(EdgeInsets(top: top, leading: 0, bottom: 0, trailing: 28))
                .transition(.opacity)
                .allowsHitTesting(false)
        }
    }
}

struct TypedEpisodeCapsule: View {
    let text: String
    var failed = false

    var body: some View {
        Text(L.episodeNumberString(number: text))
            .font(.mac(12, weight: .semibold))
            .monospacedDigit()
            .foregroundStyle(failed ? .vermilion : .silk)
            .padding(.horizontal, 12)
            .frame(height: 26)
            .background(Palette.raisedSelected, in: .capsule)
            .contentTransition(.numericText())
    }
}
