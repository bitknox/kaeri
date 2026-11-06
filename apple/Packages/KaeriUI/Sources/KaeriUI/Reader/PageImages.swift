// Page files in the cache, decoded for drawing. The engine writes a page under a temporary name
// and renames it, so a path it hands over is always whole.

import KaeriKit
import CoreGraphics
import Foundation
import ImageIO

enum PageImages {
    nonisolated(unsafe) private static let cache: NSCache<NSString, CGImage> = {
        let cache = NSCache<NSString, CGImage>()
        cache.totalCostLimit = 192 << 20
        return cache
    }()

    /// The page as an image, cropped when the file says so; nil when the file cannot be read.
    private static func key(_ file: PageFile, _ longestSide: CGFloat) -> NSString {
        "\(file.path)|\(Int(longestSide))" as NSString
    }

    /// The decoded page when it is already in the cache, so a view can draw it on its first frame.
    static func cached(_ file: PageFile, longestSide: CGFloat) -> CGImage? {
        cache.object(forKey: key(file, longestSide))
    }

    static func load(_ file: PageFile, longestSide: CGFloat) async -> CGImage? {
        let key = key(file, longestSide)
        if let hit = cache.object(forKey: key) { return hit }
        let image = await Task.detached(priority: .userInitiated) { decode(file, longestSide: longestSide) }.value
        if let image { cache.setObject(image, forKey: key, cost: image.bytesPerRow * image.height) }
        return image
    }

    static func decode(_ file: PageFile, longestSide: CGFloat) -> CGImage? {
        var image: CGImage?
        switch file.encoding {
        case .original:
            guard let source = CGImageSourceCreateWithURL(URL(filePath: file.path) as CFURL, nil) else { return nil }
            let limit = max(longestSide, 256)
            let longest = CGFloat(max(file.width, file.height))
            if longest > limit * 1.5 {
                image = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                    kCGImageSourceCreateThumbnailFromImageAlways: true,
                    kCGImageSourceCreateThumbnailWithTransform: true,
                    kCGImageSourceThumbnailMaxPixelSize: limit,
                    kCGImageSourceShouldCacheImmediately: true,
                ] as CFDictionary)
                if let thumb = image, let crop = file.crop {
                    // The crop is in the file's pixels; the thumbnail is smaller.
                    let scale = CGFloat(thumb.width) / CGFloat(file.width)
                    return thumb.cropping(to: CGRect(
                        x: CGFloat(crop.x) * scale, y: CGFloat(crop.y) * scale,
                        width: CGFloat(crop.width) * scale, height: CGFloat(crop.height) * scale))
                }
                return image
            }
            image = CGImageSourceCreateImageAtIndex(source, 0, [kCGImageSourceShouldCacheImmediately: true] as CFDictionary)
        case .raw(let format, let stride):
            guard let data = try? Data(contentsOf: URL(filePath: file.path), options: .mappedIfSafe),
                  let provider = CGDataProvider(data: data as CFData)
            else { return nil }
            let grey = format == .gray8
            image = CGImage(
                width: Int(file.width), height: Int(file.height), bitsPerComponent: 8, bitsPerPixel: grey ? 8 : 32,
                bytesPerRow: Int(stride), space: grey ? CGColorSpaceCreateDeviceGray() : CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGBitmapInfo(rawValue: grey ? 0 : CGImageAlphaInfo.premultipliedLast.rawValue),
                provider: provider, decode: nil, shouldInterpolate: true, intent: .defaultIntent)
        }
        if let image, let crop = file.crop {
            return image.cropping(to: CGRect(x: CGFloat(crop.x), y: CGFloat(crop.y), width: CGFloat(crop.width), height: CGFloat(crop.height)))
        }
        return image
    }
}
