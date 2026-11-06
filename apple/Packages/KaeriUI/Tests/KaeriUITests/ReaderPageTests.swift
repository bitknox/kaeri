// What the reader does with a page's pixels, and which tokens a look asks for.

import KaeriKit
@testable import KaeriStore
@testable import KaeriUI
import CoreGraphics
import Foundation
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

    @Test func aMissingFileDecodesToNothing() {
        let file = PageFile(
            path: "/nonexistent/page.raw", encoding: .raw(format: .gray8, stride: 4), width: 4, height: 4,
            crop: nil, colour: false, bytes: 16)
        #expect(PageImages.decode(file, longestSide: 100) == nil)
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
