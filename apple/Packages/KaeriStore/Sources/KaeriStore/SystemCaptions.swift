// The system's caption style, for the subtitles that follow it.
//
// With "Follow system caption style" on (Settings, Playback, on by default), font, text colour,
// edge and background come from the style the viewer set for the whole system: Settings,
// Accessibility, Subtitles & Captioning on iPhone and iPad, System Settings, Accessibility,
// Captions on the Mac, read through MediaAccessibility. The engine merges it into mpv's options
// (`mpv_subtitle_options`); this only reads it.

import KaeriKit
import CoreGraphics
import CoreText
import Foundation
import MediaAccessibility

public enum SystemCaptions {
    /// The style the viewer set for the whole system, now.
    public static func current() -> SystemCaptionStyle {
        let domain = MACaptionAppearanceDomain.user
        let descriptor = MACaptionAppearanceCopyFontDescriptorForStyle(domain, nil, .default).takeRetainedValue()
        let family = CTFontDescriptorCopyAttribute(descriptor, kCTFontFamilyNameAttribute) as? String
        let edge: CaptionEdge =
            switch MACaptionAppearanceGetTextEdgeStyle(domain, nil) {
            case .raised: .raised
            case .depressed: .depressed
            case .uniform: .uniform
            case .dropShadow: .dropShadow
            default: .plain
            }
        return SystemCaptionStyle(
            // The system's interface font has a hidden name (".AppleSystemUIFont").
            fontFamily: family.flatMap { $0.hasPrefix(".") ? nil : $0 },
            relativeSize: Double(MACaptionAppearanceGetRelativeCharacterSize(domain, nil)),
            textRgba: rgba(
                MACaptionAppearanceCopyForegroundColor(domain, nil).takeRetainedValue(),
                MACaptionAppearanceGetForegroundOpacity(domain, nil)),
            backgroundRgba: rgba(
                MACaptionAppearanceCopyBackgroundColor(domain, nil).takeRetainedValue(),
                MACaptionAppearanceGetBackgroundOpacity(domain, nil)),
            windowRgba: rgba(
                MACaptionAppearanceCopyWindowColor(domain, nil).takeRetainedValue(),
                MACaptionAppearanceGetWindowOpacity(domain, nil)),
            edge: edge)
    }

    /// A colour and its opacity as `0xRRGGBBAA`.
    static func rgba(_ color: CGColor, _ opacity: CGFloat) -> UInt32 {
        let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
        let c = color.converted(to: srgb, intent: .defaultIntent, options: nil)?.components ?? [0, 0, 0, 1]
        let rgb = c.count >= 3 ? Array(c.prefix(3)) : [c[0], c[0], c[0]]
        func byte(_ value: CGFloat) -> UInt32 { UInt32((min(1, max(0, value)) * 255).rounded()) }
        return byte(rgb[0]) << 24 | byte(rgb[1]) << 16 | byte(rgb[2]) << 8 | byte(opacity)
    }

    /// Where the system says the viewer changed the style.
    public static var changed: Notification.Name {
        Notification.Name(kMACaptionAppearanceSettingsChangedNotification as String)
    }
}
