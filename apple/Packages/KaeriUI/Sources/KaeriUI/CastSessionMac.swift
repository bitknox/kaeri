// The Mac while casting: the mini in the sidebar's foot, which grows into a panel over the
// content and shrinks back. One box travels between the two places: its frame, its corners and
// the art move together on the standard spring, the mini's row fades out over the first 100 ms
// and the panel's controls fade in from 100 ms. Under Reduce Motion the two crossfade in place.
// In the panel, Space plays and pauses, the arrows seek 10 s and Escape closes it.

#if os(macOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

enum MacCastMetrics {
    /// The mini, in the sidebar's foot.
    static let miniWidth: CGFloat = 200
    static let miniInset: CGFloat = 18
    static let miniHeight: CGFloat = 60
    /// The panel, over the content beside the sidebar.
    static let panelLeading: CGFloat = 236
    static let panelBottom: CGFloat = 12
    static let panelWidth: CGFloat = 400
    /// The room the sidebar keeps at its foot while the mini shows.
    static let sidebarRoom: CGFloat = miniHeight + 14
}

/// The mini and the panel, over the whole window: one box that grows from the sidebar's foot
/// into the panel, with the art travelling inside it.
struct MacCastMorph: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let cast = store.tv.cast
        let shown = cast.miniShown
        let expanded = shown && cast.expanded
        ZStack(alignment: .bottomLeading) {
            if expanded {
                // A click outside the panel closes it, as a popover's would.
                Color.black.opacity(0.001)
                    .contentShape(.rect)
                    .onTapGesture { cast.expanded = false }
            }
            if shown {
                if reduceMotion {
                    // In place: the mini and the panel crossfade.
                    if expanded {
                        MacCastBoxed(expanded: true).transition(.opacity)
                    } else {
                        MacCastBoxed(expanded: false).transition(.opacity)
                    }
                } else {
                    MacCastBoxed(expanded: expanded).transition(.opacity)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading)
        .animation(reduceMotion ? .reveal : .standard, value: expanded)
        .animation(.fadeIn, value: shown)
    }
}

/// The box in either place, with the art at its place in each and the content of the one shown.
private struct MacCastBoxed: View {
    let expanded: Bool

    var body: some View {
        let inner = MacCastMetrics.panelWidth - 32
        let art = expanded
            ? CGRect(x: 16, y: 16 + 22 + 12, width: inner, height: (inner * 9 / 16).rounded())
            : CGRect(x: 8, y: 8, width: 36, height: 36)
        let fade = AnyTransition.asymmetric(
            insertion: .opacity.animation(.easeOut(duration: 0.2).delay(0.1)),
            removal: .opacity.animation(.easeIn(duration: 0.1)))
        ZStack(alignment: .topLeading) {
            if expanded {
                MacCastPanel(artHeight: art.height).transition(fade)
            } else {
                MacCastMini().transition(fade)
            }
            MacCastArtwork(width: art.width, height: art.height, radius: expanded ? 10 : 6)
                .offset(x: art.minX, y: art.minY)
                .allowsHitTesting(false)
        }
        .frame(width: expanded ? MacCastMetrics.panelWidth : MacCastMetrics.miniWidth,
               height: expanded ? nil : MacCastMetrics.miniHeight, alignment: .topLeading)
        .background { MacCastBox(radius: expanded ? 16 : 12) }
        .clipShape(.rect(cornerRadius: expanded ? 16 : 12, style: .continuous))
        .padding(.leading, expanded ? MacCastMetrics.panelLeading : MacCastMetrics.miniInset)
        .padding(.bottom, expanded ? MacCastMetrics.panelBottom : MacCastMetrics.miniInset)
    }
}

/// The box both places draw: raised, with a hairline.
private struct MacCastBox: View {
    let radius: CGFloat

    var body: some View {
        RoundedRectangle(cornerRadius: radius, style: .continuous)
            .fill(Palette.raised)
            .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(.faintRule))
            .shadow(color: .black.opacity(0.35), radius: radius == 16 ? 24 : 8, y: radius == 16 ? 10 : 3)
    }
}

/// The art, small in the mini and wide in the panel.
private struct MacCastArtwork: View {
    @Environment(AppStore.self) private var store
    @Environment(\.displayScale) private var scale
    let width: CGFloat
    let height: CGFloat
    let radius: CGFloat

    var body: some View {
        let cast = store.tv.cast
        let entry = cast.showID.flatMap { store.titles[$0] }
        ZStack {
            Rectangle().fill(entry?.plateColor ?? Palette.ground)
            if let url = cast.item?.image ?? entry?.bannerUrl ?? entry?.coverUrl {
                ArtworkView(url: url, color: entry?.plateColor, maxPixelSize: Int(max(width, 200) * scale))
            }
        }
        .frame(width: width, height: height)
        .clipShape(.rect(cornerRadius: radius, style: .continuous))
        .accessibilityHidden(true)
    }
}

/// The mini: the art, the show and the TV, play or pause, and a thin line of the TV's progress.
/// A click opens the panel.
struct MacCastMini: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let cast = store.tv.cast
        let shown = CastShown(cast)
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                // The art's place: the box draws the art itself, travelling with it.
                Color.clear.frame(width: 36, height: 36)
                VStack(alignment: .leading, spacing: 1) {
                    Text(cast.showTitle).font(.mac(12, weight: .semibold)).foregroundStyle(.silk).lineLimit(1)
                    HStack(spacing: 4) {
                        if cast.rejoining {
                            ProgressView().controlSize(.mini).scaleEffect(0.7).frame(width: 10, height: 10)
                            Text(L.tvStatusReconnecting(name: cast.heldName))
                        } else if shown == .failed {
                            Circle().fill(.vermilion).frame(width: 5, height: 5)
                            Text(cast.sessionLine)
                        } else {
                            CastButtonGlyph(look: .connected, size: 10, color: .haze)
                            Text(cast.heldName)
                        }
                    }
                    .font(.mac(11))
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                if shown != .failed {
                    Button {
                        cast.command(.togglePause)
                    } label: {
                        Image(systemName: cast.itemPaused ? "play.fill" : "pause.fill")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(.silk)
                            .contentTransition(.symbolEffect(.replace))
                            .frame(width: 26, height: 26)
                            .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .disabled(shown != .playing)
                    .opacity(shown == .playing ? 1 : 0.4)
                    .accessibilityLabel(cast.itemPaused ? L.actionPlay : L.actionPause)
                }
            }
            .padding(.horizontal, 8)
            .padding(.top, 8)
            MacCastProgress()
                .padding(.horizontal, 10)
                .padding(.top, 6)
                .padding(.bottom, 7)
        }
        .frame(width: MacCastMetrics.miniWidth, height: MacCastMetrics.miniHeight)
        .contentShape(.rect(cornerRadius: 12))
        .onTapGesture { cast.expanded = true }
        .pointerStyle(.link)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(cast.showTitle)
        .accessibilityHint(L.tvRemoteAction)
        .accessibilityAction(named: L.tvRemoteAction) { cast.expanded = true }
    }
}

/// The mini's 3 pt line of the TV's progress.
private struct MacCastProgress: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let cast = store.tv.cast
        TimelineView(.periodic(from: .now, by: 1)) { context in
            let duration = cast.item?.duration ?? 0
            let fraction = duration > 0 && cast.itemLive ? min(1, cast.itemPosition(at: context.date) / duration) : 0
            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Capsule().fill(.edge)
                    Capsule().fill(.amber).frame(width: proxy.size.width * fraction)
                }
            }
            .frame(height: 3)
        }
        .accessibilityHidden(true)
    }
}

/// The panel: the line and Close, the art, the show and the episode, the seek line and
/// transport, the rows and the buttons at the foot.
struct MacCastPanel: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let artHeight: CGFloat
    @FocusState private var focused: Bool

    var body: some View {
        let cast = store.tv.cast
        let shown = CastShown(cast)
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                CastSessionStatus(touch: false)
                Spacer(minLength: 8)
                Button {
                    cast.expanded = false
                } label: {
                    Image(systemName: "chevron.down")
                        .font(.mac(11, weight: .semibold))
                        .foregroundStyle(.haze)
                        .frame(width: 22, height: 22)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L.actionClose)
            }
            .frame(height: 22)
            // The art's place: the box draws the art itself, travelling with it.
            Color.clear.frame(height: artHeight)
            Group {
                CastShowLines(touch: false, openShow: openShow)
                if shown == .failed, let reason = cast.reason {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Circle().fill(.vermilion).frame(width: 6, height: 6)
                        Text(reason).font(.mac(12)).foregroundStyle(.silk).fixedSize(horizontal: false, vertical: true)
                    }
                } else {
                    CastSessionSeek(touch: false)
                    CastSessionTransport(touch: false)
                    if let next = cast.card?.upNext, shown == .playing {
                        CastUpNextRow(next: next, touch: false)
                    } else if let undo = cast.card?.undo, shown == .playing {
                        CastUndoNote(undo: undo, touch: false)
                    }
                }
                CastSessionRows(touch: false)
                CastSessionFooter(touch: false) { cast.expanded = false }
            }
        }
        .padding(16)
        .frame(width: MacCastMetrics.panelWidth)
        .animation(.standard, value: shown)
        .focusable()
        .focusEffectDisabled()
        .focused($focused)
        .onAppear { focused = true }
        .onKeyPress(.space) {
            cast.command(.togglePause)
            return .handled
        }
        .onKeyPress(.leftArrow) {
            cast.command(.seek(by: -10))
            return .handled
        }
        .onKeyPress(.rightArrow) {
            cast.command(.seek(by: 10))
            return .handled
        }
        .onKeyPress(.escape) {
            cast.expanded = false
            return .handled
        }
        .accessibilityElement(children: .contain)
    }

    private func openShow(_ id: AnilistId) {
        store.tv.cast.expanded = false
        navigator.openTitle(id)
    }
}
#endif
