// The widget snapshot keeps the art its decodes returned, even when the cache lets go of it.

@testable import KaeriStore
import CoreGraphics
import Foundation
import Testing

/// A cache the system lets go of everything in the moment a decode goes in, as `NSCache` may
/// whenever memory runs short.
final class PurgedCache: NSCache<NSString, Artwork>, @unchecked Sendable {
    override func object(forKey key: NSString) -> Artwork? { nil }
    override func setObject(_ art: Artwork, forKey key: NSString, cost: Int) {}
}

@MainActor
struct ArtworkSnapshotTests {
    @Test func theWidgetSnapshotHasItsArtWhenTheCacheLetsGoOfIt() async {
        let artwork = ArtworkStore(cache: PurgedCache()) { _ in true }
        let cover = "https://art.example/glass-orchard.png"
        let banner = "https://art.example/lantern-ferry.png"

        async let gathered = artwork.images(for: [cover, banner], maxPixelSize: 300)
        while !(artwork.isPending(cover) && artwork.isPending(banner)) { await Task.yield() }
        artwork.ready(url: cover, bytes: png(width: 460, height: 690))
        artwork.ready(url: banner, bytes: png(width: 1600, height: 900))
        let images = await gathered

        #expect(images[cover]?.height == 300)
        #expect(images[banner]?.width == 300)
    }

    @Test func theWidgetSnapshotLetsGoOfItsArtOnceItHasIt() async {
        let artwork = ArtworkStore(cache: PurgedCache()) { _ in true }
        let cover = "https://art.example/glass-orchard.png"

        async let gathered = artwork.images(for: [cover], maxPixelSize: 300)
        while !artwork.isPending(cover) { await Task.yield() }
        artwork.ready(url: cover, bytes: png(width: 460, height: 690))
        _ = await gathered

        // Held only while the snapshot gathers: after that it is the cache's to keep or let go.
        #expect(artwork.image(cover) == nil)
    }
}
