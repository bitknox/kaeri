// How a title's page comes in from the element clicked. On iPhone and iPad the page zooms out of
// it (T1). On the Mac its art flies to its place on the page while the page fades in around it
// (M3): a poster lands on the page's cover, wide art on the page's banner, and Back flies it home.
// The same title can be on screen twice, in the spotlight and on a shelf, so an element is named
// by where it is as well as by its title.

import KaeriDesign
import KaeriKit
import KaeriStore
import SwiftUI

/// The element a title's page opens from.
public struct TitleSource: Hashable, Sendable {
    /// Where on screen it is.
    public enum Place: Hashable, Sendable {
        /// Home's spotlight.
        case spotlight
        /// A Home shelf, by its kind: Continue, This season, Because you finished.
        case shelf(String)
        /// A grid of covers: Seasonal, Search, the Library, the first launch's season.
        case grid
        /// One of Search's rails, by its title.
        case rail(String)
        /// A calendar's row.
        case calendar
        /// The Mac's hover card.
        case hoverCard
    }

    /// What the element shows, which decides what it becomes on the page.
    public enum Art: Hashable, Sendable {
        /// A cover at 2:3, which becomes the page's cover.
        case poster
        /// Wide art, which becomes the page's banner.
        case banner
    }

    public let id: AnilistId
    public let place: Place
    public let art: Art

    public init(_ id: AnilistId, _ place: Place, art: Art) {
        self.id = id
        self.place = place
        self.art = art
    }

    /// A card on the Home shelf of `kind`.
    static func shelf(_ id: AnilistId, _ kind: SectionKind, art: Art) -> TitleSource {
        TitleSource(id, .shelf(String(describing: kind)), art: art)
    }
}

/// A title page on a tab's stack, on iPhone and iPad, with the element it zooms out of.
public struct TitleRoute: Hashable, Sendable {
    public let id: AnilistId
    public let source: TitleSource?

    public init(id: AnilistId, from source: TitleSource? = nil) {
        self.id = id
        self.source = source
    }

    /// What the zoom grows out of: the element clicked, or with none nothing on screen, so the
    /// page never zooms out of another place the title happens to be.
    var zoomID: TitleMatch {
        source.map(TitleMatch.source) ?? .nowhere(self)
    }
}

/// Art in flight between the element a title opened from and its page, on the Mac.
public struct TitleFlight: Equatable, Sendable {
    public let source: TitleSource
    /// Opening the page, or Back from it.
    public let opening: Bool
    /// Whether it is at the page's element now, or still at the source's.
    public var atPage: Bool
    /// The page's element it lands on, or leaves from: the source's kind where the page has it,
    /// its cover where the page has no banner.
    public var landing: TitleSource.Art

    public init(source: TitleSource, opening: Bool, atPage: Bool, landing: TitleSource.Art? = nil) {
        self.source = source
        self.opening = opening
        self.atPage = atPage
        self.landing = landing ?? source.art
    }
}

/// The page's element art lands on: its cover for a poster, its banner for wide art.
struct TitleLandingID: Hashable, Sendable {
    let id: AnilistId
    let art: TitleSource.Art
}

/// What the Mac's flyer and the iPhone and iPad zoom pair by: the element clicked, the page's
/// element art lands on, or for a page opened from nowhere nothing on screen. Every end names it
/// as this one type, since a matched group is keyed by its id's type as well as its value: an id
/// wrapped as `AnyHashable` on one end never meets the same value, typed, on the other.
enum TitleMatch: Hashable, Sendable {
    case source(TitleSource)
    case landing(TitleLandingID)
    case nowhere(TitleRoute)
}

extension EnvironmentValues {
    /// The namespace a tab's zoom runs in, on iPhone and iPad.
    @Entry var zoomNamespace: Namespace.ID? = nil
    /// The namespace the Mac's title art flies in: the main window's.
    @Entry var titleSpace: Namespace.ID? = nil
}

extension View {
    /// This element opens `source`'s page, which grows out of it: the zoom's source on iPhone and
    /// iPad, and where the art flies from on the Mac.
    func titleSource(_ source: TitleSource) -> some View {
        modifier(TitleSourceMark(source: source))
    }

    /// The page's element that art of the kind `art` lands on, on the Mac: its cover or its
    /// banner. It waits unseen while the art is on its way.
    func titleLanding(_ id: AnilistId, _ art: TitleSource.Art) -> some View {
        modifier(TitleLanding(id: id, art: art))
    }
}

extension EnvironmentValues {
    /// Where each title first shows on Home, below the spotlight: the shelf the player grows out
    /// of unless play was pressed on another.
    @Entry var homeFirstShelf: [AnilistId: SectionKind] = [:]
}

extension View {
    /// A Home card the player can grow out of. When the title shows in more than one shelf, only
    /// one card is the player's source: the one play was pressed on, else the title's first
    /// shelf. The others stand aside under their shelf's name, so they never pair with it.
    func homePlayerSource(_ id: AnilistId, _ kind: SectionKind) -> some View {
        modifier(HomePlayerSource(id: id, kind: kind))
    }
}

/// Which Home card the player grows out of.
enum HomePlayer {
    /// Where each title first shows below the spotlight, in the order the shelves read. The week
    /// is left out: its cells do not grow the player.
    static func firstShelf(_ sections: [HomeSection]) -> [AnilistId: SectionKind] {
        var first: [AnilistId: SectionKind] = [:]
        for section in sections where section.kind != .thisWeek {
            for item in section.items where first[item.entry.id] == nil {
                first[item.entry.id] = section.kind
            }
        }
        return first
    }

    /// Whether the card for `id` in the shelf `kind` is the one the player grows out of.
    static func grows(
        from id: AnilistId, _ kind: SectionKind, playingFrom: TitleSource?, firstShelf: [AnilistId: SectionKind]
    ) -> Bool {
        if let from = playingFrom, from.id == id {
            return from.place == .shelf(String(describing: kind))
        }
        return firstShelf[id].map { $0 == kind } ?? true
    }
}

private struct HomePlayerSource: ViewModifier {
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeFirstShelf) private var firstShelf
    @Environment(\.playerZoomAside) private var aside
    let id: AnilistId
    let kind: SectionKind

    func body(content: Content) -> some View {
        let armed = HomePlayer.grows(from: id, kind, playingFrom: navigator.playingFrom, firstShelf: firstShelf)
        content
            .playerZoomSource(id)
            .environment(\.playerZoomAside, armed ? aside : "home \(String(describing: kind))")
    }
}

private struct TitleSourceMark: ViewModifier {
    @Environment(Navigator.self) private var navigator
    #if os(iOS)
    @Environment(\.zoomNamespace) private var space
    #else
    @Environment(\.titleSpace) private var space
    #endif
    let source: TitleSource

    func body(content: Content) -> some View {
        if let space {
            #if os(iOS)
            content.matchedTransitionSource(id: TitleMatch.source(source), in: space)
            #else
            let lifted = navigator.lifted == source
            content
                .matchedGeometryEffect(id: TitleMatch.source(source), in: space, properties: .frame, isSource: true)
                // Its art is in flight or on the page: never drawn in two places.
                .opacity(lifted ? 0 : 1)
                .animation(nil, value: lifted)
                .onAppear { navigator.sourceShown(source, true) }
                .onDisappear { navigator.sourceShown(source, false) }
                .onChange(of: source) { old, new in
                    navigator.sourceShown(old, false)
                    navigator.sourceShown(new, true)
                }
            #endif
        } else {
            content
        }
    }
}

private struct TitleLanding: ViewModifier {
    @Environment(Navigator.self) private var navigator
    @Environment(\.titleSpace) private var space
    let id: AnilistId
    let art: TitleSource.Art

    func body(content: Content) -> some View {
        #if os(macOS)
        if let space {
            let waits = navigator.awaits(id, art)
            let landing = TitleLandingID(id: id, art: art)
            content
                .matchedGeometryEffect(id: TitleMatch.landing(landing), in: space, properties: .frame, isSource: true)
                .opacity(waits ? 0 : 1)
                .animation(nil, value: waits)
                .onAppear { navigator.landingShown(landing, true) }
                .onDisappear { navigator.landingShown(landing, false) }
                .onChange(of: landing) { old, new in
                    navigator.landingShown(old, false)
                    navigator.landingShown(new, true)
                }
        } else {
            content
        }
        #else
        content
        #endif
    }
}

// MARK: - The Mac's flight

extension Navigator {
    /// Whether art in flight is to land on, or has just left, the element `art` of `id`'s page.
    func awaits(_ id: AnilistId, _ art: TitleSource.Art) -> Bool {
        guard let flight else { return false }
        return flight.source.id == id && flight.landing == art
    }

    /// An element a page can open from came on screen, or left it.
    func sourceShown(_ source: TitleSource, _ shown: Bool) {
        let count = (shownSources[source] ?? 0) + (shown ? 1 : -1)
        shownSources[source] = count > 0 ? count : nil
    }

    /// A page's element art can land on came on screen, or left it.
    func landingShown(_ landing: TitleLandingID, _ shown: Bool) {
        let count = (shownLandings[landing] ?? 0) + (shown ? 1 : -1)
        shownLandings[landing] = count > 0 ? count : nil
    }

    /// The element of `id`'s page that art from `source` lands on: the same kind where the page
    /// shows it, else its cover, as on a page with no wide art; nil with neither on screen.
    func landing(of id: AnilistId, from source: TitleSource) -> TitleSource.Art? {
        [source.art, .poster].first { shownLandings[TitleLandingID(id: id, art: $0)] != nil }
    }

    /// The page is gone at once, as a change of section takes it: nothing in flight or lifted.
    func pageGone() {
        flight = nil
        lifted = nil
        openedFrom = nil
        pageCovers = false
        pageSettled = false
    }

    #if os(macOS)
    /// The page comes in over the section with the standard spring. From an element on screen
    /// its art flies to the page's cover or banner while the rest fades in; under Reduce Motion,
    /// or from nowhere, the page crossfades in.
    func openOnMac(_ id: AnilistId, from source: TitleSource?) {
        if titleOnMac != nil, source == nil {
            // A season or related title picked on the page changes the page in place. Where it
            // was opened from stays, so Back after returning to that title still flies home.
            withAnimation(.standard) { titleOnMac = id }
            return
        }
        let flies = !motionReduced && titleOnMac == nil && source.map { $0.id == id && shownSources[$0] != nil } == true
        openedFrom = source
        pageCovers = true
        pageSettled = false
        if flies, let source {
            lifted = source
            flight = TitleFlight(source: source, opening: true, atPage: false)
        } else {
            lifted = nil
            flight = nil
        }
        withAnimation(.standard) {
            titleOnMac = id
        } completion: { [weak self] in
            guard let self, self.titleOnMac == id, self.flight == nil else { return }
            self.pageSettled = true
        }
    }

    /// Back from the page: the art flies home to the element it came from while the page fades,
    /// when that element is still on screen; otherwise the page fades out over the section.
    func backOnMac() {
        pageSettled = false
        if let source = openedFrom, source.id == titleOnMac, lifted == source, flight == nil,
           !motionReduced, shownSources[source] != nil, let landing = landing(of: source.id, from: source) {
            // The flyer takes off once it stands on the page's element; the page goes then.
            flight = TitleFlight(source: source, opening: false, atPage: true, landing: landing)
            return
        }
        flight = nil
        openedFrom = nil
        withAnimation(.standard) {
            titleOnMac = nil
            lifted = nil
        } completion: { [weak self] in
            guard let self, self.titleOnMac == nil else { return }
            self.pageCovers = false
        }
    }

    /// The flyer is drawn where the flight starts: it moves to where it ends on the standard
    /// spring, and Back's page fades out meanwhile.
    func takeOff() {
        guard var flight, flight.atPage != flight.opening else { return }
        if flight.opening {
            // The page came in with the flyer: art lands on its element of the same kind, on its
            // cover where it has no banner. With neither the art has nowhere to go, and would fly
            // to the whole page: the page comes in on its own instead.
            guard let landing = landing(of: flight.source.id, from: flight.source) else {
                self.flight = nil
                lifted = nil
                pageSettled = true
                return
            }
            flight.landing = landing
        }
        flight.atPage = flight.opening
        let landing = flight
        withAnimation(.standard) {
            self.flight = landing
            if !landing.opening { titleOnMac = nil }
        } completion: { [weak self] in
            self?.land(landing)
        }
        // The spring's own end, should a newer change cut it short without saying so.
        Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(600))
            self?.land(landing)
        }
    }

    /// The art is where it was going: the element there is drawn again, and the flyer goes.
    func land(_ landing: TitleFlight) {
        guard flight == landing else { return }
        flight = nil
        if landing.opening {
            pageSettled = true
        } else {
            lifted = nil
            openedFrom = nil
            pageCovers = false
        }
    }
    #endif
}

#if os(macOS)
/// The art in flight, over the section and the page: the same picture the element showed, at its
/// frame, then at the page's cover or banner. A banner gains the page's ground over its foot as
/// it lands.
struct TitleFlyer: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let flight: TitleFlight
    let space: Namespace.ID

    var body: some View {
        let entry = store.titles.page(flight.source.id)
        let atPage = flight.atPage
        let target = atPage
            ? TitleMatch.landing(TitleLandingID(id: flight.source.id, art: flight.landing))
            : TitleMatch.source(flight.source)
        Group {
            switch flight.source.art {
            case .poster:
                ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: TitleHero.coverPixels)
                    .clipShape(.rect(cornerRadius: 8))
            case .banner:
                // Onto the banner it gains the page's ground; onto the cover of a page with no
                // banner, the cover's corners.
                let banner = flight.landing == .banner
                HeroArtView(art: store.heroArt(entry), entry: entry)
                    .overlay(alignment: .bottom) {
                        VStack(spacing: 0) {
                            BannerScrim()
                            Palette.ground.frame(height: 96)
                        }
                        .opacity(atPage && banner ? 1 : 0)
                    }
                    .clipShape(.rect(cornerRadius: atPage ? (banner ? 0 : 8) : ArtShape.bannerRadius))
            }
        }
        .matchedGeometryEffect(id: target, in: space, properties: .frame, isSource: false)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
        // A turn later, once the page that came in with it has put its elements on screen.
        .task { navigator.takeOff() }
    }
}
#endif
