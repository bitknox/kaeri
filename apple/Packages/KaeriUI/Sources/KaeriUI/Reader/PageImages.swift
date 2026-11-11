// Page files in the cache, decoded for drawing. The engine writes a page under a temporary name
// and renames it, so a path it hands over is always whole.

import KaeriKit
import CoreGraphics
import Foundation
import ImageIO
import Synchronization

enum PageImages {
    nonisolated(unsafe) private static let cache: NSCache<NSString, CGImage> = {
        let cache = NSCache<NSString, CGImage>()
        cache.totalCostLimit = costLimit
        return cache
    }()

    /// How many bytes of decoded pages the cache keeps: the pages on screen and a few either
    /// side of them.
    static let costLimit = 192 << 20

    /// How many times the reader has closed, so a page decoded for a reader that has closed since
    /// is not kept for the next one.
    private static let closes = Mutex(0)

    static var closeCount: Int { closes.withLock { $0 } }

    /// Let every decoded page go, as the reader closes, and any decode still running with them.
    static func removeAll() {
        closes.withLock { closes in
            closes += 1
            cache.removeAllObjects()
        }
    }

    /// Keep a decoded page, unless the reader it was decoded for has closed since `closeCount`
    /// was `since`.
    static func keep(_ image: CGImage, _ file: PageFile, longestSide: CGFloat, since: Int) {
        closes.withLock { closes in
            guard closes == since else { return }
            cache.setObject(image, forKey: key(file, longestSide), cost: image.bytesPerRow * image.height)
        }
    }

    /// The file, its crop and the size it is decoded for: one file can show whole and cropped,
    /// or as two halves.
    private static func key(_ file: PageFile, _ longestSide: CGFloat) -> NSString {
        let crop = file.crop.map { "\($0.x),\($0.y),\($0.width),\($0.height)" } ?? ""
        return "\(file.path)|\(crop)|\(Int(longestSide))" as NSString
    }

    /// The decoded page when it is already in the cache, so a view can draw it on its first frame.
    static func cached(_ file: PageFile, longestSide: CGFloat) -> CGImage? {
        cache.object(forKey: key(file, longestSide))
    }

    /// The longest side of the page drawn `width` wide, in the same units as the width.
    static func longest(_ file: PageFile, width: CGFloat) -> CGFloat {
        let w = CGFloat(file.crop?.width ?? file.width)
        let h = CGFloat(file.crop?.height ?? file.height)
        guard w > 0 else { return width }
        return max(width, width * h / w)
    }

    static func load(_ file: PageFile, longestSide: CGFloat) async -> CGImage? {
        let key = key(file, longestSide)
        if let hit = cache.object(forKey: key) { return hit }
        let since = closeCount
        let image = await Task.detached(priority: .userInitiated) {
            #if os(macOS)
            // SwiftUI on the Mac readies an image for the screen itself, off the main thread. A
            // page redrawn here first is one Core Graphics converts again for that, and it keeps
            // the copy in a cache of its own that outlives the page and the reader: a long strip
            // left over 100 MB behind after the reader closed.
            decode(file, longestSide: longestSide)
            #else
            decode(file, longestSide: longestSide).map(forScreen)
            #endif
        }.value
        if let image { keep(image, file, longestSide: longestSide, since: since) }
        return image
    }

    /// The image redrawn in the layout the screen draws, off the main thread, so its first frame
    /// on screen converts nothing. A page a turn brings in would otherwise hold that frame up.
    /// iPhone and iPad only: see `load`.
    static func forScreen(_ image: CGImage) -> CGImage {
        let grey = image.colorSpace?.model == .monochrome
        let space = grey ? CGColorSpaceCreateDeviceGray() : CGColorSpace(name: CGColorSpace.sRGB)!
        let info = grey
            ? CGImageAlphaInfo.none.rawValue
            : CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        if image.bitsPerComponent == 8, image.bitmapInfo.rawValue == info, image.colorSpace == space {
            return image
        }
        guard let context = CGContext(
            data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: 0,
            space: space, bitmapInfo: info)
        else { return image }
        context.interpolationQuality = .none
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return context.makeImage() ?? image
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
