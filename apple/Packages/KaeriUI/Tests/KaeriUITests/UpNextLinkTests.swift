// Where Read Home's Up next leads: View all opens Planning, so it shows only with Planning in it.

import KaeriKit
import KaeriStore
@testable import KaeriUI
import Foundation
import Testing

@MainActor
struct UpNextLinkTests {
    func tile(_ work: String, planning: Bool) -> WorkTile {
        WorkTile(
            work: WorkId(work), title: "Lantern Street Notes", medium: .manga, cover: nil, // i18n-skip
            formatLabel: "manga", progress: nil, next: nil, newChapter: false, adult: false, // i18n-skip
            facts: [.reading(.readingKindManga)] + (planning ? [.reading(.readingOnPlanning)] : []),
            demo: false, notInSources: false, origin: nil, amount: nil, started: nil)
    }

    /// Suggestions alone, as the row holds with nothing on Planning: View all would open an empty
    /// Planning list, so there is none.
    @Test func suggestionsAloneHaveNoViewAll() {
        let row = [tile("a:900301", planning: false), tile("a:900302", planning: false)]
        #expect(!ReadingModel.upNextLinksPlanning(row))
    }

    @Test func aRowWithPlanningLeadsToPlanning() {
        let row = [tile("a:900301", planning: true), tile("a:900302", planning: false)]
        #expect(ReadingModel.upNextLinksPlanning(row))
    }
}
