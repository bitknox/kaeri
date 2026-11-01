// The subtitle preview: a 16:9 frame, bright sky on its left half and night on its right, so poor
// contrast shows on either, with two lines of dialogue at the bottom and, in Styled, a typeset sign
// at the top. It redraws on every change at once, with no animation, and draws what mpv will:
// size, font, colour, edge, the plate behind the text, Raise, and what Styled subtitles keeps.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// Plain subtitles (SRT, WebVTT) or styled ones (ASS).
enum SubtitleKind: Hashable {
    case plain, styled
}

struct SubtitlePreview: View {
    let style: SubtitleStyle
    var kind: SubtitleKind
    /// The system's caption style, when the style follows it.
    var system: SystemCaptionStyle?
    /// The frame's width; its height is 9/16 of it.
    var width: CGFloat

    private var height: CGFloat { width * 9 / 16 }
    /// Points per mpv pixel at this width: the frame stands for a 720-line picture.
    private var unit: CGFloat { height / 720 }

    var body: some View {
        ZStack {
            HStack(spacing: 0) {
                Color(red: 0.62, green: 0.80, blue: 0.91)
                Color(red: 0.07, green: 0.08, blue: 0.16)
            }
            if kind == .styled, style.styled != .strip {
                sign
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                    .padding(.top, height * 0.08)
            }
            dialogue
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
                .padding(.bottom, height * (0.06 + CGFloat(style.raise) / 100))
        }
        .frame(width: width, height: height)
        .clipShape(.rect(cornerRadius: 6))
        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Palette.glassRule, lineWidth: 1))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L.settingsSubsPreview)
        .accessibilityValue(L.settingsSubsPreviewLine + " " + L.settingsSubsPreviewLine2)
    }

    // MARK: What is drawn

    /// The authors' own look for styled subtitles under Keep their look.
    private var keeps: Bool { kind == .styled && style.styled == .keep }

    private var lines: [String] {
        var lines = [L.settingsSubsPreviewLine, L.settingsSubsPreviewLine2]
        // Remove all styling: the sign becomes a line of plain dialogue.
        if kind == .styled, style.styled == .strip { lines.insert(L.settingsSubsPreviewSign, at: 0) }
        return lines
    }

    private var dialogue: some View {
        let look = Look(style: style, system: system, authors: keeps)
        let size = 38 * look.scale * unit
        return VStack(spacing: size * 0.12) {
            if look.plate == .allLines {
                VStack(spacing: size * 0.12) { ForEach(lines, id: \.self) { line($0, look, size) } }
                    .padding(.horizontal, size * 0.5)
                    .padding(.vertical, size * 0.25)
                    .background(look.plateColour, in: .rect)
            } else {
                ForEach(lines, id: \.self) { line($0, look, size) }
            }
        }
        .frame(maxWidth: width * 0.9)
    }

    private func line(_ text: String, _ look: Look, _ size: CGFloat) -> some View {
        Text(text)
            .font(look.font(size: size))
            .foregroundStyle(look.text)
            .multilineTextAlignment(.center)
            .lineLimit(2)
            .minimumScaleFactor(0.5)
            .modifier(Edge(look: look, unit: unit))
            .padding(.horizontal, look.plate == .eachLine ? size * 0.4 : 0)
            .background(look.plate == .eachLine ? look.plateColour : .clear, in: .rect)
    }

    /// A typeset sign: its authors' look under Keep, the viewer's own under Restyle.
    private var sign: some View {
        let restyled = style.styled == .restyle
        let look = Look(style: style, system: system, authors: !restyled)
        let size = 38 * look.scale * unit
        return Text(L.settingsSubsPreviewSign)
            .font(restyled ? look.font(size: size) : .system(size: size * 1.3, weight: .heavy, design: .rounded).italic())
            .foregroundStyle(restyled ? look.text : Color(red: 1, green: 0.55, blue: 0.62))
            .modifier(Edge(look: look, unit: unit))
            .padding(.horizontal, look.plate == .eachLine && restyled ? size * 0.4 : 0)
            .background(look.plate == .eachLine && restyled ? look.plateColour : .clear, in: .rect)
    }

    // MARK: The look

    enum Plate { case none, eachLine, allLines }

    /// The style resolved: the viewer's choices, with the system's over them while it is followed.
    struct Look {
        var scale: CGFloat
        var text: Color
        var fontChoice: SubtitleFont
        var systemFamily: String?
        var outline: CGFloat = 0
        var shadow: CGFloat = 0
        var shadowColour: Color = .black.opacity(0.7)
        var plate: Plate = .none
        var plateColour: Color = .black.opacity(0.75)
        var authors: Bool

        init(style: SubtitleStyle, system: SystemCaptionStyle?, authors: Bool) {
            self.authors = authors
            scale = CGFloat(style.size) / 100 * CGFloat(system?.relativeSize ?? 1)
            fontChoice = style.font
            systemFamily = system?.fontFamily
            let thin = style.thickness == .thin, thick = style.thickness == .thick
            switch style.edge {
            case .outline: outline = thin ? 1 : thick ? 2.5 : 1.65
            case .dropShadow: shadow = thin ? 1 : thick ? 3 : 2
            case .noEdge: break
            }
            let plateAlpha = Double(style.opacity) / 100
            switch style.background {
            case .noBackground: break
            case .eachLine: plate = .eachLine; plateColour = .black.opacity(plateAlpha); outline = 0; shadow = 0
            case .allLines: plate = .allLines; plateColour = .black.opacity(plateAlpha)
            }
            switch style.colour {
            case .white: text = .white
            case .yellow: text = Color(red: 1, green: 1, blue: 0)
            case .cyan: text = Color(red: 0, green: 1, blue: 1)
            case .green: text = Color(red: 0, green: 1, blue: 0)
            }
            if authors { text = Color(red: 1, green: 0.93, blue: 0.7); outline = 1.65; shadow = 0; plate = .none }
            guard let system else { return }
            if let rgba = system.textRgba { text = Color(rgba: rgba) }
            if let edge = system.edge {
                outline = 0
                shadow = 0
                switch edge {
                case .plain: break
                case .uniform: outline = 1.65
                case .dropShadow: shadow = 2
                case .raised, .depressed: shadow = 1
                }
            }
            if let background = system.backgroundRgba, background & 0xFF > 0 {
                plate = .eachLine
                plateColour = Color(rgba: background)
                outline = 0
                shadow = 0
            } else if let window = system.windowRgba, window & 0xFF > 0 {
                plate = .allLines
                plateColour = Color(rgba: window)
            } else if system.backgroundRgba != nil || system.windowRgba != nil {
                plate = .none
            }
        }

        func font(size: CGFloat) -> Font {
            if authors { return .system(size: size, weight: .semibold, design: .serif) }
            if let systemFamily { return .custom(systemFamily, size: size) }
            switch fontChoice {
            case .default: return .system(size: size, weight: .medium)
            case .zenKakuGothicNew: return .custom(ShowTitleFace.postScriptName, size: size)
            case .serif: return .system(size: size, weight: .medium, design: .serif)
            case .monospace: return .system(size: size, weight: .medium, design: .monospaced)
            }
        }
    }

    /// The edge: an outline all round, or a shadow below and to the right.
    struct Edge: ViewModifier {
        let look: Look
        let unit: CGFloat

        func body(content: Content) -> some View {
            let o = look.outline * unit * 1.3
            content
                .shadow(color: .black, radius: 0, x: o, y: 0)
                .shadow(color: .black, radius: 0, x: -o, y: 0)
                .shadow(color: .black, radius: 0, x: 0, y: o)
                .shadow(color: .black, radius: 0, x: 0, y: -o)
                .shadow(color: look.shadowColour, radius: 0, x: look.shadow * unit * 2, y: look.shadow * unit * 2)
        }
    }
}

extension Color {
    /// A colour from `0xRRGGBBAA`, as the system's caption style gives it.
    init(rgba: UInt32) {
        self.init(
            .sRGB, red: Double(rgba >> 24 & 0xFF) / 255, green: Double(rgba >> 16 & 0xFF) / 255,
            blue: Double(rgba >> 8 & 0xFF) / 255, opacity: Double(rgba & 0xFF) / 255)
    }
}
