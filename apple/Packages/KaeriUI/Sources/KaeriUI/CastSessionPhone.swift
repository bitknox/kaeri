// iPhone and iPad while casting: the mini as the tab bar's bottom accessory, and the controller
// it grows into, full screen. One box carries the mini's frame and corners into the controller's
// and back, read from where the accessory really is, and a drag down closes it with the finger;
// under Reduce Motion it crossfades in place. On iPad the controller lays the art beside the
// controls.

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
    /// The controls fade in after the frame has started to grow, and out at once on the way back.
    @State private var controlsIn = false

    var body: some View {
        let cast = store.tv.cast
        let shown = CastShown(cast)
        VStack(spacing: 0) {
            if sizeClass != .regular {
                // A drag down closes it, as the grabber says.
                Capsule().fill(Palette.rule).frame(width: 36, height: 5).padding(.top, 6).accessibilityHidden(true)
            }
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
                .scrollIndicators(.hidden)
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
            if !shown, !cast.connected { cast.expanded = false }
        }
        .onChange(of: cast.expanded) { _, open in
            if !open { withAnimation(.easeIn(duration: 0.1)) { controlsIn = false } }
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
            // Once the controller has shrunk back into the mini.
            try? await Task.sleep(for: .milliseconds(450))
            navigator.openTitle(id)
        }
    }
}

/// Where the mini sits on screen, read from the accessory itself at the moment the controller
/// opens or closes, so the box leaves from and lands on the mini as it is drawn.
@MainActor
final class CastMiniAnchor {
    weak var view: UIView?

    /// The mini's frame in its window, or nil once it is gone.
    var frame: CGRect? {
        guard let view, view.window != nil, view.bounds.width > 0 else { return nil }
        return view.convert(view.bounds, to: nil)
    }
}

/// An empty view behind the mini that tells the anchor where the mini is.
struct CastMiniProbe: UIViewRepresentable {
    let anchor: CastMiniAnchor

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.isUserInteractionEnabled = false
        anchor.view = view
        return view
    }

    func updateUIView(_ view: UIView, context: Context) { anchor.view = view }
}

/// The controller over everything, opened from the mini. The cover itself appears and goes at
/// once on a clear background; the box inside it does the travelling.
struct CastAccessoryCover: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let anchor: CastMiniAnchor
    @State private var presented = false

    func body(content: Content) -> some View {
        let cast = store.tv.cast
        content
            .fullScreenCover(isPresented: $presented) {
                CastMorph(anchor: anchor) {
                    withTransaction(\.disablesAnimations, true) { presented = false }
                }
                .environment(store)
                .environment(navigator)
                .presentationBackground(.clear)
            }
            .onChange(of: cast.expanded, initial: true) { _, open in
                if open, !presented {
                    withTransaction(\.disablesAnimations, true) { presented = true }
                }
            }
    }
}

/// One box that grows from the mini into the controller and shrinks back: its frame and corners
/// move on the standard spring with the controller drawn inside it at full opacity, the mini's
/// row fades out over the first 100 ms and the controls fade in from 100 ms. A drag down from
/// the top closes it with the finger. Under Reduce Motion the controller crossfades in place.
private struct CastMorph: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let anchor: CastMiniAnchor
    let closed: () -> Void
    @State private var from: CGRect?
    @State private var open = false
    @State private var drag: CGFloat = 0

    init(anchor: CastMiniAnchor, closed: @escaping () -> Void) {
        self.anchor = anchor
        self.closed = closed
        _from = State(initialValue: anchor.frame)
    }

    var body: some View {
        let cast = store.tv.cast
        GeometryReader { proxy in
            let origin = proxy.frame(in: .global).origin
            let full = CGRect(origin: .zero, size: proxy.size)
            let mini = from.map { $0.offsetBy(dx: -origin.x, dy: -origin.y) }
            let travels = !reduceMotion && mini != nil
            let box = travels && !open ? (mini ?? full) : dragged(full)
            let scale = box.width / max(full.width, 1)
            let radius = open ? (drag > 0 ? 38 : 0) : (mini?.height ?? 0) / 2
            ZStack(alignment: .topLeading) {
                CastController()
                    .frame(width: full.width, height: full.height)
                    .scaleEffect(scale, anchor: .topLeading)
                    .offset(x: box.minX, y: box.minY)
                if travels, let mini {
                    CastMini()
                        .frame(width: box.width, height: mini.height)
                        .offset(x: box.minX, y: box.minY)
                        .opacity(open ? 0 : 1)
                        .animation(open ? .easeIn(duration: 0.1) : .easeOut(duration: 0.2).delay(0.1), value: open)
                        .allowsHitTesting(false)
                        .accessibilityHidden(true)
                }
            }
            .frame(width: full.width, height: full.height, alignment: .topLeading)
            .mask(alignment: .topLeading) {
                RoundedRectangle(cornerRadius: radius, style: .continuous)
                    .frame(width: box.width, height: box.height)
                    .offset(x: box.minX, y: box.minY)
            }
            .opacity(travels || open ? 1 : 0)
            .simultaneousGesture(
                DragGesture(minimumDistance: 12)
                    .onChanged { value in
                        guard open, value.startLocation.y < 140 else { return }
                        drag = max(0, value.translation.height)
                    }
                    .onEnded { value in
                        guard open, drag > 0 else { return }
                        if value.translation.height > 120 || value.predictedEndTranslation.height > 260 {
                            cast.expanded = false
                        } else {
                            withAnimation(.standard) { drag = 0 }
                        }
                    }
            )
        }
        .ignoresSafeArea()
        .accessibilityAction(.escape) { cast.expanded = false }
        .onAppear {
            // One frame on the mini first, then the box grows.
            Task { @MainActor in
                withAnimation(reduceMotion ? .easeInOut(duration: 0.25) : .standard) { open = true }
            }
        }
        .onChange(of: cast.expanded) { _, expanded in
            if expanded {
                if !open { withAnimation(reduceMotion ? .easeInOut(duration: 0.25) : .standard) { open = true } }
                return
            }
            from = anchor.frame
            withAnimation(reduceMotion || from == nil ? .easeInOut(duration: 0.25) : .standard) {
                open = false
                drag = 0
            } completion: {
                if !cast.expanded { closed() }
            }
        }
    }

    /// The open box, following the finger down and shrinking a little as it goes.
    private func dragged(_ full: CGRect) -> CGRect {
        guard drag > 0 else { return full }
        let k = min(drag / max(full.height, 1), 1)
        let width = full.width * (1 - 0.2 * k)
        let height = full.height * (1 - 0.2 * k)
        return CGRect(x: (full.width - width) / 2, y: drag, width: width, height: height)
    }
}
#endif
