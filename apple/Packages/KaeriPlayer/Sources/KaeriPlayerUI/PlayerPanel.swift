// The player's Episodes and Sources panel on Mac, iPad and iPhone (player panel board v3): a
// Liquid Glass side panel over the dimmed video, floating in from the trailing edge, with two
// tabs. Episodes shows what the title page knows of each episode and reveals one row's synopsis
// and actions; Sources says what plays now, folds in the sub and dub switch and lists every other
// source in the order the viewer set, with its releases. A pick plays from the playhead, for this
// episode only.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The two tabs. The panel opens on Episodes.
public enum PanelTab: Hashable, Sendable {
    case episodes, sources
}

/// The board's sizes per platform, in points.
struct PanelMetrics {
    var touch: Bool
    /// The pane's width, inside its inset from the window's edges.
    var width: CGFloat
    /// How far the pane floats in from the top, trailing and bottom edges.
    var inset: CGFloat
    /// The pane's corner radius, concentric with the window's or the display's.
    var radius: CGFloat
    /// Room above the header (the iPad's and iPhone's), and inside the trailing edge (the
    /// iPhone's, clear of the display's curve).
    var top: CGFloat = 0
    var trailing: CGFloat = 0
    /// The list's side padding and the header's.
    var pad: CGFloat
    var head: CGFloat
    var close: CGFloat
    var tabWidth: CGFloat?
    var still: CGSize
    var stillRadius: CGFloat
    var title: CGFloat
    var titleLine: CGFloat
    var detail: CGFloat
    var detailLine: CGFloat
    var synopsis: CGFloat
    var synopsisLine: CGFloat
    var section: CGFloat
    var rowPad: CGFloat
    var rowRadius: CGFloat
    var gap: CGFloat
    /// A Sources row's least height.
    var choice: CGFloat
    /// The check column's width on a Sources row.
    var check: CGFloat
    /// Space between a revealed row's head and its synopsis, and between the synopsis and the
    /// buttons.
    var reveal: CGFloat

    static let mac = PanelMetrics(
        touch: false, width: 380, inset: 8, radius: 12, pad: 16, head: 52, close: 28, tabWidth: 200,
        still: CGSize(width: 96, height: 54), stillRadius: 6, title: 13, titleLine: 18, detail: 12, detailLine: 16,
        synopsis: 12, synopsisLine: 17, section: 12, rowPad: 8, rowRadius: 8, gap: 12, choice: 40, check: 18, reveal: 10)

    static let pad = PanelMetrics(
        touch: true, width: 400, inset: 10, radius: 20, top: 24, pad: 20, head: 64, close: 36,
        still: CGSize(width: 112, height: 63), stillRadius: 8, title: 15, titleLine: 20, detail: 13, detailLine: 18,
        synopsis: 13, synopsisLine: 19, section: 13, rowPad: 10, rowRadius: 12, gap: 14, choice: 52, check: 24, reveal: 12)

    /// The iPhone, always sideways in the player: the iPad's rows, corners concentric with the
    /// display's, and the trailing padding clear of its curve.
    static let phone = PanelMetrics(
        touch: true, width: 400, inset: 10, radius: 45, top: 10, trailing: 14, pad: 16, head: 58, close: 30,
        still: CGSize(width: 112, height: 63), stillRadius: 8, title: 15, titleLine: 20, detail: 13, detailLine: 18,
        synopsis: 13, synopsisLine: 19, section: 13, rowPad: 10, rowRadius: 12, gap: 14, choice: 52, check: 24, reveal: 12)

    /// The pane's width with its inset: what slides in from the edge.
    var outer: CGFloat { width + inset }
}

// MARK: The layer

/// The dim over the video and the panel at the trailing edge. A click or tap on the dim closes
/// it; on touch so does a swipe toward the edge.
struct PlayerPanelLayer: View {
    let model: PlayerModel
    let metrics: PanelMetrics
    let open: Bool
    @Binding var tab: PanelTab
    let close: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack(alignment: .trailing) {
            if open {
                Palette.controlsDim
                    .contentShape(.rect)
                    .onTapGesture(perform: close)
                    .accessibilityHidden(true)
                    .transition(.opacity)
                PlayerPanel(model: model, metrics: metrics, tab: $tab, close: close)
                    .padding(metrics.inset)
                    .frame(maxHeight: .infinity)
                    .transition(reduceMotion ? .opacity : .move(edge: .trailing))
                    #if os(iOS)
                    .simultaneousGesture(DragGesture(minimumDistance: 24).onEnded { value in
                        if value.translation.width > 60, abs(value.translation.width) > abs(value.translation.height) { close() }
                    })
                    #endif
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .trailing)
        // In on the standard spring, out on the quick one; under Reduce Motion a fade each way.
        .animation(open ? (reduceMotion ? .fadeIn : .standard) : (reduceMotion ? .fadeOut : .quick), value: open)
        .onChange(of: open, initial: true) { _, now in
            model.panelOpen = now
            if now { model.loadPanelReleases() }
            if now, let shot = model.panelShot { tab = shot.tab }
        }
        .onChange(of: model.panelShot) { _, shot in
            if let shot { tab = shot.tab }
        }
    }
}

// MARK: The panel

struct PlayerPanel: View {
    let model: PlayerModel
    let metrics: PanelMetrics
    @Binding var tab: PanelTab
    let close: () -> Void
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency

    var body: some View {
        ZStack(alignment: .top) {
            PanelEpisodes(model: model, metrics: metrics, close: close)
                .opacity(tab == .episodes ? 1 : 0)
                .allowsHitTesting(tab == .episodes)
                .accessibilityHidden(tab != .episodes)
            PanelSources(model: model, metrics: metrics)
                .opacity(tab == .sources ? 1 : 0)
                .allowsHitTesting(tab == .sources)
                .accessibilityHidden(tab != .sources)
        }
        // Old and new together, never through empty.
        .animation(.standard, value: tab)
        // The rows scroll on under the header, softened by the system's edge effect as they go.
        .safeAreaBar(edge: .top, spacing: 0) { header }
        .scrollEdgeEffectStyle(.soft, for: .top)
        .padding(.top, metrics.top)
        .padding(.trailing, metrics.trailing)
        .frame(width: metrics.width)
        .frame(maxHeight: .infinity)
        .foregroundStyle(.silk)
        .panelGlass(radius: metrics.radius, reduceTransparency: reduceTransparency)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(L.playerPanel)
        .accessibilityAddTraits(.isModal)
        .accessibilityAction(.escape, close)
        #if os(macOS)
        .takesScrollWheel()
        #endif
    }

    private var header: some View {
        HStack(spacing: 12) {
            Picker(L.playerPanel, selection: $tab) {
                Text(L.playerEpisodes).tag(PanelTab.episodes)
                Text(L.playerSources).tag(PanelTab.sources)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .controlSize(metrics.touch ? .large : .regular)
            // The segments share the board's width on the Mac too, as they do on touch.
            .buttonSizing(.flexible)
            .frame(width: metrics.tabWidth)
            .frame(maxWidth: metrics.tabWidth == nil ? .infinity : nil)
            if metrics.tabWidth != nil { Spacer(minLength: 0) }
            Button(action: close) {
                Image(systemName: "xmark")
                    .font(.system(size: metrics.touch ? 14 : 12, weight: .semibold))
                    .foregroundStyle(.haze)
                    .frame(width: metrics.close, height: metrics.close)
                    .contentShape(.circle)
            }
            .buttonStyle(.plain)
            .glassEffect(.regular.interactive(), in: .circle)
            .pointingHand()
            .help(L.actionClose)
            .accessibilityLabel(L.actionClose)
            .keyboardShortcut(.cancelAction)
        }
        .padding(.horizontal, metrics.pad)
        .frame(height: metrics.head)
    }
}

extension View {
    /// Liquid Glass over the dimmed video: the system's glass, tinted the player's ground at 60%
    /// so silk text holds over a bright frame, or solid Raised under Reduce Transparency.
    @ViewBuilder
    func panelGlass(radius: CGFloat, reduceTransparency: Bool) -> some View {
        let shape = RoundedRectangle(cornerRadius: radius, style: .continuous)
        if reduceTransparency {
            background(Palette.raised, in: shape).clipShape(shape)
        } else {
            // The system's tint alone stays too clear over a bright frame: the board's 60% ground
            // is laid inside the glass as well, under the content.
            background(Palette.ground.opacity(0.5), in: shape)
                .clipShape(shape)
                .glassEffect(.regular.tint(Palette.ground.opacity(0.6)), in: shape)
        }
    }
}

// MARK: The held picture

/// The picture on screen when a switch began to load, over the video until the new release's
/// first frame, then crossfaded away as the new picture comes up under it.
struct HeldPictureView: View {
    let model: PlayerModel

    var body: some View {
        ZStack {
            if let held = model.heldPicture {
                Image(decorative: held.image, scale: 1)
                    .resizable()
                    .scaledToFit()
                    .opacity(model.holding ? 1 : 0)
                    .animation(.fadeIn, value: model.holding)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

// MARK: Shared parts

/// The panel's two buttons: the amber one for playing, the glass one for the rest.
struct PanelButton: View {
    enum Kind { case primary, secondary }
    let title: String
    let kind: Kind
    let metrics: PanelMetrics
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .font(.system(size: metrics.touch ? 15 : 13, weight: kind == .primary ? .semibold : .medium))
                .lineLimit(1)
                .padding(.horizontal, metrics.touch ? 18 : 14)
                .frame(height: metrics.touch ? 36 : 28)
                .foregroundStyle(kind == .primary ? Palette.ground : Palette.silk)
                // The Mac's glass tint alone washes amber out over the dim: the fill is laid inside.
                .background(kind == .primary ? Palette.amber : .clear, in: .capsule)
                .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .glassEffect(kind == .primary ? .regular.tint(Palette.amber).interactive() : .regular.interactive(), in: .capsule)
        .pointingHand()
    }
}

/// A group's name over its rows: "Playing now", "Other sources".
struct PanelHeading: View {
    let title: String
    let metrics: PanelMetrics

    var body: some View {
        Text(title)
            .font(.system(size: metrics.section, weight: .semibold))
            .foregroundStyle(.haze)
            .padding(.horizontal, metrics.rowPad)
            .padding(.top, 10)
            .padding(.bottom, 4)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityAddTraits(.isHeader)
    }
}

/// Text at the board's line height: SwiftUI's own leading for the font, plus the rest between
/// lines.
struct PanelText: View {
    let text: String
    let size: CGFloat
    let line: CGFloat
    var weight: Font.Weight = .regular
    var color: Color = Palette.silk
    var lines: Int? = nil

    var body: some View {
        Text(text)
            .font(.system(size: size, weight: weight))
            .lineSpacing(max(0, line - size * 1.2))
            .foregroundStyle(color)
            .lineLimit(lines)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}
