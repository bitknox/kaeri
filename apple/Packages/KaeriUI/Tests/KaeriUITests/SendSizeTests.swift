// The Send sheet's size: the downloaded chapters' share and the engine's estimate for the rest.
// The engine says 0 for a size it can't tell yet, which is unknown, never nothing to add.

@testable import KaeriUI
import Testing

struct SendSizeTests {
    @Test func anEstimateOfZeroIsUnknown() {
        #expect(SendWords.size(held: 0, of: 3, kept: nil, toFetch: 0) == nil)
        #expect(SendWords.size(held: 1, of: 3, kept: (bytes: 30_000_000, chapters: 3), toFetch: 0) == nil)
        #expect(SendWords.size(held: 0, of: 3, kept: nil, toFetch: nil) == nil)
    }

    @Test func knownPartsAddUp() {
        #expect(SendWords.size(held: 0, of: 3, kept: nil, toFetch: 45_000_000) == 45_000_000)
        #expect(SendWords.size(held: 1, of: 3, kept: (bytes: 30_000_000, chapters: 3), toFetch: 20_000_000) == 30_000_000)
        // Every chapter downloaded: no estimate is needed.
        #expect(SendWords.size(held: 2, of: 2, kept: (bytes: 30_000_000, chapters: 3), toFetch: nil) == 20_000_000)
    }
}
