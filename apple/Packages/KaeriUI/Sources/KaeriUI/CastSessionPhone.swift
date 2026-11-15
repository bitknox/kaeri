// iPhone and iPad while casting: the mini as the tab bar's bottom accessory, and the controller
// it zooms into, full screen. The system's zoom carries the mini's frame, corners and art into the
// controller's and back, and a drag down closes it with the finger; under Reduce Motion it
// crossfades in place. On iPad the controller lays the art beside the controls.

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The mini: the art, the show and the TV, play or pause and next, and the TV's progress along
/// its foot. While a session left running is joined again, a spinner and Reconnecting. A tap
/// opens the controller.
struct CastMini: View {
    @Environment(AppStore.self) private var store
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement

    var body: some View {
        let cast = store.tv.cast
        let inline = placement == .inline
        let joining = cast.rejoining
        let shown = CastShown(cast)
        HStack(spacing: 10) {
            if !inline { CastThumb(width: 32).clipShape(.circle) }
            VStack(alignment: .leading, spacing: 1) {
                Text(cast.showTitle).font(.ui(15, weight: .semibold)).foregroundStyle(.silk).lineLimit(1)
                if !inline {
                    HStack(spacing: 5) {
                        if joining {
                            ProgressView().controlSize(.mini).tint(.haze)
                            Text(L.tvStatusReconnecting(name: cast.heldName))
                        } else if shown == .failed {
                            Circle().fill(.vermilion).frame(width: 6, height: 6)
                            Text(cast.sessionLine)
                        } else {
                            CastButtonGlyph(look: .connected, size: 12, color: .haze)
                            Text(cast.heldName)
                        }
                    }
                    .font(.ui(12))
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if shown != .failed {
                Button {
                    cast.command(.togglePause)
                } label: {
                    Image(systemName: cast.itemPaused ? "play.fill" : "pause.fill")
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(.silk)
                        .contentTransition(.symbolEffect(.replace))
                        .frame(width: 36, height: 36)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .disabled(shown != .playing)
                .opacity(shown == .playing ? 1 : 0.4)
                .accessibilityLabel(cast.itemPaused ? L.actionPlay : L.actionPause)
                if !inline {
                    Button {
                        cast.command(.next)
                    } label: {
                        Image(systemName: "forward.end.fill")
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(.silk)
                            .frame(width: 32, height: 36)
                            .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .disabled(shown != .playing)
                    .opacity(shown == .playing ? 1 : 0.4)
                    .accessibilityLabel(L.playerNextEpisode)
                }
            }
        }
        .padding(.leading, inline ? 14 : 8)
        .padding(.trailing, 8)
        .frame(maxHeight: .infinity)
        .overlay(alignment: .bottom) {
            if !inline { CastMiniProgress().padding(.horizontal, 22) }
        }
        .contentShape(.rect)
        .onTapGesture { cast.expanded = true }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(cast.showTitle)
        .accessibilityHint(L.tvRemoteAction)
        .accessibilityAction(named: L.tvRemoteAction) { cast.expanded = true }
    }
}

/// The 2 pt line of the TV's progress along the mini's foot.
struct CastMiniProgress: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let cast = store.tv.cast
        TimelineView(.periodic(from: .now, by: 1)) { context in
            let duration = cast.item?.duration ?? 0
            let fraction = duration > 0 && cast.itemLive ? min(1, cast.itemPosition(at: context.date) / duration) : 0
            GeometryReader { proxy in
                Capsule().fill(Color.silk.opacity(0.7)).frame(width: proxy.size.width * fraction, height: 2)
            }
            .frame(height: 2)
        }
        .accessibilityHidden(true)
    }
}

/// The controller, full screen: the line and the TV, the art, the show and the episode, the seek
/// line and transport, the note to keep the app open, the rows and the buttons. The screen stays
/// awake while it is up, as the iPhone relays the stream.
struct CastController: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.dismiss) private var dismiss
    /// The controls fade in after the frame has started to grow, and out at once on the way back.
    @State private var controlsIn = false

    var body: some View {
        let cast = store.tv.cast
        let shown = CastShown(cast)
        VStack(spacing: 0) {
            header
            if sizeClass == .regular {
                HStack(alignment: .top, spacing: 32) {
                    CastArt(radius: 16).frame(maxWidth: 600)
                    VStack(alignment: .leading, spacing: 18) {
                        CastShowLines(touch: true, openShow: openShow)
                        controls(shown)
                    }
                    .frame(maxWidth: 420)
                    .opacity(controlsIn ? 1 : 0)
                }
                .padding(.horizontal, 32)
                .padding(.top, 20)
                Spacer(minLength: 0)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        CastArt(radius: 14)
                        VStack(alignment: .leading, spacing: 16) {
                            CastShowLines(touch: true, openShow: openShow)
                            controls(shown)
                        }
                        .opacity(controlsIn ? 1 : 0)
                    }
                    .padding(.horizontal, 20)
                    .padding(.top, 12)
                    .padding(.bottom, 20)
                }
                .scrollBounceBehavior(.basedOnSize)
            }
        }
        .background(Palette.ground.ignoresSafeArea())
        .preferredColorScheme(.dark)
        .animation(.standard, value: shown)
        .onAppear {
            UIApplication.shared.isIdleTimerDisabled = true
            withAnimation(.easeOut(duration: 0.2).delay(0.1)) { controlsIn = true }
        }
        .onDisappear { UIApplication.shared.isIdleTimerDisabled = false }
        .onChange(of: cast.miniShown) { _, shown in
            // The session ended on the TV or elsewhere: nothing left to drive.
            if !shown, !cast.connected { dismiss() }
        }
    }

    private var header: some View {
        let cast = store.tv.cast
        return HStack(spacing: 12) {
            Button {
                withAnimation(.easeIn(duration: 0.1)) { controlsIn = false }
                cast.expanded = false
            } label: {
                Image(systemName: "chevron.down")
                    .font(.ui(17, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(width: 44, height: 44)
                    .glassEffect(.regular.interactive(), in: .circle)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L.actionClose)
            Spacer(minLength: 8)
            CastSessionStatus(touch: true)
            Spacer(minLength: 8)
            Button {
                cast.expanded = false
                cast.dialogShown = true
            } label: {
                CastButtonGlyph(look: .connected, size: 20)
                    .frame(width: 44, height: 44)
                    .glassEffect(.regular.interactive(), in: .circle)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L.castButtonConnected(name: cast.heldName))
        }
        .padding(.horizontal, 16)
        .padding(.top, 8)
        .padding(.bottom, 4)
    }

    @ViewBuilder
    private func controls(_ shown: CastShown) -> some View {
        let cast = store.tv.cast
        if shown == .failed, let reason = cast.reason {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Circle().fill(.vermilion).frame(width: 7, height: 7).alignmentGuide(.firstTextBaseline) { $0[.bottom] - 1 }
                Text(reason).font(.ui(15)).foregroundStyle(.silk).fixedSize(horizontal: false, vertical: true)
            }
            .transition(.opacity)
        } else {
            CastSessionSeek(touch: true)
            CastSessionTransport(touch: true).padding(.vertical, 4)
            if let next = cast.card?.upNext, shown == .playing {
                CastUpNextRow(next: next, touch: true).transition(.opacity)
            } else if let undo = cast.card?.undo, shown == .playing {
                CastUndoNote(undo: undo, touch: true).transition(.opacity)
            }
            if shown == .playing {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: "info.circle").font(.ui(13)).foregroundStyle(.water).accessibilityHidden(true)
                    Text(L.castKeepOpen(device: TVRemoteModel.deviceName, name: cast.heldName))
                        .font(.ui(13))
                        .foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .transition(.opacity)
            }
        }
        CastSessionRows(touch: true)
        CastSessionFooter(touch: true) { cast.expanded = false }
    }

    private func openShow(_ id: AnilistId) {
        store.tv.cast.expanded = false
        Task { @MainActor in
            // Once the controller has zoomed back into the mini.
            try? await Task.sleep(for: .milliseconds(450))
            navigator.openTitle(id)
        }
    }
}

/// The mini over the tab bar, zooming into the controller and back. Before iOS 26.1 the
/// controller opens from the cast button's dialog and the title page.
struct CastAccessoryCover: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let zoom: Namespace.ID

    func body(content: Content) -> some View {
        let cast = store.tv.cast
        content
            .fullScreenCover(isPresented: Binding(
                get: { cast.expanded },
                set: { cast.expanded = $0 }
            )) {
                CastController()
                    .environment(store)
                    .environment(navigator)
                    .navigationTransition(.zoom(sourceID: CastAccessoryCover.zoomID, in: zoom))
            }
    }

    static let zoomID = "cast-mini" // i18n-skip
}
#endif
