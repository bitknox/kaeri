// What the store asks the engine for, and what it does not ask twice: AniList answers about thirty
// requests a minute, and a screen shown again or a selection moving back and forth must not
// spend them.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
struct AskingTests {
    @Test func aListAnsweredWithinAMinuteIsNotAskedForAgain() {
        let store = AppStore()
        store.loadContinue()
        store.loadContinue()
        #expect(store.sentLog == ["loadContinue", "loadContinue"], "unanswered, it may be asked again")
        store.apply(.content(view: .continue, content: .entries([entry(900_001, "Stardust Wayfarer")])))
        store.loadContinue()
        #expect(store.sentLog.count == 2, "answered just now")
        store.loadContinue(force: true)
        #expect(store.sentLog.count == 3, "a watch asks anyway")
    }

    @Test func aHoverCardIsAskedForOnceAndNeverAsAPage() {
        let store = AppStore()
        store.loadCards([900_001])
        #expect(store.sentLog == ["loadCards"], "the card, not the page")
        store.apply(.cards([entry(900_001, "Stardust Wayfarer")]))
        store.loadCards([900_001])
        #expect(store.sentLog.count == 1, "its card is in")
        #expect(store.titles[900_001]?.title == "Stardust Wayfarer")
    }

    @Test func aScreensCoversAskForTheirCardsAndTablesOnceAndAHoverAsksQuietly() {
        let store = AppStore()
        store.prepareCards([900_001, 900_002, 900_001])
        store.prepareCards([900_001, 900_002])
        #expect(store.sentLog == ["prepareCards"], "a screen shown again asks nothing")
        store.prepareCards([900_002, 900_003])
        #expect(store.sentLog == ["prepareCards", "prepareCards"], "only the cover not asked yet")
        store.loadHoverEpisodes(900_001)
        store.loadHoverEpisodes(900_001)
        #expect(store.sentLog.last == "loadStills", "at low priority, behind the screen's own requests")
        #expect(store.sentLog.count == 3)
    }

    @Test func aQuestionAboutATitleIsNotPutAgainWhileItsAnswerIsOnItsWay() {
        let store = AppStore()
        store.loadDetail(900_001)
        store.loadEpisodes(900_001)
        store.loadDetail(900_001)
        store.loadEpisodes(900_001)
        store.loadDetail(900_005)
        #expect(store.sentLog == ["loadDetail", "loadEpisodes", "loadDetail"])
    }

    @Test func theDubIsAskedForOnceAndSaidInTheEnginesWords() throws {
        let store = AppStore()
        store.loadDub(900_001)
        store.loadDub(900_001)
        #expect(store.sentLog == ["loadDub"])
        #expect(store.titles.dubLines[900_001] == nil, "nothing shows before the engine answers")
        store.apply(.titleDub(id: 900_001, dub: try DubExtent(through: 6, complete: false)))
        #expect(store.titles.dubLines[900_001] == "Dubbed to episode 6")
        store.apply(.titleDub(id: 900_001, dub: nil))
        #expect(store.titles.dubLines[900_001] == nil, "a dub the source cannot say shows nothing")
    }

    @Test func titlesNamedAnotherWayAskAgainForWhatWasShown() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries([entry(900_001, "Stardust Wayfarer")])))
        store.apply(.content(view: .library(.watching), content: .entries([entry(900_001, "Stardust Wayfarer")])))
        store.loadDetail(900_001)
        store.apply(.titlesChanged(.romaji))
        #expect(store.sentLog == ["loadDetail", "loadContinue", "loadLibrary"], "Seasonal was never shown")
        store.loadDetail(900_001)
        #expect(store.sentLog.last == "loadDetail", "the page asks again")
    }

    @Test func stillsAndTheEpisodesShareOneQuestion() {
        let store = AppStore()
        store.apply(.detail(entry(900_005, "The Steel Gardener")))
        store.loadEpisodes(900_005)
        #expect(!store.loadStills(900_005), "the episodes are on their way")
        #expect(store.sentLog == ["loadEpisodes"])
    }

    @Test func aPageOpenedAfterAHoverAsksForItsEpisodesAtThePagesPriority() {
        let store = AppStore()
        store.loadHoverEpisodes(900_001)
        store.loadEpisodes(900_001)
        #expect(store.sentLog == ["loadStills", "loadEpisodes"], "the page does not wait behind the hover's quiet ask")
        #expect(store.loadStills(900_002))
        store.loadEpisodes(900_002)
        #expect(store.sentLog.suffix(2) == ["loadStills", "loadEpisodes"], "nor behind a card's stills")
        store.loadEpisodes(900_003)
        store.loadHoverEpisodes(900_003)
        #expect(!store.loadStills(900_003))
        #expect(store.sentLog.last == "loadEpisodes", "a slow page's own listing is never replaced by a quiet one")
    }

    @Test func whatTheServicesSayIsAskedAgainWhenTheEpisodesSayOtherwise() {
        let store = AppStore()
        let row = EpisodeRow(number: "1", title: nil, thumbnail: nil, description: nil, durationSecs: nil,
                             watched: 0, completed: false, kind: nil, skippable: false, airsAt: nil,
                             delayedFrom: nil, metadataOnly: true)
        // Asked with the page, before the episodes say whether a source lists it.
        store.apply(.availability(TitleAvailability(id: 900_001, case: .playsHere)))
        #expect(!store.offersKnown(900_001), "its episodes are on their way")
        store.apply(.episodes(id: 900_001, rows: [row]))
        #expect(store.sentLog == ["licensed"], "answered for a source that lists it, and none does")
        #expect(!store.offersKnown(900_001), "Add a source would show, then give way to Watch on")
        store.apply(.availability(TitleAvailability(id: 900_001, case: .notInSources)))
        #expect(store.offersKnown(900_001))
        store.apply(.episodes(id: 900_001, rows: [row]))
        #expect(store.sentLog.count == 1, "answered for what is so: not asked again")
    }

    @Test func aPageOpenedAgainAsksForItsEpisodesOnlyOnceTheTableIsOld() {
        let store = AppStore()
        var clock = ContinuousClock.now
        store.now = { clock }
        let row = { (number: String) in
            EpisodeRow(number: number, title: nil, thumbnail: nil, description: nil, durationSecs: nil, watched: 0,
                       completed: false, kind: nil, skippable: false, airsAt: nil, delayedFrom: nil,
                       metadataOnly: false)
        }
        store.loadEpisodesForPage(900_001)
        store.apply(.episodes(id: 900_001, rows: [row("1")]))
        clock += .seconds(4 * 60)
        store.loadEpisodesForPage(900_001)
        #expect(store.sentLog == ["loadEpisodes"], "within the window: nothing asked")
        clock += .seconds(2 * 60)
        store.loadEpisodesForPage(900_001)
        #expect(store.sentLog == ["loadEpisodes", "loadEpisodes"], "older than the window: asked again")
        #expect(store.titles.episodes[900_001]?.count == 1, "the held table stays until the new one lands")
        store.apply(.episodes(id: 900_001, rows: [row("1"), row("2")]))
        #expect(store.titles.episodes[900_001]?.count == 2, "the new episode replaces the old table")
        store.loadEpisodesForPage(900_001)
        #expect(store.sentLog.count == 2, "fresh again")
    }

    @Test func aSourceReadyWhereNoneWasAsksAgainForEpisodesThatWereAniListsAlone() {
        let store = AppStore()
        let row = { (metadataOnly: Bool) in
            EpisodeRow(number: "1", title: nil, thumbnail: nil, description: nil, durationSecs: nil, watched: 0,
                       completed: false, kind: nil, skippable: false, airsAt: nil, delayedFrom: nil,
                       metadataOnly: metadataOnly)
        }
        store.loadEpisodes(900_001)
        store.loadEpisodes(900_005)
        store.apply(.episodes(id: 900_001, rows: [row(true)]))
        store.apply(.episodes(id: 900_005, rows: [row(false)]))
        #expect(store.titles.unlisted(900_001))
        #expect(!store.titles.unlisted(900_005), "a source listed it")
        store.apply(.providers([provider("fixture", usable: false)]))
        #expect(store.sentLog.count == 2, "none ready yet")
        // Ready now: asked again at once, though it was asked a moment ago.
        store.apply(.providers([provider("fixture")]))
        #expect(store.sentLog == ["loadEpisodes", "loadEpisodes", "loadEpisodes"])
        store.apply(.providers([provider("fixture")]))
        #expect(store.sentLog.count == 3, "ready already: nothing new")
    }

    @Test func theSystemsSignInSheetOpensWhenTheEngineAsksAndItsAnswerGoesBack() throws {
        let store = AppStore()
        store.apply(.authorize(tracker: "anilist", url: "https://anilist.co/api/v2/oauth/authorize?client_id=1",
                               callbackScheme: "kaeri"))
        let request = try #require(store.accounts.signIn)
        #expect(request.tracker == "anilist" && request.callbackScheme == "kaeri")
        store.finishSignIn(request, callback: URL(string: "kaeri://auth/anilist#access_token=x")!)
        #expect(store.accounts.signIn == nil)
        #expect(store.sentLog == ["finishSignIn"])

        store.apply(.authorize(tracker: "mal", url: "https://myanimelist.net/v1/oauth2/authorize?code_challenge=c",
                               callbackScheme: "kaeri"))
        store.cancelSignIn(try #require(store.accounts.signIn))
        #expect(store.accounts.signIn == nil)
        #expect(store.sentLog == ["finishSignIn", "cancelSignIn"])
    }
}
