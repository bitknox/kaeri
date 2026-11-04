// How deep the starting screen's ground sits over its art: 55% over normal art and 80% over
// bright art, measured once when the art loads (the board's decision 13).

import CoreGraphics
import Foundation

enum GroundDepth {
    /// Above this mean relative luminance the art counts as bright.
    static let brightAbove = 0.15
    static let normal = 0.55
    static let deep = 0.80

    static func depth(forLuminance luminance: Double) -> Double {
        luminance > brightAbove ? deep : normal
    }

    /// The mean relative luminance (linear sRGB) of the middle 60% of the width and the middle
    /// half of the height, from a copy about 64 pixels wide: where the column sits.
    static func centreLuminance(of image: CGImage) -> Double? {
        let width = 64
        let height = max(1, Int((Double(width) * Double(image.height) / Double(max(1, image.width))).rounded()))
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
            let context = CGContext(
                data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4, space: space,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        context.interpolationQuality = .medium
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        guard let data = context.data else { return nil }
        let pixels = data.bindMemory(to: UInt8.self, capacity: width * height * 4)
        let columns = Int(Double(width) * 0.2)..<Int(Double(width) * 0.8)
        let rows = Int(Double(height) * 0.25)..<max(Int(Double(height) * 0.25) + 1, Int(Double(height) * 0.75))
        var total = 0.0
        var count = 0
        for y in rows {
            for x in columns {
                let at = (y * width + x) * 4
                let alpha = Double(pixels[at + 3]) / 255
                guard alpha > 0 else { continue }
                // Premultiplied: undo it before the curve.
                let r = Self.linear(Double(pixels[at]) / 255 / alpha)
                let g = Self.linear(Double(pixels[at + 1]) / 255 / alpha)
                let b = Self.linear(Double(pixels[at + 2]) / 255 / alpha)
                total += 0.2126 * r + 0.7152 * g + 0.0722 * b
                count += 1
            }
        }
        return count == 0 ? nil : total / Double(count)
    }

    private static func linear(_ value: Double) -> Double {
        let v = min(1, max(0, value))
        return v <= 0.04045 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4)
    }
}
