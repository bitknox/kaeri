// Downloads' empty line speaks of what Kaeri is for: episodes when it watches, chapters when it
// only reads, both when it does both (iPhone R8I-3: a reader saw the episodes line).

import KaeriKit
import KaeriStrings
@testable import KaeriUI
import Testing

@MainActor
struct NoDownloadsLineTests {
    @Test func readingOnlySaysChapters() {
        #expect(NoDownloads.line(.read) == L.readDownloadsEmptyDetail)
    }

    @Test func bothSaysEpisodesAndChapters() {
        #expect(NoDownloads.line(.both) == L.readDownloadsEmptyBoth)
    }

    @Test func watchingKeepsTheEpisodesLine() {
        #expect(NoDownloads.line(.watch) == L.downloadsEmptyDetail)
    }
}
