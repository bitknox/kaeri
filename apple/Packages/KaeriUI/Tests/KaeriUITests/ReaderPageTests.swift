// What the reader does with a page's pixels, and which tokens a look asks for.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import CoreGraphics
import Foundation
import ImageIO
import Testing

struct ReaderPageTests {
    private func raw(_ format: PixelFormat, width: UInt32, height: UInt32, crop: PixelRect? = nil) throws -> PageFile {
        let channels: UInt32 = format == .gray8 ? 1 : 4
        let stride = width * channels
        let bytes = Data(repeating: 0x80, count: Int(stride * height))
        let url = FileManager.default.temporaryDirectory.appending(path: "reader-page-\(UUID().uuidString).raw")
        try bytes.write(to: url)
        return PageFile(
            path: url.path, encoding: .raw(format: format, stride: stride), width: width, height: height,
            crop: crop, colour: format == .rgba8, bytes: UInt64(bytes.count))
    }

    @Test func aGreyPageDecodesToItsSize() throws {
        let file = try raw(.gray8, width: 40, height: 60)
        defer { try? FileManager.default.removeItem(atPath: file.path) }
        let image = try #require(PageImages.decode(file, longestSide: 400))
        #expect(image.width == 40)
        #expect(image.height == 60)
    }

    @Test func aColourPageIsCroppedToItsBorders() throws {
        let file = try raw(.rgba8, width: 40, height: 60, crop: PixelRect(x: 4, y: 6, width: 30, height: 44))
        defer { try? FileManager.default.removeItem(atPath: file.path) }
        let image = try #require(PageImages.decode(file, longestSide: 400))
        #expect(image.width == 30)
        #expect(image.height == 44)
    }

    /// A made-up page saved as a PNG, as the engine hands over an original: stripes a pixel wide.
    private func original(width: Int, height: Int) throws -> PageFile {
        let context = try #require(CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue))
        context.setFillColor(red: 0.9, green: 0.9, blue: 0.85, alpha: 1)
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        context.setFillColor(red: 0.1, green: 0.1, blue: 0.3, alpha: 1)
        for x in stride(from: 0, to: width, by: 2) { context.fill(CGRect(x: x, y: 0, width: 1, height: height)) }
        let image = try #require(context.makeImage())
        let url = FileManager.default.temporaryDirectory.appending(path: "reader-page-\(UUID().uuidString).png")
        let destination = try #require(CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, image, nil)
        #expect(CGImageDestinationFinalize(destination))
        let bytes = try Data(contentsOf: url).count
        return PageFile(
            path: url.path, encoding: .original(mime: "image/png"), width: UInt32(width), height: UInt32(height),
            crop: nil, colour: true, bytes: UInt64(bytes))
    }

    @Test func aFitWidthPageIsDecodedForTheHeightItIsDrawnAt() throws {
        // A tall page in a Mac window 760 by 859 points at 2x, drawn 760 points wide and
        // scrolled. Decoded for the box's longest side it would come out 573 pixels wide and be
        // stretched across 1520.
        let file = try original(width: 2000, height: 6000)
        defer { try? FileManager.default.removeItem(atPath: file.path) }
        let box = CGSize(width: 760, height: 859)
        let longest = PageImages.longest(file, width: box.width) * 2
        #expect(longest == 4560)
        let image = try #require(PageImages.decode(file, longestSide: longest))
        #expect(image.width >= Int(box.width * 2))
    }

    @Test func aStripImageKeepsEveryPixelOfItsWidth() throws {
        // The long-strip shape, drawn in the Mac's 720 point column at 2x.
        let file = try original(width: 720, height: 4000)
        defer { try? FileManager.default.removeItem(atPath: file.path) }
        let image = try #require(PageImages.decode(file, longestSide: PageImages.longest(file, width: 720) * 2))
        #expect(image.width == 720)
        #expect(image.height == 4000)
    }

    @Test func aCroppedOriginalDecodesForTheSizeItsCropIsDrawnAt() throws {
        // A scan with its borders cropped, as the engine sends it with crop borders on, shown
        // 600 pixels tall. Decoded for 600 on the whole file, the crop would come out smaller.
        var file = try original(width: 3000, height: 4000)
        defer { try? FileManager.default.removeItem(atPath: file.path) }
        file.crop = PixelRect(x: 300, y: 400, width: 2400, height: 3200)
        let image = try #require(PageImages.decode(file, longestSide: 600))
        #expect(image.height >= 600)
        #expect(abs(Double(image.width) / Double(image.height) - 0.75) < 0.01)
    }

    @Test func aMissingFileDecodesToNothing() {
        let file = PageFile(
            path: "/nonexistent/page.raw", encoding: .raw(format: .gray8, stride: 4), width: 4, height: 4,
            crop: nil, colour: false, bytes: 16)
        #expect(PageImages.decode(file, longestSide: 100) == nil)
    }

    /// The decoded pages' cache, which the reader's close empties. One at a time, as they share it.
    @Suite(.serialized) struct Cache {
        @Test func aPageDecodedBeforeTheReaderClosedIsNotKept() throws {
            let file = try ReaderPageTests().original(width: 40, height: 60)
            defer { try? FileManager.default.removeItem(atPath: file.path) }
            let image = try #require(PageImages.decode(file, longestSide: 60))
            let opened = PageImages.closeCount
            PageImages.removeAll()
            PageImages.keep(image, file, longestSide: 60, since: opened)
            #expect(PageImages.cached(file, longestSide: 60) == nil)
            PageImages.keep(image, file, longestSide: 60, since: PageImages.closeCount)
            #expect(PageImages.cached(file, longestSide: 60) != nil)
            PageImages.removeAll()
            #expect(PageImages.cached(file, longestSide: 60) == nil)
        }

        #if os(macOS)
        /// SwiftUI on the Mac readies a page for the screen itself. A page redrawn first is one
        /// Core Graphics keeps a converted copy of in its own cache, past the reader's close.
        @Test func theMacKeepsAPageAsItDecodes() async throws {
            let file = try ReaderPageTests().original(width: 40, height: 60)
            defer { try? FileManager.default.removeItem(atPath: file.path) }
            let decoded = try #require(PageImages.decode(file, longestSide: 60))
            let loaded = try #require(await PageImages.load(file, longestSide: 60))
            let redrawn = PageImages.forScreen(decoded)
            #expect(redrawn.bitmapInfo != decoded.bitmapInfo)
            #expect(loaded.bitmapInfo == decoded.bitmapInfo)
            PageImages.removeAll()
        }
        #endif
    }

    @Test func theLightLooksUseTheLightTokensAndDarkKeepsTheApps() {
        #expect(ReaderTheme.forLook("paper").isLight)
        #expect(ReaderTheme.forLook("sepia").isLight)
        #expect(ReaderTheme.forLook("light").isLight)
        #expect(!ReaderTheme.forLook("dark").isLight)
        #expect(!ReaderTheme.forLook(nil).isLight)
        #expect(ReaderTheme.forLook("paper").page != ReaderTheme.forLook("sepia").page)
    }

    @Test func theBackdropFollowsTheSheetsChoice() {
        let theme = ReaderTheme.forLook("dark")
        #expect(theme.backdrop("black") == .black)
        #expect(theme.backdrop("white") == .white)
        #expect(theme.backdrop("theme") == theme.page)
    }
}
