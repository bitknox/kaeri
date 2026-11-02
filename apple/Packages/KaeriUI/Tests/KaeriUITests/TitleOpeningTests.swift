// Where a title's page opens from: the element clicked, named by its place as well as its title,
// and on the Mac the art that flies from it to the page's cover or banner.

@testable import KaeriUI
import KaeriKit
import KaeriStore
import Testing

@MainActor
struct TitleOpeningTests {
    private let spotlight = TitleSource(900_001, .spotlight, art: .banner)
    private let shelf = TitleSource.shelf(900_001, .thisSeason, art: .poster)
    private let cover = TitleLandingID(id: 900_001, art: .poster)
    private let banner = TitleLandingID(id: 900_001, art: .banner)

    @Test func theSameTitleInTwoPlacesIsTwoSources() {
        #expect(spotlight != shelf)
        #expect(TitleRoute(id: 900_001, from: spotlight).zoomID != TitleRoute(id: 900_001, from: shelf).zoomID)
        #expect(TitleSource.shelf(900_001, .thisSeason, art: .poster) != TitleSource.shelf(900_001, .because, art: .poster))
    }

    @Test func thePushedPageKeepsWhereItOpenedFrom() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.openTitle(900_001, from: spotlight)
        #expect(navigator.paths[.home] == [TitleRoute(id: 900_001, from: spotlight)])
        #expect(navigator.paths[.home]?.last?.zoomID == .source(spotlight))
    }

    @Test func aPageFromNowhereZoomsOutOfNoElement() {
        let route = TitleRoute(id: 900_001)
        #expect(route.zoomID == .nowhere(route))
        #expect(route.zoomID != .source(spotlight))
        #expect(route.zoomID != .landing(banner))
    }

    @Test func thePlayerGrowsOutOfOneHomeCardForATitleOnTwoShelves() {
        let first: [AnilistId: SectionKind] = [900_001: .continue]
        #expect(HomePlayer.grows(from: 900_001, .continue, playingFrom: nil, firstShelf: first))
        #expect(!HomePlayer.grows(from: 900_001, .thisSeason, playingFrom: nil, firstShelf: first))
        // Play pressed on the season's poster: that card, not the Continue card.
        let pressed = TitleSource.shelf(900_001, .thisSeason, art: .poster)
        #expect(HomePlayer.grows(from: 900_001, .thisSeason, playingFrom: pressed, firstShelf: first))
        #expect(!HomePlayer.grows(from: 900_001, .continue, playingFrom: pressed, firstShelf: first))
        // From the spotlight: no shelf card.
        #expect(!HomePlayer.grows(from: 900_001, .continue, playingFrom: spotlight, firstShelf: first))
        // A press on another title leaves this one to its first shelf.
        let other = TitleSource.shelf(900_002, .thisSeason, art: .poster)
        #expect(HomePlayer.grows(from: 900_001, .continue, playingFrom: other, firstShelf: first))
    }

    #if os(macOS)
    @Test func aPosterOnScreenFliesToThePagesCover() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(shelf, true)
        navigator.openTitle(900_001, from: shelf)
        #expect(navigator.flight == TitleFlight(source: shelf, opening: true, atPage: false))
        #expect(navigator.lifted == shelf, "the tile is not drawn under its art in flight")
        #expect(navigator.awaits(900_001, .poster), "the page's cover waits for it")
        #expect(!navigator.awaits(900_001, .banner), "the banner comes in with the page")
        #expect(navigator.pageCovers && !navigator.pageSettled)
        navigator.landingShown(cover, true)
        navigator.takeOff()
        // Without a window the spring ends at once, so the flight may already have landed.
        if let flight = navigator.flight {
            #expect(flight.atPage)
            navigator.land(flight)
        }
        #expect(navigator.flight == nil && navigator.pageSettled && navigator.lifted == shelf)
    }

    @Test func theSpotlightFliesToThePagesBannerAndNotFromTheShelf() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(shelf, true)
        navigator.sourceShown(spotlight, true)
        navigator.openTitle(900_001, from: spotlight)
        #expect(navigator.flight?.source == spotlight)
        #expect(navigator.lifted == spotlight)
        #expect(navigator.awaits(900_001, .banner))
        #expect(!navigator.awaits(900_001, .poster))
    }

    @Test func theSpotlightLandsOnTheCoverOfAPageWithNoBanner() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(spotlight, true)
        navigator.openTitle(900_001, from: spotlight)
        navigator.landingShown(cover, true)
        navigator.takeOff()
        if let flight = navigator.flight {
            #expect(flight.landing == .poster)
            #expect(navigator.awaits(900_001, .poster))
            #expect(!navigator.awaits(900_001, .banner))
        }
    }

    @Test func artWithNowhereToLandLeavesThePageToComeInAlone() {
        // Art flying to an element that is not on screen would fly to the whole page.
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(spotlight, true)
        navigator.openTitle(900_001, from: spotlight)
        navigator.takeOff()
        #expect(navigator.flight == nil)
        #expect(navigator.lifted == nil)
        #expect(navigator.pageSettled)
        #expect(navigator.titleOnMac == 900_001)
    }

    @Test func aSeasonPickedOnThePageKeepsWhereThePageOpenedFrom() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(spotlight, true)
        navigator.openTitle(900_001, from: spotlight)
        navigator.landingShown(banner, true)
        navigator.takeOff()
        if let flight = navigator.flight { navigator.land(flight) }
        navigator.openTitle(900_002)
        #expect(navigator.titleOnMac == 900_002)
        #expect(navigator.flight == nil)
        #expect(navigator.lifted == spotlight, "the spotlight stays lifted until the page goes")
    }

    @Test func nothingFliesFromAnElementOffScreenOrUnderReduceMotion() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.openTitle(900_001, from: shelf)
        #expect(navigator.flight == nil)
        #expect(navigator.lifted == nil)
        #expect(navigator.titleOnMac == 900_001)
        navigator.back()

        let reduced = Navigator(route: LaunchRoute())
        reduced.motionReduced = true
        reduced.sourceShown(shelf, true)
        reduced.openTitle(900_001, from: shelf)
        #expect(reduced.flight == nil)
        #expect(reduced.lifted == nil)
    }

    @Test func aTitleOpenedFromItsPageComesInWithoutAFlight() {
        let navigator = Navigator(route: LaunchRoute())
        let related = TitleSource(900_002, .grid, art: .poster)
        navigator.sourceShown(related, true)
        navigator.openTitle(900_001)
        navigator.openTitle(900_002, from: related)
        #expect(navigator.flight == nil)
        #expect(navigator.titleOnMac == 900_002)
    }

    @Test func backFliesTheArtHomeOnceItStandsOnThePage() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(shelf, true)
        navigator.openTitle(900_001, from: shelf)
        navigator.landingShown(cover, true)
        navigator.takeOff()
        if let flight = navigator.flight { navigator.land(flight) }
        #expect(navigator.pageSettled)
        navigator.back()
        #expect(navigator.flight == TitleFlight(source: shelf, opening: false, atPage: true))
        #expect(navigator.titleOnMac == 900_001, "the page stays until the flyer is drawn on it")
        #expect(!navigator.pageSettled, "the page's banner stands aside for the player")
        navigator.takeOff()
        #expect(navigator.titleOnMac == nil)
        if let flight = navigator.flight {
            #expect(!flight.atPage)
            #expect(navigator.lifted == shelf, "the tile stays hidden until the art lands on it")
            navigator.land(flight)
        }
        #expect(navigator.flight == nil && navigator.lifted == nil && !navigator.pageCovers)
    }

    @Test func aTileClickedWhileItsArtFliesHomeFliesAgainAsANewTrip() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(shelf, true)
        navigator.openTitle(900_001, from: shelf)
        navigator.landingShown(cover, true)
        navigator.takeOff()
        if let flight = navigator.flight { navigator.land(flight) }
        navigator.back()
        navigator.takeOff()
        // The art is on its way home, or just landed there, when the tile is clicked again.
        let backTrip = TitleFlight(source: shelf, opening: false, atPage: false).trip
        if let flight = navigator.flight { #expect(flight.trip == backTrip) }
        navigator.openTitle(900_001, from: shelf)
        let opening = navigator.flight
        #expect(opening?.opening == true)
        // The shell keys the flyer by its trip: a new trip is a new flyer, which takes off on
        // its own. Kept as the last one, it never took off and the page's cover stayed hidden.
        #expect(opening?.trip != backTrip)
        #expect(navigator.awaits(900_001, .poster))
        navigator.takeOff()
        if let flight = navigator.flight {
            #expect(flight.atPage)
            navigator.land(flight)
        }
        #expect(navigator.flight == nil && navigator.pageSettled)
        #expect(!navigator.awaits(900_001, .poster), "the cover is drawn once the art lands")
    }

    @Test func aSecondClickWhileThePageOpensLetsTheArtFlyOn() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(shelf, true)
        navigator.openTitle(900_001, from: shelf)
        let opening = navigator.flight
        // The second click of a double click, before the flyer takes off.
        navigator.openTitle(900_001, from: shelf)
        #expect(navigator.flight == opening, "the flight is not torn down")
        #expect(navigator.lifted == shelf)
        #expect(navigator.awaited == cover)
        navigator.landingShown(cover, true)
        navigator.takeOff()
        // And again with the art in the air, or just landed: without a window the spring ends
        // at once.
        let inAir = navigator.flight
        navigator.openTitle(900_001, from: shelf)
        #expect(navigator.flight == inAir)
        if let flight = navigator.flight { navigator.land(flight) }
        #expect(navigator.flight == nil && navigator.pageSettled)
        #expect(navigator.awaited == nil, "the cover is drawn once the art lands")
        navigator.back()
        navigator.takeOff()
        if let flight = navigator.flight { navigator.land(flight) }
        #expect(navigator.titleOnMac == nil && !navigator.canGoBack, "one Back leaves the page")
    }

    @Test func aSecondClickOnAnOpenPageLeavesItsCoverDrawn() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(shelf, true)
        navigator.openTitle(900_001, from: shelf)
        navigator.landingShown(cover, true)
        navigator.takeOff()
        if let flight = navigator.flight { navigator.land(flight) }
        navigator.openTitle(900_001, from: shelf)
        navigator.openTitle(900_001)
        #expect(navigator.flight == nil && navigator.awaited == nil)
        #expect(navigator.pageSettled && navigator.lifted == shelf, "Back still flies home")
    }

    @Test func aPageFromNowhereAwaitsNoArt() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.openTitle(900_001)
        #expect(navigator.flight == nil && navigator.awaited == nil)
        navigator.openTitle(900_001)
        #expect(navigator.awaited == nil)
    }

    @Test func backFadesWhenTheElementHasGone() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(shelf, true)
        navigator.openTitle(900_001, from: shelf)
        navigator.landingShown(cover, true)
        navigator.takeOff()
        if let flight = navigator.flight { navigator.land(flight) }
        navigator.sourceShown(shelf, false)
        navigator.back()
        #expect(navigator.flight == nil)
        #expect(navigator.titleOnMac == nil)
        #expect(navigator.lifted == nil)
    }

    @Test func aSectionChangeTakesThePageAndItsFlightAtOnce() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.sourceShown(shelf, true)
        navigator.openTitle(900_001, from: shelf)
        navigator.section = .calendar
        #expect(navigator.titleOnMac == nil)
        #expect(navigator.flight == nil)
        #expect(navigator.lifted == nil)
        #expect(!navigator.pageCovers)
    }
    #endif
}
