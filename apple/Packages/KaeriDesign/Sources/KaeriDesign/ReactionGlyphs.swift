// The four reactions of a watch party, as one drawn set shared by every platform: monochrome,
// stroked at the platform's own weight, on a 24 unit grid. The system's symbols have no clap or
// crying face to match, so none of the set comes from them.

import SwiftUI

public enum ReactionGlyphKind: CaseIterable, Sendable {
    case laugh, gasp, clap, cry
}

/// The outlines of a glyph: stroked.
struct ReactionOutline: Shape {
    let kind: ReactionGlyphKind

    func path(in rect: CGRect) -> Path {
        let k = rect.width / 24
        func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: rect.minX + x * k, y: rect.minY + y * k) }
        var path = Path()
        func circle(_ x: CGFloat, _ y: CGFloat, _ r: CGFloat) {
            path.addEllipse(in: CGRect(x: rect.minX + (x - r) * k, y: rect.minY + (y - r) * k, width: 2 * r * k, height: 2 * r * k))
        }
        switch kind {
        case .laugh:
            circle(12, 12, 9)
            path.move(to: p(8, 10.3)); path.addQuadCurve(to: p(11, 10.3), control: p(9.5, 8.4))
            path.move(to: p(13, 10.3)); path.addQuadCurve(to: p(16, 10.3), control: p(14.5, 8.4))
            // The open mouth: a flat top over a half circle.
            path.move(to: p(7.6, 13.4)); path.addLine(to: p(16.4, 13.4))
            path.addArc(center: p(12, 13.4), radius: 4.4 * k, startAngle: .degrees(0), endAngle: .degrees(180), clockwise: false)
            path.closeSubpath()
        case .gasp:
            circle(12, 12, 9)
            circle(12, 15.3, 2.2)
        case .clap:
            for (x, angle) in [(CGFloat(5.6), -16.0), (12.6, 16.0)] {
                let pivot = p(x + 2.9, 14.6)
                let box = CGRect(x: rect.minX + x * k, y: rect.minY + 8.6 * k, width: 5.8 * k, height: 12 * k)
                let rounded = Path(roundedRect: box, cornerRadius: 2.9 * k)
                let turned = CGAffineTransform(translationX: pivot.x, y: pivot.y)
                    .rotated(by: angle * .pi / 180)
                    .translatedBy(x: -pivot.x, y: -pivot.y)
                path.addPath(rounded, transform: turned)
            }
            path.move(to: p(12, 2.6)); path.addLine(to: p(12, 4.9))
            path.move(to: p(7.4, 3.8)); path.addLine(to: p(8.6, 5.7))
            path.move(to: p(16.6, 3.8)); path.addLine(to: p(15.4, 5.7))
        case .cry:
            circle(12, 12, 9)
            path.move(to: p(8, 10)); path.addLine(to: p(10.5, 10))
            path.move(to: p(13.5, 10)); path.addLine(to: p(16, 10))
            path.move(to: p(9, 16.6)); path.addQuadCurve(to: p(15, 16.6), control: p(12, 14.3))
        }
        return path
    }
}

/// The parts of a glyph that are solid: eyes and a tear.
struct ReactionSolids: Shape {
    let kind: ReactionGlyphKind

    func path(in rect: CGRect) -> Path {
        let k = rect.width / 24
        func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: rect.minX + x * k, y: rect.minY + y * k) }
        var path = Path()
        switch kind {
        case .gasp:
            for x in [CGFloat(9), 15] {
                path.addEllipse(in: CGRect(x: rect.minX + (x - 1) * k, y: rect.minY + 9 * k, width: 2 * k, height: 2 * k))
            }
        case .cry:
            path.move(to: p(16.2, 11.7))
            path.addCurve(to: p(15, 14.3), control1: p(16.2, 11.7), control2: p(15, 13.5))
            path.addCurve(to: p(17.4, 14.3), control1: p(15, 15.9), control2: p(17.4, 15.9))
            path.addCurve(to: p(16.2, 11.7), control1: p(17.4, 13.5), control2: p(16.2, 11.7))
            path.closeSubpath()
        case .laugh, .clap:
            break
        }
        return path
    }
}

/// A reaction glyph at `size`, in the colour of the text around it, stroked at `weight` of 24.
public struct ReactionGlyphView: View {
    let kind: ReactionGlyphKind
    let size: CGFloat
    let weight: CGFloat

    public init(_ kind: ReactionGlyphKind, size: CGFloat = 20, weight: CGFloat = 1.6) {
        self.kind = kind
        self.size = size
        self.weight = weight
    }

    public var body: some View {
        ZStack {
            ReactionOutline(kind: kind)
                .stroke(style: StrokeStyle(lineWidth: weight * size / 24, lineCap: .round, lineJoin: .round))
            ReactionSolids(kind: kind)
                .fill()
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}
