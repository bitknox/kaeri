// Go to episode: a number typed into a long list lands its row at the top. From the range menu's
// last item, ⌘L, or on a desktop digits typed into the focused list (BrowsingExtras board, 3).

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// A row Go to episode landed: shown with the selection fill until another is, or the page goes.
struct GoToLanding: Equatable {
    var number: String
    var token: Int
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
        if let range { episodeRange[title] = range }
        goToRow[title] = GoToLanding(number: rows[index].number, token: (goToRow[title]?.token ?? 0) + 1)
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
