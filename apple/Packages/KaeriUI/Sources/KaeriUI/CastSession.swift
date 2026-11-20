// The cast session's parts every Apple platform shares: the cast button and its glyph, the
// chooser and the connected dialog it opens, and the controller's pieces (the status line, the
// art, the seek line, the transport, the rows and the buttons at the foot). The engine's session
// view says what they read (CastModel's `held`); the card it follows adds the tracks and why a
// play failed. Within playing nothing here dims, disables or leaves: only starting is dimmed.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

// MARK: - The glyph

/// The cast glyph on a 24 grid: a screen with waves in its lower corner. Connected fills the
/// screen; connecting lights the dot and the waves in turn.
struct CastButtonGlyph: View {
    let look: CastButtonLook
    var size: CGFloat = 22
    var color: Color = .silk
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let scale = size / 24
        let line = StrokeStyle(lineWidth: 1.8, lineCap: .round, lineJoin: .round)
        ZStack {
            if look == .connected {
                CastGlyphPart.filledScreen.path(scale: scale).fill(color)
            } else {
                CastGlyphPart.screen.path(scale: scale).stroke(color, style: line)
            }
            if look == .connecting {
                TimelineView(.animation(paused: reduceMotion)) { context in
                    let t = context.date.timeIntervalSinceReferenceDate
                    parts(scale: scale, line: line, opacities: reduceMotion ? [1, 0.35, 0.35] : (0..<3).map { pulse(t, delay: Double($0) * 0.2) })
                }
            } else {
                parts(scale: scale, line: line, opacities: [1, 1, 1])
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }

    private func parts(scale: CGFloat, line: StrokeStyle, opacities: [Double]) -> some View {
        ZStack {
            CastGlyphPart.dot.path(scale: scale).fill(color).opacity(opacities[0])
            CastGlyphPart.innerWave.path(scale: scale).stroke(color, style: line).opacity(opacities[1])
            CastGlyphPart.outerWave.path(scale: scale).stroke(color, style: line).opacity(opacities[2])
        }
    }

    /// 0.3 to 1 and back over 1.2 s.
    private func pulse(_ t: Double, delay: Double) -> Double {
        let phase = ((t - delay).truncatingRemainder(dividingBy: 1.2) + 1.2).truncatingRemainder(dividingBy: 1.2) / 1.2
        return 0.3 + 0.7 * (0.5 - 0.5 * cos(phase * 2 * .pi))
    }
}

/// The glyph's parts, drawn from the board's 24-unit paths.
enum CastGlyphPart {
    case screen, filledScreen, outerWave, innerWave, dot

    func path(scale: CGFloat) -> Path {
        var path = Path()
        switch self {
        case .screen:
            path.move(to: CGPoint(x: 3, y: 7.4))
            path.addLine(to: CGPoint(x: 3, y: 3.6))
            path.addLine(to: CGPoint(x: 21, y: 3.6))
            path.addLine(to: CGPoint(x: 21, y: 19.2))
            path.addLine(to: CGPoint(x: 13.6, y: 19.2))
        case .filledScreen:
            path.move(to: CGPoint(x: 3.6, y: 4.4))
            path.addLine(to: CGPoint(x: 20.4, y: 4.4))
            path.addLine(to: CGPoint(x: 20.4, y: 18.4))
            path.addLine(to: CGPoint(x: 13.2, y: 18.4))
            // The arc from (13.2, 18.4) round to (3.6, 8.8), radius 10, centred at (3.6, 18.4).
            path.addArc(center: CGPoint(x: 3.6, y: 18.4), radius: 9.6, startAngle: .degrees(0), endAngle: .degrees(-90), clockwise: true)
            path.closeSubpath()
        case .outerWave:
            path.move(to: CGPoint(x: 3, y: 10.4))
            path.addArc(center: CGPoint(x: 3, y: 19.4), radius: 9, startAngle: .degrees(-90), endAngle: .degrees(-1.3), clockwise: false)
        case .innerWave:
            path.move(to: CGPoint(x: 3, y: 14))
            path.addArc(center: CGPoint(x: 3, y: 19.4), radius: 5.4, startAngle: .degrees(-90), endAngle: .degrees(-2), clockwise: false)
        case .dot:
            path.addEllipse(in: CGRect(x: 3.9 - 1.6, y: 18.3 - 1.6, width: 3.2, height: 3.2))
        }
        return path.applying(CGAffineTransform(scaleX: scale, y: scale))
    }
}

// MARK: - The button

/// The cast button in the top bars: the outline while a TV is found, the waves while one
/// connects, filled once it holds one. Hidden while there is none. It opens the chooser, or with
/// a TV connected the dialog that says so: a popover on iPad and the Mac, a sheet on iPhone.
struct CastButton: View {
    @Environment(AppStore.self) private var store
    /// Only a button on screen presents: each tab keeps its own bar.
    @State private var onScreen = false

    var body: some View {
        let tv = store.tv
        let cast = tv.cast
        let look = tv.castButton
        if look != .hidden {
            Button {
                tv.castButtonTapped()
            } label: {
                CastButtonGlyph(look: look, size: buttonGlyph)
                    .animation(.standard, value: look)
                    .contentShape(.rect)
            }
            .accessibilityLabel(label(look))
            .tooltip(label(look))
            .onAppear { onScreen = true }
            .onDisappear { onScreen = false }
            .popover(isPresented: Binding(
                get: { onScreen && cast.chooserShown },
                set: { if !$0 { cast.chooserShown = false } }
            ), arrowEdge: .top) {
                CastChooser()
                    .environment(store)
                    .castPopoverSizing()
            }
            .popover(isPresented: Binding(
                get: { onScreen && cast.dialogShown && !cast.chooserShown },
                set: { if !$0 { cast.dialogShown = false } }
            ), arrowEdge: .top) {
                CastConnectedDialog()
                    .environment(store)
                    .castPopoverSizing()
            }
            .transition(.opacity)
        }
    }

    private var buttonGlyph: CGFloat {
        #if os(macOS)
        18
        #else
        22
        #endif
    }

    private func label(_ look: CastButtonLook) -> String {
        let tv = store.tv
        let name = tv.cast.connected ? tv.cast.heldName : tv.card?.tvName ?? tv.cast.tvName
        switch look {
        case .connecting: return L.castButtonConnecting(name: name)
        case .connected: return L.castButtonConnected(name: name)
        case .available, .hidden: return L.tvPlayOn
        }
    }
}

extension View {
    /// A popover on iPad and the Mac; on iPhone a sheet at half height.
    func castPopoverSizing() -> some View {
        #if os(iOS)
        self
            .presentationCompactAdaptation(.sheet)
            .presentationDetents([.medium, .large])
            .presentationDragIndicator(.visible)
        #else
        self
        #endif
    }
}

// MARK: - The chooser

/// Play on TV from the cast button: TVs with Chromecast built in, then Kaeri TVs on this
/// network. A pick connects; what is played then goes there.
struct CastChooser: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let tv = store.tv
        let kaeri = tv.rows.filter { $0.kind != .needsUpdate }
        VStack(alignment: .leading, spacing: 0) {
            CastSheetHeader(title: L.tvPlayOn) { dismiss() }
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    if tv.cast.localNetworkDenied {
                        CastGroupBox { CastLocalNetworkRow() }
                    } else {
                        if !tv.cast.rows.isEmpty {
                            CastGroupHeading(text: L.castChromecast)
                            CastGroupBox {
                                ForEach(tv.cast.rows) { shown in
                                    CastChooserRow(name: shown.row.name, line: CastWords.route(shown.row), gone: shown.gone) {
                                        tv.connectFromChooser(shown.row)
                                    }
                                }
                            }
                        }
                        if !kaeri.isEmpty {
                            CastGroupHeading(text: L.tvSectionNetwork).padding(.top, tv.cast.rows.isEmpty ? 0 : 8)
                            CastGroupBox {
                                ForEach(kaeri, id: \.key) { row in
                                    CastChooserRow(name: row.name, line: row.detail, gone: false) {
                                        tv.chooseFromChooser(row)
                                    }
                                }
                            }
                        }
                        if tv.cast.rows.isEmpty, kaeri.isEmpty {
                            CastGroupBox {
                                TVSearchingRow(phone: CastMetrics.touch, request: nil, cast: tv.cast.shown)
                                    .padding(.horizontal, CastMetrics.rowInset)
                            }
                        }
                        if let problem = tv.cast.problem {
                            TVProblemLine(text: problem, touch: CastMetrics.touch).padding(.horizontal, 4)
                        }
                    }
                }
                .padding(.horizontal, CastMetrics.sheetInset)
                .padding(.bottom, CastMetrics.sheetInset)
                .animation(.standard, value: tv.cast.rows)
            }
            .scrollBounceBehavior(.basedOnSize)
        }
        .frame(width: CastMetrics.popoverWidth)
        .background(CastMetrics.sheetGround)
        .onAppear { tv.startBrowsing() }
        .onDisappear { tv.stopBrowsing() }
    }
}

/// A TV in the chooser: the screen, its name and the line under it.
struct CastChooserRow: View {
    let name: String
    let line: String
    let gone: Bool
    let action: () -> Void

    var body: some View {
        let touch = CastMetrics.touch
        Button(action: action) {
            HStack(spacing: touch ? 14 : 10) {
                Image(systemName: "tv")
                    .font(touch ? .ui(17) : .mac(13))
                    .foregroundStyle(.haze)
                    .frame(width: touch ? 22 : 18)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    Text(name).font(touch ? .ui(17) : .mac(13)).foregroundStyle(gone ? .haze : .silk)
                    if gone {
                        TVProblemLine(text: L.noticeCastNoDevice, touch: touch)
                    } else {
                        Text(line).font(touch ? .ui(13) : .mac(11)).foregroundStyle(.haze)
                    }
                }
                Spacer(minLength: 8)
            }
            .padding(.horizontal, CastMetrics.rowInset)
            .frame(minHeight: touch ? 60 : 44)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .disabled(gone)
        .accessibilityElement(children: .combine)
    }
}

// MARK: - The connected dialog

/// The cast button's dialog while a TV is held: what plays there, or that nothing does yet; the
/// TV's volume; and Stop casting.
struct CastConnectedDialog: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let cast = store.tv.cast
        let touch = CastMetrics.touch
        VStack(alignment: .leading, spacing: 0) {
            CastSheetHeader(title: L.castButtonConnected(name: cast.heldName)) { dismiss() }
            VStack(spacing: touch ? 16 : 10) {
                CastGroupBox {
                    Button {
                        guard cast.miniShown else { return }
                        cast.dialogShown = false
                        cast.expanded = true
                    } label: {
                        HStack(spacing: touch ? 12 : 10) {
                            if cast.item != nil || cast.active {
                                CastThumb(width: touch ? 40 : 32)
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(cast.showTitle).font(touch ? .ui(17) : .mac(13)).foregroundStyle(.silk).lineLimit(1)
                                    Text(cast.sessionLine).font(touch ? .ui(13) : .mac(11)).foregroundStyle(.haze).lineLimit(1)
                                }
                            } else {
                                CastButtonGlyph(look: .connected, size: touch ? 20 : 16, color: .haze)
                                    .frame(width: touch ? 22 : 18)
                                Text(L.castIdle).font(touch ? .ui(17) : .mac(13)).foregroundStyle(.haze)
                            }
                            Spacer(minLength: 0)
                        }
                        .padding(.horizontal, CastMetrics.rowInset)
                        .frame(minHeight: touch ? 52 : 40)
                        .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    CastSessionVolumeRow()
                }
                // Disconnect while nothing plays on the TV; Stop casting while something does.
                Button {
                    dismiss()
                    cast.disconnect()
                } label: {
                    Text(cast.heldIdle ? L.actionDisconnect : L.castStop)
                        .font(touch ? .ui(17, weight: .semibold) : .mac(13, weight: .semibold))
                        .foregroundStyle(.silk)
                        .frame(maxWidth: .infinity, minHeight: touch ? 50 : 30)
                        .background(Palette.raisedSelected, in: .capsule)
                        .contentShape(.capsule)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, CastMetrics.sheetInset)
            .padding(.bottom, CastMetrics.sheetInset)
        }
        .frame(width: CastMetrics.popoverWidth)
        .background(CastMetrics.sheetGround)
        .animation(.standard, value: cast.item != nil)
    }
}

// MARK: - Shared sheet parts

enum CastMetrics {
    static var touch: Bool {
        #if os(iOS)
        true
        #else
        false
        #endif
    }

    static var sheetInset: CGFloat { touch ? 16 : 10 }
    static var rowInset: CGFloat { touch ? 16 : 10 }
    /// The popover's width on iPad and the Mac; nil lets the iPhone's sheet fill.
    static var popoverWidth: CGFloat? {
        #if os(macOS)
        280
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? 380 : nil
        #endif
    }

    static var sheetGround: Color {
        #if os(macOS)
        .clear
        #else
        Palette.raised
        #endif
    }
}

/// The sheet's header: Close in a glass circle, and the title centred.
struct CastSheetHeader: View {
    let title: String
    let close: () -> Void

    var body: some View {
        #if os(iOS)
        ZStack {
            Text(title)
                .font(.ui(17, weight: .semibold))
                .foregroundStyle(.silk)
                .lineLimit(1)
                .padding(.horizontal, 60)
            HStack {
                Button(action: close) {
                    Image(systemName: "xmark")
                        .font(.ui(15, weight: .semibold))
                        .foregroundStyle(.silk)
                        .frame(width: 44, height: 44)
                        .glassEffect(.regular.interactive(), in: .circle)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L.actionClose)
                Spacer()
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 14)
        .padding(.bottom, 12)
        #else
        Text(title)
            .font(.mac(13, weight: .semibold))
            .foregroundStyle(.silk)
            .lineLimit(1)
            .padding(.horizontal, 12)
            .padding(.top, 10)
            .padding(.bottom, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
        #endif
    }
}

struct CastGroupHeading: View {
    let text: String

    var body: some View {
        Text(text)
            .font(CastMetrics.touch ? .ui(13, weight: .semibold) : .mac(11, weight: .semibold))
            .foregroundStyle(.haze)
            .padding(.leading, CastMetrics.touch ? 16 : 6)
            .padding(.top, 4)
            .accessibilityAddTraits(.isHeader)
    }
}

/// Rows in a box, with hairlines between them.
struct CastGroupBox<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        VStack(spacing: 0) {
            Group(subviews: content) { rows in
                ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
                    if index > 0 {
                        Rectangle().fill(.faintRule).frame(height: 1).padding(.leading, CastMetrics.rowInset)
                    }
                    row
                }
            }
        }
        .background(CastMetrics.touch ? Palette.raised : Palette.ground.opacity(0.6), in: .rect(cornerRadius: CastMetrics.touch ? 26 : 8))
    }
}

// MARK: - The controller's parts

/// The art of what plays: the TV's still, else the show's banner, else its cover filling the
/// frame.
struct CastArt: View {
    @Environment(AppStore.self) private var store
    @Environment(\.displayScale) private var scale
    var radius: CGFloat = 14

    var body: some View {
        let cast = store.tv.cast
        GeometryReader { proxy in
            let entry = cast.showID.flatMap { store.titles[$0] }
            ZStack {
                Rectangle().fill(entry?.plateColor ?? Palette.raised)
                if let url = cast.item?.image ?? entry?.bannerUrl {
                    ArtworkView(url: url, color: entry?.plateColor, maxPixelSize: Int(proxy.size.width * scale))
                } else if let entry {
                    CoverFill(entry: entry, sharpCover: false)
                }
            }
            .frame(width: proxy.size.width, height: proxy.size.height)
        }
        .aspectRatio(16 / 9, contentMode: .fit)
        .clipShape(.rect(cornerRadius: radius))
        .accessibilityHidden(true)
    }
}

/// The small art in the mini and the dialog.
struct CastThumb: View {
    @Environment(AppStore.self) private var store
    @Environment(\.displayScale) private var scale
    let width: CGFloat

    var body: some View {
        let cast = store.tv.cast
        let entry = cast.showID.flatMap { store.titles[$0] }
        ZStack {
            Rectangle().fill(entry?.plateColor ?? Palette.raised)
            if let url = cast.item?.image ?? entry?.bannerUrl ?? entry?.coverUrl {
                ArtworkView(url: url, color: entry?.plateColor, maxPixelSize: Int(width * 2 * scale))
            }
        }
        .frame(width: width, height: width)
        .clipShape(.rect(cornerRadius: width / 4))
        .accessibilityHidden(true)
    }
}

/// The controller's status: a spinner while it starts or joins, the water dot while it plays,
/// the vermilion dot once it failed; and the line.
struct CastSessionStatus: View {
    @Environment(AppStore.self) private var store
    let touch: Bool

    var body: some View {
        let cast = store.tv.cast
        let line = cast.sessionLine
        let shown = CastShown(cast)
        HStack(spacing: 8) {
            ZStack {
                // The mark crossfades: the one leaving fades out on 0.3 s, the new one in on 0.2 s.
                switch shown.mark {
                case .spinner:
                    ProgressView().controlSize(.mini).tint(.haze).transition(CastShown.markFade)
                case .playing:
                    Circle().fill(.water).frame(width: 7, height: 7).transition(CastShown.markFade)
                case .stopped:
                    Circle().fill(.vermilion).frame(width: 7, height: 7).transition(CastShown.markFade)
                case .glyph:
                    CastButtonGlyph(look: .connected, size: 12, color: .haze).transition(CastShown.markFade)
                }
            }
            .frame(width: 14, height: 14)
            .animation(.fadeIn, value: shown.mark)
            Text(line)
                .font(touch ? .ui(15, weight: .semibold) : .mac(12, weight: .semibold))
                .foregroundStyle(.silk)
                .lineLimit(1)
                .allowsTightening(true)
                .minimumScaleFactor(0.9)
                .monospacedDigit()
                .contentTransition(.opacity)
        }
        .animation(.standard, value: line)
        .accessibilityElement(children: .combine)
        .onChange(of: shown) { before, now in
            // Said as it happens: the connection is being made again, it joined again, or the
            // cast ended by itself.
            let joined = before == .reconnecting && now == .playing
            guard now == .reconnecting || now == .failed || joined else { return }
            AccessibilityNotification.Announcement(cast.sessionLine).post()
        }
    }
}

/// How the controller is laid out. It changes as an episode starts, as the TV waits, as the
/// connection is made again (or a session left running is joined), as the cast fails or stops,
/// and as the TV is left with nothing loaded; paused or not, playing holds. No row moves between
/// them: the controls dim in place.
enum CastShown: Equatable {
    case starting, playing, waiting, reconnecting, failed, connected

    @MainActor init(_ cast: CastModel) {
        if cast.reconnecting {
            self = .reconnecting
        } else if cast.active, cast.stage == .failed || cast.stage == .stopped {
            self = .failed
        } else if cast.heldIdle {
            self = .connected
        } else if cast.itemLive {
            self = cast.waitingShown ? .waiting : .playing
        } else {
            self = .starting
        }
    }

    /// The controls that reach the TV dim to 38% and take no input: while the TV waits, while
    /// the connection is made again, once the cast ended, and with nothing playing.
    var dims: Bool {
        switch self {
        case .waiting, .reconnecting, .failed, .connected: true
        case .starting, .playing: false
        }
    }

    /// How far the controls dim: 38% in the states above, and as before while an episode starts.
    func dimmed(starting: Double) -> Double {
        dims ? 0.38 : self == .starting ? starting : 1
    }

    enum Mark { case spinner, playing, stopped, glyph }

    /// The mark before the line: a spinner while it starts, waits or reconnects.
    var mark: Mark {
        switch self {
        case .starting, .waiting, .reconnecting: .spinner
        case .playing: .playing
        case .failed: .stopped
        case .connected: .glyph
        }
    }

    @MainActor static var markFade: AnyTransition { .asymmetric(insertion: .opacity.animation(.fadeIn), removal: .opacity.animation(.fadeOut)) }
}

/// The show's name, which opens its page, and the episode under it.
struct CastShowLines: View {
    @Environment(AppStore.self) private var store
    let touch: Bool
    let openShow: (AnilistId) -> Void

    var body: some View {
        let cast = store.tv.cast
        if cast.heldIdle {
            // Nothing plays on the held TV.
            Text(L.castIdle)
                .font(.showTitle(size: touch ? 22 : 15))
                .foregroundStyle(.haze)
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            lines(cast)
        }
    }

    private func lines(_ cast: CastModel) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Button {
                if let id = cast.showID { openShow(id) }
            } label: {
                HStack(spacing: 6) {
                    Text(cast.showTitle)
                        .font(.showTitle(size: touch ? 22 : 15))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    if cast.showID != nil {
                        Image(systemName: "chevron.right")
                            .font(touch ? .ui(15, weight: .semibold) : .mac(11, weight: .semibold))
                            .foregroundStyle(.haze)
                    }
                }
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .disabled(cast.showID == nil)
            .accessibilityLabel(L.castOpenShow(title: cast.showTitle))
            Text(cast.episodeLine)
                .font(touch ? .ui(15) : .mac(11))
                .foregroundStyle(.haze)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The TV's playhead: a capsule with the played part in silk and a thumb, dragged to seek, and
/// the times under it. A drag holds where it let go until the TV's next revision. Starting has
/// no thumb and no times; waiting and reconnecting hold the playhead, dimmed, and a cast that
/// ended keeps where it was. Waiting counts under the line from four seconds. Joined again, the
/// playhead moves on the standard spring to where the TV is, or jumps under Reduce Motion.
struct CastSessionSeek: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.castSliding) private var sliding
    let touch: Bool
    @State private var dragging: Double?
    @State private var held: Double?
    @State private var heldRev: UInt64?

    var body: some View {
        let cast = store.tv.cast
        let shown = CastShown(cast)
        TimelineView(.periodic(from: .now, by: 0.5)) { context in
            let duration = max(cast.seekDuration ?? 0, 1)
            let live = shown == .playing && cast.item?.duration != nil
            let point = shown == .starting ? nil : cast.seekPoint(at: context.date)
            let known = live || (point != nil && cast.seekDuration != nil)
            let position = live
                ? min(dragging ?? held ?? cast.itemPosition(at: context.date), duration)
                : known ? min(point ?? 0, duration) : 0
            VStack(spacing: touch ? 6 : 4) {
                GeometryReader { proxy in
                    let width = proxy.size.width
                    let x = width * position / duration
                    ZStack(alignment: .leading) {
                        Capsule().fill(.edge).frame(height: touch ? 4 : 3)
                        Capsule().fill(.silk).frame(width: max(0, x), height: touch ? 4 : 3)
                        if known {
                            Circle()
                                .fill(.silk)
                                .frame(width: dragging != nil ? 16 : 12, height: dragging != nil ? 16 : 12)
                                .offset(x: x - (dragging != nil ? 8 : 6))
                        }
                    }
                    .frame(maxHeight: .infinity)
                    .animation(reduceMotion || navigator.motionReduced ? nil : .standard, value: cast.reconnecting)
                    .contentShape(.rect)
                    .gesture(
                        DragGesture(minimumDistance: 0)
                            .onChanged { drag in
                                guard live else { return }
                                if dragging == nil { sliding?.changed(true) }
                                dragging = min(max(0, drag.location.x / max(width, 1)), 1) * duration
                            }
                            .onEnded { _ in
                                sliding?.changed(false)
                                if let to = dragging {
                                    held = to
                                    heldRev = cast.held?.rev
                                    cast.command(.seekTo(position: to))
                                }
                                dragging = nil
                            }
                    )
                }
                .frame(height: touch ? 20 : 14)
                HStack {
                    Text(ResumePoint(position: position, fraction: nil).clock)
                    Spacer()
                    Text(cast.item?.duration.map { "−" + ResumePoint(position: max(0, $0 - position), fraction: nil).clock } ?? "")
                }
                .font(touch ? .ui(12) : .mac(10))
                .monospacedDigit()
                .foregroundStyle(.haze)
                .opacity(known ? 1 : 0)
                if let waiting = cast.waitingSeconds, live || shown == .waiting {
                    Text(Msg.castPhaseWaiting(name: cast.heldName, seconds: Int64(waiting)).text)
                        .font(touch ? .ui(13) : .mac(11))
                        .foregroundStyle(.haze)
                        .monospacedDigit()
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .transition(.opacity)
                }
            }
            .accessibilityElement()
            .accessibilityLabel(L.playerPosition)
            .accessibilityValue(ResumePoint(position: position, fraction: nil).clock)
            .accessibilityAdjustableAction { direction in
                guard live else { return }
                cast.command(.seek(by: direction == .increment ? 10 : -10))
            }
        }
        .opacity(shown.dimmed(starting: 1))
        .allowsHitTesting(shown == .playing)
        // Dimmed is off too, and VoiceOver says so.
        .disabled(shown != .playing)
        .animation(.quick, value: shown.dims)
        .animation(.fadeIn, value: cast.waitingSeconds != nil)
        .onChange(of: cast.held?.rev) { _, rev in
            if held != nil, rev != heldRev { held = nil }
        }
    }
}

/// Back 10, play or pause, forward 10 and the next episode, live only while it plays. Once the
/// cast failed or stopped, the reason takes the row's place, crossfading, at the same height.
struct CastSessionTransport: View {
    @Environment(AppStore.self) private var store
    let touch: Bool

    var body: some View {
        let cast = store.tv.cast
        let shown = CastShown(cast)
        let reason = shown == .failed ? cast.reason : nil
        ZStack {
            if let reason {
                HStack(alignment: .firstTextBaseline, spacing: touch ? 8 : 6) {
                    Circle().fill(.vermilion).frame(width: touch ? 7 : 6, height: touch ? 7 : 6)
                        .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 1 }
                    Text(reason)
                        .font(touch ? .ui(15) : .mac(12))
                        .foregroundStyle(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .transition(.opacity)
            } else {
                buttons(shown).transition(.opacity)
            }
        }
        .frame(minHeight: touch ? 64 : 32)
        .animation(.standard, value: reason)
    }

    private func buttons(_ shown: CastShown) -> some View {
        let cast = store.tv.cast
        let live = shown == .playing
        let paused = cast.itemPaused
        return HStack(spacing: touch ? 28 : 14) {
            glyph("gobackward.10", L.playerBack10, size: touch ? 24 : 15) { cast.command(.seek(by: -10)) }
            glyph(paused ? "play.fill" : "pause.fill", paused ? L.actionPlay : L.actionPause, size: touch ? 36 : 22) {
                cast.command(.togglePause)
            }
            .contentTransition(.symbolEffect(.replace))
            glyph("goforward.10", L.playerForward10, size: touch ? 24 : 15) { cast.command(.seek(by: 10)) }
            glyph("forward.end.fill", L.playerNextEpisode, size: touch ? 18 : 13) { cast.command(.next) }
        }
        .frame(maxWidth: .infinity)
        .disabled(!live)
        .opacity(shown.dimmed(starting: 0.4))
        .animation(.quick, value: shown)
    }

    private func glyph(_ symbol: String, _ label: String, size: CGFloat, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: size, weight: .medium))
                .foregroundStyle(.silk)
                .frame(width: touch ? 56 : 32, height: touch ? 56 : 32)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerLift()
        .accessibilityLabel(label)
    }
}

/// Subtitles, Audio and the TV's volume. The rows are there from the start, so nothing below
/// them moves as the TV reports; they dim while it starts, waits, reconnects and once it ended.
struct CastSessionRows: View {
    @Environment(AppStore.self) private var store
    let touch: Bool

    var body: some View {
        let cast = store.tv.cast
        let shown = CastShown(cast)
        let live = shown == .playing
        let now = cast.now
        CastGroupBox {
            row(L.playerSubtitles) {
                TVTrackMenu(title: L.playerSubtitles, kind: .subtitle, tracks: now?.subtitles ?? [], off: now?.subtitlesOff ?? false, phone: touch)
            }
            row(L.playerAudio) {
                TVTrackMenu(title: L.playerAudio, kind: .audio, tracks: now?.audio ?? [], off: false, phone: touch)
            }
            CastSessionVolumeRow()
        }
        .disabled(!live)
        .opacity(shown.dimmed(starting: 0.5))
        .animation(.quick, value: shown)
    }

    private func row(_ title: String, @ViewBuilder value: () -> some View) -> some View {
        HStack {
            Text(title).font(touch ? .ui(17) : .mac(12)).foregroundStyle(.silk)
            Spacer(minLength: 8)
            value()
        }
        .padding(.horizontal, CastMetrics.rowInset)
        .frame(minHeight: touch ? 52 : 32)
    }
}

/// The TV's volume: a slider for a TV that takes a level, with Mute where a mute reaches its
/// sound; Set on the TV alone for one that keeps its own.
struct CastSessionVolumeRow: View {
    @Environment(AppStore.self) private var store
    @Environment(\.castSliding) private var sliding
    @State private var dragging: Double?

    var body: some View {
        let cast = store.tv.cast
        let touch = CastMetrics.touch
        let canMute = cast.held?.canMute ?? cast.canMute
        let muted = canMute && (cast.held?.muted ?? cast.muted)
        HStack(spacing: 10) {
            Text(L.tvVolume).font(touch ? .ui(17) : .mac(12)).foregroundStyle(.silk)
            Spacer(minLength: 8)
            if cast.held?.volumeControl == .level, let level = cast.held?.level {
                Slider(value: Binding(
                    get: { dragging ?? level },
                    set: { dragging = $0 }
                ), in: 0...1) { editing in
                    sliding?.changed(editing)
                    if !editing, let to = dragging {
                        cast.command(.volume(step: ((to - level) * 100).rounded()))
                        dragging = nil
                    }
                }
                .tint(.silk)
                .controlSize(touch ? .regular : .small)
                .frame(maxWidth: touch ? 160 : 120)
                .accessibilityLabel(L.tvVolume)
            } else {
                Text(muted ? L.playerMuted : L.castVolumeFixed)
                    .font(touch ? .ui(15) : .mac(11))
                    .foregroundStyle(.haze)
                    .contentTransition(.opacity)
            }
            // Mute or Unmute keeps its whole label: the line or the slider beside it gives way.
            if canMute {
                CastMuteButton(muted: muted, touch: touch) { cast.command(.mute(muted: !muted)) }
                    .layoutPriority(1)
            }
        }
        .padding(.horizontal, CastMetrics.rowInset)
        .frame(minHeight: touch ? 52 : 32)
        .animation(.standard, value: muted)
    }
}

/// The TV's Mute, or Unmute while it is muted, as a pill with its glyph.
struct CastMuteButton: View {
    let muted: Bool
    let touch: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Label(muted ? L.playerUnmute : L.playerMute, systemImage: muted ? "speaker.wave.2" : "speaker.slash")
                .labelStyle(.titleAndIcon)
        }
        .buttonStyle(SecondaryCapsuleStyle(height: touch ? 32 : 22, fontSize: touch ? 15 : 11))
    }
}

/// The buttons at the foot: Play here, Stop casting and More while it runs, with Play here dimmed
/// while the connection is made again; once a play failed or the cast stopped, what fixes it,
/// the first in amber; with nothing playing on the held TV, Choose another TV and Disconnect.
struct CastSessionFooter: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let touch: Bool
    let close: () -> Void

    var body: some View {
        let cast = store.tv.cast
        let shown = CastShown(cast)
        let failed = shown == .failed
        let idle = shown == .connected
        let reconnecting = shown == .reconnecting
        HStack(spacing: touch ? 10 : 6) {
            if failed {
                ForEach(Array(cast.actions.prefix(2).enumerated()), id: \.offset) { index, action in
                    pill(title(action), primary: index == 0) { run(action) }
                        .transition(.opacity)
                }
            } else if idle {
                pill(L.tvChooseAnother, primary: false) { run(.chooseAnotherTV) }
                pill(L.actionDisconnect, primary: false) {
                    close()
                    cast.disconnect()
                }
            } else {
                pill(L.castPlayHere, primary: false) {
                    close()
                    cast.playHereFromSession()
                }
                .disabled(reconnecting)
                .opacity(reconnecting ? 0.38 : 1)
                pill(L.castStop, primary: false) {
                    close()
                    cast.disconnect()
                }
            }
            Menu {
                Button(L.tvChooseAnother) { store.tv.chooseAnotherTV() }
                if idle {
                    Button(L.actionDisconnect) {
                        close()
                        cast.disconnect()
                    }
                } else if !failed {
                    Button(L.castStop, role: .destructive) {
                        close()
                        cast.disconnect()
                    }
                }
            } label: {
                Image(systemName: "ellipsis")
                    .font(touch ? .ui(17, weight: .semibold) : .mac(12, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(width: touch ? 44 : 26, height: touch ? 44 : 26)
                    .background(touch ? Palette.raised : Palette.raisedSelected, in: .circle)
                    .contentShape(.circle)
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .fixedSize()
            .accessibilityLabel(L.actionMore)
            .disabled(reconnecting)
            .opacity(reconnecting ? 0.38 : 1)
        }
        .animation(.quick, value: shown)
    }

    private func pill(_ title: String, primary: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(touch ? .ui(17, weight: .semibold) : .mac(12, weight: .semibold))
                .foregroundStyle(primary ? Palette.ground : .silk)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
                .frame(maxWidth: .infinity, minHeight: touch ? 44 : 26)
                .background(primary ? AnyShapeStyle(.amber) : AnyShapeStyle(touch ? Palette.raised : Palette.raisedSelected), in: .capsule)
                .contentShape(.capsule)
        }
        .buttonStyle(.plain)
    }

    private func title(_ action: CastAction) -> String {
        switch action {
        case .chooseSource: L.playerChooseSource
        case .playHere: L.castPlayHere
        case .tryAgain: L.actionTryAgain
        case .chooseAnotherTV: L.tvChooseAnother
        }
    }

    private func run(_ action: CastAction) {
        let cast = store.tv.cast
        switch action {
        case .chooseSource:
            let id = cast.showID
            close()
            guard let id else { return }
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(400))
                navigator.sourcePicker = id
            }
        case .playHere:
            close()
            cast.playHereFromSession()
        case .tryAgain:
            cast.tryAgain()
        case .chooseAnotherTV:
            // The chooser carries the episode on from where the TV is, and closes the controller.
            store.tv.chooseAnotherTV()
        }
    }
}

/// The title page's line under its buttons while a TV is held: what is played goes there.
struct CastPlaysOnLine: View {
    @Environment(AppStore.self) private var store
    let touch: Bool

    var body: some View {
        let cast = store.tv.cast
        if cast.connected {
            HStack(spacing: 8) {
                Circle().fill(.water).frame(width: 6, height: 6)
                Text(L.castPlaysOn(name: cast.heldName))
                    .font(touch ? .ui(15) : .mac(12))
                    .foregroundStyle(.haze)
            }
            .accessibilityElement(children: .combine)
            .transition(.opacity)
        }
    }
}

/// The cast button as a toolbar item: none at all while no TV is found, so the bar keeps no
/// empty place for it.
struct CastToolbarItem: ToolbarContent {
    @Environment(AppStore.self) private var store

    var body: some ToolbarContent {
        if store.tv.castButton != .hidden {
            #if os(macOS)
            // One width in the bar and in the overflow menu, as every item in the Mac's toolbar
            // is (`toolbarItemWidth`).
            ToolbarItem(placement: .primaryAction) { CastButton().toolbarItemWidth(store.tv.castButton) }
            #else
            ToolbarItem(placement: .topBarTrailing) { CastButton() }
            #endif
        }
    }
}

/// Told as a slider in the cast controller starts and stops being dragged: the drawer around it
/// leaves that drag alone.
struct CastSliding {
    let changed: @MainActor (Bool) -> Void
}

extension EnvironmentValues {
    @Entry var castSliding: CastSliding?
}
