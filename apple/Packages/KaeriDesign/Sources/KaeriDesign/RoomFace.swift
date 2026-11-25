// A person's face in a room and the room's capsule buttons (the room board, v5), shared by the
// player's room and the Watch together page.

import SwiftUI

/// A face's ring (the room board): water with the room, haze paused, a silk arc filling toward
/// the 20 s cap while the room waits for them, or none.
public enum FaceRing: Equatable, Sendable {
    case with
    case paused
    case waiting(Double)
    case none
}

/// A person's face of `disc` points: their initial on a disc, with its ring around it, the pause
/// badge and the error dot.
public struct PersonFace: View {
    let name: String
    let ring: FaceRing
    let dim: Double
    let pauseBadge: Bool
    let error: Bool
    let disc: CGFloat

    public init(name: String, ring: FaceRing, dim: Double = 1, pauseBadge: Bool = false, error: Bool = false, disc: CGFloat) {
        self.name = name
        self.ring = ring
        self.dim = dim
        self.pauseBadge = pauseBadge
        self.error = error
        self.disc = disc
    }

    private var stroke: CGFloat { disc >= 48 ? 3 : disc >= 30 ? 2 : 1.75 }
    private var gap: CGFloat { disc >= 48 ? 3 : 2 }
    var size: CGFloat { disc + 2 * (stroke + gap) }

    public var body: some View {
        ZStack {
            ringView
            Text(String(name.prefix(1)))
                .font(.system(size: (disc * 0.42).rounded(), weight: .semibold))
                .foregroundStyle(.silk)
                .frame(width: disc, height: disc)
                .background(Palette.raisedSelected, in: .circle)
                .opacity(dim)
                .animation(.fadeIn, value: dim)
        }
        .frame(width: size, height: size)
        .overlay(alignment: .bottomTrailing) {
            if pauseBadge {
                let badge = max(12, (disc * 0.42).rounded())
                Image(systemName: "pause.fill")
                    .font(.system(size: badge * 0.5, weight: .bold))
                    .foregroundStyle(.silk)
                    .frame(width: badge, height: badge)
                    .background(Palette.ground, in: .circle)
                    .offset(x: 1, y: 1)
            }
        }
        .overlay(alignment: .bottomTrailing) {
            if error {
                let dot = max(8, (disc * 0.28).rounded())
                Circle()
                    .fill(Palette.vermilion)
                    .frame(width: dot, height: dot)
                    .overlay(Circle().strokeBorder(Palette.ground, lineWidth: 2).padding(-2))
                    .padding(stroke)
            }
        }
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private var ringView: some View {
        let inset = stroke / 2
        switch ring {
        case .with:
            Circle().inset(by: inset).stroke(Palette.water, lineWidth: stroke)
        case .paused:
            Circle().inset(by: inset).stroke(Palette.haze, lineWidth: stroke)
        case .waiting(let part):
            ZStack {
                Circle().inset(by: inset).stroke(Palette.rule, lineWidth: stroke)
                Circle().inset(by: inset)
                    .trim(from: 0, to: part)
                    .stroke(Palette.silk, style: StrokeStyle(lineWidth: stroke, lineCap: .round))
                    .rotationEffect(.degrees(-90))
                    .animation(.linear(duration: 1), value: part)
            }
        case .none:
            EmptyView()
        }
    }
}

/// The "+3" face that opens the full list.
public struct RoomMoreFace: View {
    let label: String
    let disc: CGFloat

    public init(label: String, disc: CGFloat) {
        self.label = label
        self.disc = disc
    }

    public var body: some View {
        let stroke: CGFloat = disc >= 48 ? 3 : disc >= 30 ? 2 : 1.75
        let gap: CGFloat = disc >= 48 ? 3 : 2
        Text(label)
            .font(.system(size: (disc * 0.36).rounded(), weight: .semibold))
            .foregroundStyle(.haze)
            .frame(width: disc, height: disc)
            .background(Palette.raised, in: .circle)
            .overlay(Circle().strokeBorder(Palette.rule, lineWidth: 1))
            .frame(width: disc + 2 * (stroke + gap), height: disc + 2 * (stroke + gap))
            .accessibilityHidden(true)
    }
}

/// The room's buttons: amber for the one thing to press, quiet glass for the rest.
public struct RoomButtonStyle: ButtonStyle {
    public enum Kind { case primary, secondary, glass }
    var kind: Kind = .secondary
    var height: CGFloat = 32
    var size: CGFloat = 13
    var padding: CGFloat = 14
    var wide = false
    @Environment(\.isEnabled) private var enabled

    public init(kind: Kind = .secondary, height: CGFloat = 32, size: CGFloat = 13, padding: CGFloat = 14, wide: Bool = false) {
        self.kind = kind
        self.height = height
        self.size = size
        self.padding = padding
        self.wide = wide
    }

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: size, weight: .semibold))
            .lineLimit(1)
            .padding(.horizontal, padding)
            .frame(maxWidth: wide ? .infinity : nil)
            .frame(height: height)
            .foregroundStyle(kind == .primary ? Palette.ground : Palette.silk)
            .background {
                switch kind {
                case .primary: Capsule().fill(Palette.amber)
                case .secondary: Capsule().fill(Palette.silk.opacity(0.12)).overlay(Capsule().strokeBorder(Palette.glassEdge, lineWidth: 1))
                case .glass: Capsule().fill(Palette.glass).overlay(Capsule().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
                }
            }
            .opacity(configuration.isPressed ? 0.8 : enabled ? 1 : 0.5)
            .contentShape(.capsule)
    }
}
