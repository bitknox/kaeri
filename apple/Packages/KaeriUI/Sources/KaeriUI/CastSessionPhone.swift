// iPhone and iPad while casting: the mini as the tab bar's bottom accessory, and the controller
// it grows into, full screen. One box carries the mini's frame and corners into the controller's
// and back, read from where the accessory really is. A tap on the mini opens it; a drag up from
// the mini opens it and a drag down on the controller closes it, the box following the finger
// and settling with the release's speed (`CastDrawer`), and a drawer on its way can be caught.
// Under Reduce Motion it crossfades in place. On iPad the controller lays the art beside the
// controls.

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The mini: the art, the show and the TV, play or pause and next, and the TV's progress along
/// its foot. While a session left running is joined again, a spinner and Reconnecting. A tap or
/// a drag up opens the controller.
struct CastMini: View {
    @Environment(AppStore.self) private var store
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement
    @Environment(CastDrawer.self) private var drawer: CastDrawer?
    @GestureState private var dragging = false

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
        .simultaneousGesture(rise, isEnabled: drawer != nil)
        .onChange(of: dragging) { _, on in
            // The system took the drag: no release came.
            if !on, let drawer { settle(drawer, speed: 0) }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(cast.showTitle)
        .accessibilityHint(L.tvRemoteAction)
        .accessibilityAction(named: L.tvRemoteAction) { cast.expanded = true }
    }

    /// A drag up opens the controller with the finger: the box's top travels from the mini's top
    /// to the screen's, one to one.
    private var rise: some Gesture {
        DragGesture(minimumDistance: 8, coordinateSpace: .global)
            .updating($dragging) { _, state, _ in state = true }
            .onChanged { value in
                guard let drawer else { return }
                if drawer.owner == nil {
                    drawer.travel = drawer.anchor?.frame?.minY ?? 600
                    guard drawer.begin(DrawerDrag(value.translation), fromMini: true, rise: -value.translation.height) else { return }
                    store.tv.cast.expanded = true
                }
                if drawer.owner == true { drawer.drag(rise: -value.translation.height) }
            }
            .onEnded { value in
                guard let drawer else { return }
                settle(drawer, speed: -value.velocity.height)
            }
    }

    private func settle(_ drawer: CastDrawer, speed: CGFloat) {
        guard drawer.held else {
            drawer.cancel()
            return
        }
        let open = drawer.release(speed: speed)
        if store.tv.cast.expanded != open { store.tv.cast.expanded = open }
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
/// line and transport, the rows and the buttons. The screen stays awake while it is up.
struct CastController: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(CastDrawer.self) private var drawer: CastDrawer?

    var body: some View {
        let cast = store.tv.cast
        let shown = CastShown(cast)
        // The controls fade in once the box is a third of the way open, and out below that.
        let controlsIn = drawer?.revealed ?? true
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
                    .animation(controlsIn ? .easeOut(duration: 0.2) : .easeIn(duration: 0.1), value: controlsIn)
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
                        .animation(controlsIn ? .easeOut(duration: 0.2) : .easeIn(duration: 0.1), value: controlsIn)
                    }
                    .padding(.horizontal, 20)
                    .padding(.top, 12)
                    .padding(.bottom, 20)
                }
                .scrollBounceBehavior(.basedOnSize)
                .scrollIndicators(.hidden)
                // At the top, a drag down closes the drawer rather than scrolling; while the
                // drawer has the finger, the scroll stays where it is.
                .onScrollGeometryChange(for: Bool.self) { geometry in
                    geometry.contentOffset.y <= -geometry.contentInsets.top + 1
                } action: { _, top in
                    drawer?.atTop = top
                }
                .scrollDisabled(drawer?.held ?? false)
            }
        }
        .background(Palette.ground.ignoresSafeArea())
        .preferredColorScheme(.dark)
        .animation(.standard, value: shown)
        .environment(\.castSliding, CastSliding { drawer?.sliding = $0 })
        .onAppear { UIApplication.shared.isIdleTimerDisabled = true }
        .onDisappear { UIApplication.shared.isIdleTimerDisabled = false }
        .onChange(of: cast.miniShown) { _, shown in
            // The session ended on the TV or elsewhere: nothing left to drive.
            if !shown, !cast.connected { cast.expanded = false }
        }
    }

    private var header: some View {
        let cast = store.tv.cast
        return HStack(spacing: 12) {
            Button {
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

    /// The window's safe area, which the controller keeps while the box fills the screen.
    var insets: EdgeInsets? {
        guard let window = view?.window else { return nil }
        let i = window.safeAreaInsets
        return EdgeInsets(top: i.top, leading: i.left, bottom: i.bottom, trailing: i.right)
    }

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
/// once on a clear background; the box inside it does the travelling, and the cover goes once
/// the drawer rests closed.
struct CastAccessoryCover: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let anchor: CastMiniAnchor
    let drawer: CastDrawer
    @State private var presented = false

    func body(content: Content) -> some View {
        let cast = store.tv.cast
        content
            .fullScreenCover(isPresented: $presented) {
                CastMorph(anchor: anchor, drawer: drawer)
                    .environment(store)
                    .environment(navigator)
                    .environment(drawer)
                    .presentationBackground(.clear)
            }
            .onChange(of: cast.expanded, initial: true) { _, open in
                if open, !presented {
                    withTransaction(\.disablesAnimations, true) { presented = true }
                } else if !open, presented, drawer.shut {
                    // Closed before it had moved: nothing will settle.
                    withTransaction(\.disablesAnimations, true) { presented = false }
                }
            }
            .onChange(of: drawer.shut) { _, shut in
                if shut, !cast.expanded, presented {
                    withTransaction(\.disablesAnimations, true) { presented = false }
                }
            }
    }
}

/// One box that grows from the mini into the controller and shrinks back: its frame and corners
/// follow the drawer, the finger or the standard spring, with the controller drawn inside it at
/// full opacity; the mini's row fades out over the first quarter of the way and the controls
/// fade in past a third. A drag down closes it and a drag while it moves catches it. Under
/// Reduce Motion the controller crossfades in place, with the finger too.
private struct CastMorph: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let anchor: CastMiniAnchor
    let drawer: CastDrawer
    @State private var from: CGRect?
    @State private var insets: EdgeInsets?
    @GestureState private var dragging = false

    init(anchor: CastMiniAnchor, drawer: CastDrawer) {
        self.anchor = anchor
        self.drawer = drawer
        _from = State(initialValue: anchor.frame)
        _insets = State(initialValue: anchor.insets)
    }

    var body: some View {
        let cast = store.tv.cast
        GeometryReader { proxy in
            let origin = proxy.frame(in: .global).origin
            let full = CGRect(origin: .zero, size: proxy.size)
            let mini = reduceMotion ? nil : from.map { $0.offsetBy(dx: -origin.x, dy: -origin.y) }
            let p = drawer.progress
            let box = mini.map { Self.box(from: $0, to: full, at: p) } ?? full
            let scale = box.width / max(full.width, 1)
            let radius = mini.map { Self.radius(mini: $0.height / 2, at: p) } ?? 0
            ZStack(alignment: .topLeading) {
                // The box spans the whole screen, so the controller gets the safe area back.
                CastController()
                    .safeAreaPadding(insets ?? proxy.safeAreaInsets)
                    .frame(width: full.width, height: full.height)
                    .scaleEffect(scale, anchor: .topLeading)
                    .offset(x: box.minX, y: box.minY)
                if let mini {
                    CastMini()
                        .frame(width: box.width, height: mini.height)
                        .offset(x: box.minX, y: box.minY)
                        .opacity(max(0, 1 - p / 0.25))
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
            .opacity(mini == nil ? p : 1)
            .simultaneousGesture(close(travel: mini?.minY ?? full.height / 2))
        }
        .ignoresSafeArea()
        .accessibilityAction(.escape) { cast.expanded = false }
        .onAppear {
            // One frame on the mini first, then the box grows. A finger already holding it (a
            // drag up from the mini) keeps it.
            Task { @MainActor in if cast.expanded { drawer.settle(open: true) } }
        }
        .onChange(of: cast.expanded) { _, expanded in
            // It lands on the mini where it is drawn now.
            if !expanded, let frame = anchor.frame { from = frame }
            drawer.settle(open: expanded)
        }
        .onChange(of: dragging) { _, on in
            // The system took the drag: no release came.
            if !on { settle(speed: 0) }
        }
    }

    /// A drag on the controller: down from the top of its scroll closes it, and up or down
    /// while it moves catches it; the box's top follows the finger one to one.
    private func close(travel: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 10, coordinateSpace: .global)
            .updating($dragging) { _, state, _ in state = true }
            .onChanged { value in
                let rise = -value.translation.height
                if drawer.owner == nil {
                    drawer.travel = travel
                    guard drawer.begin(DrawerDrag(value.translation), fromMini: false, rise: rise) else { return }
                }
                if drawer.owner == true { drawer.drag(rise: rise) }
            }
            .onEnded { value in settle(speed: -value.velocity.height) }
    }

    private func settle(speed: CGFloat) {
        guard drawer.held else {
            drawer.cancel()
            return
        }
        let cast = store.tv.cast
        let open = drawer.release(speed: speed)
        if cast.expanded != open { cast.expanded = open }
    }

    /// The box at `p` of the way from the mini to the full screen.
    static func box(from mini: CGRect, to full: CGRect, at p: CGFloat) -> CGRect {
        let p = min(max(p, 0), 1)
        func mix(_ a: CGFloat, _ b: CGFloat) -> CGFloat { a + (b - a) * p }
        return CGRect(
            x: mix(mini.minX, full.minX), y: mix(mini.minY, full.minY),
            width: mix(mini.width, full.width), height: mix(mini.height, full.height))
    }

    /// The box's corners: the mini's capsule, opening to the screen's own curve and squaring off
    /// over the last stretch, where the screen's corners hide them.
    static func radius(mini: CGFloat, at p: CGFloat) -> CGFloat {
        let open: CGFloat = 38
        if p <= 0.85 { return mini + (open - mini) * max(p, 0) / 0.85 }
        return open * (1 - min(p, 1)) / 0.15
    }
}
#endif
