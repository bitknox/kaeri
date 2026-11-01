// The names the config file uses for the subtitle choices, which a setting is sent by.

import KaeriKit

extension SubtitleFont {
    public var fileName: String {
        switch self {
        case .default: "default"
        case .zenKakuGothicNew: "zen-kaku-gothic-new"
        case .serif: "serif"
        case .monospace: "monospace"
        }
    }
}

extension SubtitleColour {
    public var fileName: String {
        switch self {
        case .white: "white"
        case .yellow: "yellow"
        case .cyan: "cyan"
        case .green: "green"
        }
    }
}

extension SubtitleEdge {
    public var fileName: String {
        switch self {
        case .outline: "outline"
        case .dropShadow: "drop-shadow"
        case .noEdge: "none"
        }
    }
}

extension SubtitleThickness {
    public var fileName: String {
        switch self {
        case .thin: "thin"
        case .medium: "medium"
        case .thick: "thick"
        }
    }
}

extension SubtitleBackground {
    public var fileName: String {
        switch self {
        case .noBackground: "none"
        case .eachLine: "each-line"
        case .allLines: "all-lines"
        }
    }
}

extension StyledSubtitles {
    public var fileName: String {
        switch self {
        case .keep: "keep"
        case .restyle: "restyle"
        case .strip: "strip"
        }
    }
}
