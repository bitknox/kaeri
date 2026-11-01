// The flag beside a language's own name in the language picker. It is decoration: VoiceOver
// skips it, and the row's label stays the language's name.

import KaeriDesign
import KaeriStrings
import SwiftUI

/// A flag image, 20 by 15 pt with a corner radius of 2 and a hairline edge in the faint rule so a
/// white flag stays visible on a light surface.
struct LanguageFlag: View {
    let code: String

    static let size = CGSize(width: 20, height: 15)

    var body: some View {
        if let image = Self.image(code) {
            image
                .resizable()
                .interpolation(.high)
                .frame(width: Self.size.width, height: Self.size.height)
                .clipShape(.rect(cornerRadius: 2))
                .overlay(RoundedRectangle(cornerRadius: 2).strokeBorder(Palette.faintRule, lineWidth: 0.5))
                .accessibilityHidden(true)
        }
    }

    private static func image(_ code: String) -> Image? {
        guard let url = L.flagURL(code), let data = try? Data(contentsOf: url) else { return nil }
        #if os(macOS)
        return NSImage(data: data).map(Image.init(nsImage:))
        #else
        return UIImage(data: data).map(Image.init(uiImage:))
        #endif
    }

    /// The flag as a menu item's icon, 20 by 15 pt in its own colours: a menu draws an image at
    /// its own size, and on iPhone and iPad as a template unless told otherwise.
    static func menuIcon(_ code: String) -> Image? {
        guard let url = L.flagURL(code), let data = try? Data(contentsOf: url) else { return nil }
        #if os(macOS)
        guard let image = NSImage(data: data) else { return nil }
        image.size = size
        return Image(nsImage: image)
        #else
        guard let raw = UIImage(data: data), let cg = raw.cgImage else { return nil }
        let scale = CGFloat(cg.width) / size.width
        return Image(uiImage: UIImage(cgImage: cg, scale: scale, orientation: .up).withRenderingMode(.alwaysOriginal))
        #endif
    }
}
