// A season or related title picked on the title page: the last one's page stays until the next
// one's words and episodes are here, then dissolves into it. Its art and main button hold on
// their own until they land.

import KaeriKit
import KaeriStore
@testable import KaeriUI
import Testing

@MainActor
struct TitleSwitchTests {
    let row = EpisodeRow(number: "1", title: nil, thumbnail: nil, description: nil, durationSecs: nil,
                         watched: 0, completed: false, kind: nil, skippable: false, airsAt: nil,
                         delayedFrom: nil, metadataOnly: true)

    @Test func thePickedTitleShowsOnceItsWordsAndEpisodesAreHere() {
        let store = AppStore()
        let side: AnilistId = 900_010
        #expect(!TitlePage.pageHere(side, in: store), "nothing of it is here")
        store.apply(.detail(entry { $0.id = side; $0.format = "OVA"; $0.detail = detail() }))
        #expect(!TitlePage.pageHere(side, in: store), "its episodes are on their way")
        store.apply(.episodes(id: side, rows: [row]))
        #expect(TitlePage.pageHere(side, in: store), "the services and the art need not have said")
    }

    @Test func theRowItWasPickedFromStaysUntilItsPageSaysWhatIsInIt() {
        let store = AppStore()
        let side: AnilistId = 900_010
        store.apply(.detail(entry {
            $0.related = [RelatedTitle(id: side, title: "Wayfarer's Long Road", relation: "side story", format: "OVA")]
            $0.detail = detail(seasons: [linked(900_001, nil, "TV", 2024), linked(900_004, nil, "TV", 2025)])
        }))
        // Shown as its card while it waits: it knows no seasons, and the row would empty.
        let card = store.titles.page(side)
        #expect(TitlePage.kin(of: card, last: 900_001, in: store).id == 900_001)
        #expect(TitleFamily(TitlePage.kin(of: card, last: 900_001, in: store), this: side).seasons.count == 2)
        store.apply(.detail(entry { $0.id = side; $0.format = "OVA"; $0.detail = detail() }))
        #expect(TitlePage.kin(of: store.titles.page(side), last: 900_001, in: store).id == side, "its own, once here")
    }
}
