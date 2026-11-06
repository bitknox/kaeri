// The reader's colours. The reader follows the look: Dark keeps the app's tokens, and Paper,
// Light and Sepia switch the chrome to the light tokens from Foundations.

import KaeriDesign
import SwiftUI

struct ReaderTheme: Equatable {
    var isLight: Bool
    var ground: Color
    var raised: Color
    var selected: Color
    var high: Color
    var silk: Color
    var haze: Color
    var rule: Color
    var faint: Color
    var track: Color
    var hair: Color
    var glass: Color
    var glassSolid: Color
    var shadow: Color
    var vermilion: Color
    var water: Color
    var amber: Color
    var onAmber: Color
    /// The page's own backdrop for the look: what shows round a page.
    var page: Color

    var scheme: ColorScheme { isLight ? .light : .dark }

    static let dark = ReaderTheme(
        isLight: false, ground: Color(rgb: 0x161A2E), raised: Color(rgb: 0x222638), selected: Color(rgb: 0x343A57),
        high: Color(rgb: 0x2A2F45), silk: Color(rgb: 0xE4E0D6), haze: Color(rgb: 0x9298B5), rule: Color(rgb: 0x434653),
        faint: Color(rgb: 0x2F3242), track: Color(rgb: 0xE4E0D6, opacity: 0.25), hair: Color(rgb: 0xE4E0D6, opacity: 0.08),
        glass: Color(rgb: 0x282D44, opacity: 0.74), glassSolid: Color(rgb: 0x282D44, opacity: 0.96),
        shadow: Color.black.opacity(0.45), vermilion: Color(rgb: 0xE06B54), water: Color(rgb: 0x7FC7D9),
        amber: Color(rgb: 0xF2A64B), onAmber: Color(rgb: 0x161A2E), page: Color(rgb: 0x161A2E))

    static let light = ReaderTheme(
        isLight: true, ground: Color(rgb: 0xF0EDE4), raised: Color(rgb: 0xE3DFD4), selected: Color(rgb: 0xD6D1C4),
        high: Color(rgb: 0xDCD7CB), silk: Color(rgb: 0x1E2033), haze: Color(rgb: 0x4F546E), rule: Color(rgb: 0xC4BFB2),
        faint: Color(rgb: 0xDAD5C9), track: Color(rgb: 0x1E2033, opacity: 0.2), hair: Color(rgb: 0x1E2033, opacity: 0.1),
        glass: Color(rgb: 0xFFFDF8, opacity: 0.78), glassSolid: Color(rgb: 0xFCFAF4, opacity: 0.97),
        shadow: Color(rgb: 0x1E2033, opacity: 0.18), vermilion: Color(rgb: 0x94301F), water: Color(rgb: 0x1F5D6E),
        amber: Color(rgb: 0xF2A64B), onAmber: Color(rgb: 0x1E2033), page: Color(rgb: 0xF7F5F0))

    /// The tokens a look asks for, and the colour behind the pages for it.
    static func forLook(_ look: String?) -> ReaderTheme {
        var theme: ReaderTheme
        switch look {
        case "paper": theme = .light; theme.page = Color(rgb: 0xEFE9DC)
        case "light": theme = .light
        case "sepia": theme = .light; theme.page = Color(rgb: 0xE9DCC4)
        default: theme = .dark
        }
        return theme
    }

    /// The backdrop a manga page sits on: the look's, or black or white as the sheet says.
    func backdrop(_ background: String?) -> Color {
        switch background {
        case "black": Color.black
        case "white": Color.white
        default: page
        }
    }
}

/// The reader's own device-bound choices that the engine has no key for: the backdrop of the
/// pages, the gap between a strip's images and the keyboard step.
enum ReaderDefaults {
    static let background = "reader.background"
    static let gap = "reader.gap"
    static let step = "reader.step"
    /// The look follows the system's light or dark, which the engine has no choice for.
    static let system = "reader.systemLook"
}
