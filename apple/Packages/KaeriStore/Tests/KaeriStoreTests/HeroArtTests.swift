// The one rule for a title's wide art.

import KaeriKit
@testable import KaeriStore
import Testing

struct HeroArtTests {
    /// Episodes 1 to `count`, each with a still, the first `completed` of them watched.
    func rows(_ count: Int, completed: Int = 0, stills: Bool = true) -> [EpisodeRow] {
        (1...count).map { n in
            EpisodeRow(
                number: String(n), title: nil, thumbnail: stills ? "ep/900005/\(n).jpg" : nil,
                description: nil, durationSecs: 1440, watched: n <= completed ? 1 : 0,
                completed: n <= completed, kind: nil, skippable: false, airsAt: nil
            )
        }
    }

    let bannerless = entry(900_005, "Hagane no Niwashi") { $0.bannerUrl = nil }

    @Test func theBannerWins() {
        let titled = entry(900_005, "Hagane no Niwashi") { $0.bannerUrl = "banner/900001.jpg" }
        #expect(HeroArt.resolve(titled, episodes: rows(12, completed: 6)) == .banner("banner/900001.jpg"))
        #expect(HeroArt.resolve(titled, episodes: nil) == .banner("banner/900001.jpg"))
    }

    @Test func theLastWatchedEpisodesStill() {
        #expect(HeroArt.resolve(bannerless, episodes: rows(12, completed: 6)) == .still("ep/900005/6.jpg"))
    }

    @Test func whatTheTrackerCountsAsDoneIsWatchedToo() {
        let tracked = entry(900_005, "Hagane no Niwashi") {
            $0.bannerUrl = nil
            $0.progress = TitleProgress(done: 4, next: 5)
        }
        #expect(HeroArt.resolve(tracked, episodes: rows(12)) == .still("ep/900005/4.jpg"))
    }

    @Test func episodeOneWhenNothingIsWatched() {
        #expect(HeroArt.resolve(bannerless, episodes: rows(12)) == .still("ep/900005/1.jpg"))
    }

    @Test func neverAStillFromAnEpisodeNotYetSeen() {
        // Episodes 2 to 12 have stills; 1, the only one watched, has none.
        var table = rows(12, completed: 1)
        table[0].thumbnail = nil
        #expect(HeroArt.resolve(bannerless, episodes: table) == .compact)
        // A watched episode with no still gives way to an earlier one that has one, never a later.
        var later = rows(12, completed: 5)
        later[4].thumbnail = nil
        #expect(HeroArt.resolve(bannerless, episodes: later) == .still("ep/900005/4.jpg"))
    }

    @Test func compactWhenThereIsNothingWide() {
        #expect(HeroArt.resolve(bannerless, episodes: rows(12, completed: 3, stills: false)) == .compact)
        #expect(HeroArt.resolve(bannerless, episodes: []) == .compact)
    }

    @Test func thePlateWhileTheEpisodesAreOnTheirWay() {
        #expect(HeroArt.resolve(bannerless, episodes: nil) == .waiting)
    }

    @Test func theCoverFillsTheFrameWhereItMay() {
        let covered = entry(900_005, "Hagane no Niwashi") {
            $0.bannerUrl = nil
            $0.coverUrl = "cover/900005.jpg"
        }
        let stillless = rows(12, completed: 3, stills: false)
        #expect(HeroArt.resolve(covered, episodes: stillless, coverFills: true) == .cover("cover/900005.jpg"))
        #expect(HeroArt.resolve(covered, episodes: stillless) == .compact)
        // A seen still still comes first, and the plate holds while the episodes are on their way.
        #expect(HeroArt.resolve(covered, episodes: rows(12, completed: 3), coverFills: true) == .still("ep/900005/3.jpg"))
        #expect(HeroArt.resolve(covered, episodes: nil, coverFills: true) == .waiting)
        #expect(HeroArt.cover("cover/900005.jpg").url == nil)
        #expect(!HeroArt.cover("cover/900005.jpg").isCompact)
    }

    @Test func theStillTheEngineSentStandsInBeforeTheTable() {
        let sent = entry(900_005, "Hagane no Niwashi") {
            $0.bannerUrl = nil
            $0.coverUrl = "cover/900005.jpg"
            $0.seenStill = "ep/900005/2.jpg"
        }
        // From the first frame, with no plate.
        #expect(HeroArt.resolve(sent, episodes: nil) == .still("ep/900005/2.jpg"))
        #expect(HeroArt.resolve(sent, episodes: nil, coverFills: true) == .still("ep/900005/2.jpg"))
        // The table, once here, says by the same rule, and what it says comes first.
        #expect(HeroArt.resolve(sent, episodes: rows(12, completed: 3)) == .still("ep/900005/3.jpg"))
        #expect(HeroArt.resolve(sent, episodes: rows(12, stills: false), coverFills: true) == .still("ep/900005/2.jpg"))
        // A banner still wins.
        var bannered = sent
        bannered.bannerUrl = "banner/900005.jpg"
        #expect(HeroArt.resolve(bannered, episodes: nil) == .banner("banner/900005.jpg"))
    }

    @Test func thePlateHoldsOnlyUntilTheWaitIsOver() {
        let covered = entry(900_005, "Hagane no Niwashi") {
            $0.bannerUrl = nil
            $0.coverUrl = "cover/900005.jpg"
        }
        #expect(HeroArt.resolve(covered, episodes: nil, coverFills: true, waited: true) == .cover("cover/900005.jpg"))
        #expect(HeroArt.resolve(covered, episodes: nil, waited: true) == .compact)
        #expect(HeroArt.resolve(covered, episodes: rows(12), coverFills: true, waited: true) == .still("ep/900005/1.jpg"))
    }

    @Test func aFrameThatKeepsItsShapeFallsBackStepByStep() {
        // The iPhone's cards: banner, then a seen still, then the cover, then a titled plate.
        let full = entry(900_005, "Hagane no Niwashi") {
            $0.bannerUrl = "banner/900005.jpg"
            $0.coverUrl = "cover/900005.jpg"
            $0.seenStill = "ep/900005/2.jpg"
        }
        let stillless = rows(12, completed: 3, stills: false)
        #expect(HeroArt.resolve(full, episodes: stillless, coverFills: true, plates: true) == .banner("banner/900005.jpg"))
        var noBanner = full
        noBanner.bannerUrl = nil
        #expect(HeroArt.resolve(noBanner, episodes: stillless, coverFills: true, plates: true) == .still("ep/900005/2.jpg"))
        var coverOnly = noBanner
        coverOnly.seenStill = nil
        #expect(HeroArt.resolve(coverOnly, episodes: stillless, coverFills: true, plates: true) == .cover("cover/900005.jpg"))
        var nothing = coverOnly
        nothing.coverUrl = nil
        #expect(HeroArt.resolve(nothing, episodes: stillless, coverFills: true, plates: true) == .plate)
        #expect(HeroArt.plate.url == nil)
        #expect(!HeroArt.plate.isCompact)
        // The plate waits for the episodes as the cover does, and only frames that keep their shape
        // get one: elsewhere a title with nothing wide is compact.
        #expect(HeroArt.resolve(nothing, episodes: nil, coverFills: true, plates: true) == .waiting)
        #expect(HeroArt.resolve(nothing, episodes: nil, coverFills: true, plates: true, waited: true) == .plate)
        #expect(HeroArt.resolve(nothing, episodes: stillless, coverFills: true) == .compact)
        #expect(HeroArt.resolve(nothing, episodes: stillless) == .compact)
    }

    @MainActor @Test func theStoreFillsOnlyTheFramesThatAskForIt() {
        let store = AppStore()
        let covered = entry(900_005, "Hagane no Niwashi") {
            $0.bannerUrl = nil
            $0.coverUrl = "cover/900005.jpg"
        }
        var bare = covered
        bare.id = 900_006
        bare.coverUrl = nil
        store.apply(.episodes(id: 900_005, rows: rows(12, completed: 2, stills: false)))
        store.apply(.episodes(id: 900_006, rows: rows(12, completed: 2, stills: false)))
        #expect(store.heroArt(covered, fills: true) == .cover("cover/900005.jpg"))
        #expect(store.heroArt(bare, fills: true) == .plate)
        // Elsewhere as before: the Mac fills with the cover, an iPhone sets it beside the title.
        #if os(macOS)
        #expect(store.heroArt(covered) == .cover("cover/900005.jpg"))
        #else
        #expect(store.heroArt(covered) == .compact)
        #endif
        #expect(store.heroArt(bare) == .compact)
    }

    @MainActor @Test func theStoresPlateWaitsSixHundredMillisecondsByItsClock() {
        let store = AppStore()
        var clock = ContinuousClock.now
        store.now = { clock }
        let covered = entry(900_005, "Hagane no Niwashi") {
            $0.bannerUrl = nil
            $0.coverUrl = "cover/900005.jpg"
        }
        #if os(macOS)
        let after = HeroArt.cover("cover/900005.jpg")
        #else
        let after = HeroArt.compact
        #endif
        #expect(store.heroArt(covered) == .waiting, "the wait starts the first time the art is asked")
        clock += .milliseconds(599)
        #expect(store.heroArt(covered) == .waiting)
        clock += .milliseconds(1)
        #expect(store.heroArt(covered) == after, "past the wait, what there is without a still")
        // A still landing later takes the frame.
        store.apply(.episodes(id: 900_005, rows: rows(12, completed: 2)))
        #expect(store.heroArt(covered) == .still("ep/900005/2.jpg"))

        // A title the engine sent a still with never waits, and one that waited once does not
        // wait again.
        var sent = covered
        sent.id = 900_006
        sent.seenStill = "ep/900006/1.jpg"
        #expect(store.heroArt(sent) == .still("ep/900006/1.jpg"))
        var other = covered
        other.id = 900_004
        #expect(store.heroArt(other) == .waiting)
        clock += .seconds(1)
        #expect(store.heroArt(other) == after)
        #expect(store.heroArt(other) == after)
    }

    @MainActor @Test func theSpotlightAsksForItsEpisodesOnceAtThePagesPriority() {
        let store = AppStore()
        let bare = entry(900_005, "Hagane no Niwashi") { $0.bannerUrl = nil }
        store.loadSpotlightEpisodes(bare)
        store.loadSpotlightEpisodes(bare)
        #expect(store.sentLog == ["loadEpisodes"], "once a session, not as the quiet stills")
        var sent = bare
        sent.id = 900_006
        sent.seenStill = "ep/900006/1.jpg"
        store.loadSpotlightEpisodes(sent)
        var bannered = bare
        bannered.id = 900_001
        bannered.bannerUrl = "banner/900001.jpg"
        store.loadSpotlightEpisodes(bannered)
        #expect(store.sentLog == ["loadEpisodes"], "a still or a banner known: nothing asked")
    }

    @Test func aWatchedStillIsOnlyEverAWatchedEpisodes() {
        #expect(HeroArt.watchedStill(rows(12), done: 0) == nil)
        #expect(HeroArt.watchedStill(rows(12, completed: 2), done: 0) == "ep/900005/2.jpg")
        #expect(HeroArt.watchedStill(rows(12), done: 5) == "ep/900005/5.jpg")
    }
}
