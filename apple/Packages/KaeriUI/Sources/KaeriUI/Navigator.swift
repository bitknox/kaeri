// Where the app is: the section on screen, the title page over it, the palette, and what the
// menus act on.

import KaeriDesign
import KaeriKit
import KaeriStore
import SwiftUI

/// The app's place, which the shells show and the menus, the palette and the screens move.
@Observable @MainActor
public final class Navigator {
    /// The section on screen: the Mac sidebar's selection, the iPhone's tab.
    public var section: AppSection {
        didSet {
            if section != oldValue {
                titleOnMac = nil
                seriesOnMac = nil
                pageGone()
            }
        }
    }
    /// The title page open over the section on the Mac. On iPhone and iPad a page is pushed on
    /// the tab's own stack, `paths`.
    public internal(set) var titleOnMac: AnilistId? {
        didSet { if titleOnMac != oldValue { endClosedEpisodeVisits() } }
    }
    /// The series page open over the section on the Mac, by work id. On iPhone and iPad it is
    /// pushed on the tab's stack.
    public var seriesOnMac: String?
    /// Whether the Library shows reading rather than shows, with both media on. Each screen
    /// keeps its own.
    public var libraryReading = false
    /// Whether Search looks for reading rather than shows, with both media on.
    public var searchReading = false
    /// A genre to browse in reading's Search, from Home's genre shelf.
    public var readingGenre: String?
    /// The Library's reading list on screen.
    public var readingSegment: LibrarySegmentRead = .reading
    /// The series whose sheet is up, and which: change source, tracking, choose chapters.
    public var seriesSheet: SeriesSheet?
    /// Whether the Add a source sheet is up, and the Add a server step of it.
    public var addBookSource: AddBookSourceStep?
    /// Whether first launch has decided: shown and gone, or not due. Until then the medium
    /// question of a first run waits for its page.
    public var firstLaunchSettled = false
    /// A shot's first-launch page and the medium answer before it, taken as the pages appear.
    var firstLaunchStart: (FirstLaunch.Page, KaeriIsFor?)?
    /// On iPhone and in a narrow window, a place with no tab of its own (Calendar, Seasonal or
    /// Updates when both media are on), pushed onto the tab that leads to it.
    var placePushed: AppSection?
    /// The element the Mac's title page was opened from, which its art flies back to on Back.
    public internal(set) var openedFrom: TitleSource?
    /// The art on its way between that element and the page, on the Mac (M3).
    public internal(set) var flight: TitleFlight?
    /// The element whose art is in flight or on the page: drawn by neither until it is back.
    public internal(set) var lifted: TitleSource?
    /// Whether a title page covers the Mac's section, from its opening until it has gone: the
    /// section's banners are not where the player grows from meanwhile.
    public internal(set) var pageCovers = false
    /// Whether the Mac's page has come in, its art landed: from then its banner is where the
    /// player grows from.
    public internal(set) var pageSettled = false
    /// Reduce Motion, as the Mac's window has it: the page crossfades and nothing flies.
    public var motionReduced = false
    /// The elements on screen a page can open from, on the Mac: art flies only from one of them.
    @ObservationIgnored var shownSources: [TitleSource: Int] = [:]
    /// The elements of the Mac's page on screen that art can land on: its cover, and its banner
    /// where it has wide art.
    @ObservationIgnored var shownLandings: [TitleLandingID: Int] = [:]
    /// Each tab's pushed title pages, on iPhone and iPad, each with the element it zooms from.
    public var paths: [AppSection: [TitleRoute]] = [:] {
        didSet { if paths != oldValue { endClosedEpisodeVisits() } }
    }
    /// The title the menus' Episode items and the palette's commands act on: the one selected
    /// in a list, or the page that is open.
    public var focusedTitle: AnilistId?
    /// Whether the ⌘K palette is up.
    public var paletteShown = false
    /// Whether Messages is up.
    public var messagesShown = false
    /// Whether Kitsu's sign-in sheet is up, as a notice's Sign in asks.
    public var kitsuShown = false
    /// What Search's field holds.
    public var query = ""
    /// Whether every filter is up: a panel on the Mac, the inspector on iPad, a sheet on iPhone.
    public var searchFiltersShown = false
    /// The search the launch arguments ask for, for a screenshot, until Search runs it.
    public var launchSearch: SearchQuery?
    /// Points to scroll Search's results at launch, for shots.
    public var launchScroll: Int?
    /// Counts the asks to scroll the iPhone's pinned filter row to its end, for the motion audit.
    public var searchRowToEnd = 0
    /// Whether Downloads is pushed over the Library, as its first row does on iPhone.
    public var downloadsPushed = false
    /// Counts the viewer's own marks of an episode as watched, for the light impact on iPhone:
    /// a mark the engine makes, from sync or the player closing, has none (Haptics rule 1).
    public var watchedByHand = 0
    /// The Home element play was last pressed on, so the player grows out of that one when the
    /// title shows in more than one place.
    public var playingFrom: TitleSource?
    /// The Library's list on screen.
    public var librarySegment: LibrarySegment = .watching
    /// The calendar's week, as weeks from this one.
    public var calendarWeek = 0
    /// The day the calendar opens on, as days after today, until it has: the launch route's.
    public var calendarOpensOn = 0
    /// Whether the last step in the calendar went forward, for the way the week slides.
    public var calendarForward = true
    /// Counts the times the Calendar tab was tapped again on its own screen: each one returns
    /// the iPhone agenda to today.
    public var calendarToToday = 0
    /// The launch route asks for My list: set on the first look at the calendar.
    var launchCalendarMine = false
    /// The Settings pane on screen, on the Mac and iPad.
    public var settingsPane: SettingsPane = .general
    /// What Settings' own search field holds.
    public var settingsQuery = ""
    /// The iPhone's Settings sheet, open at a page when it is up.
    public var phoneSettings: PhoneSettingsPage?
    /// The Report a bug sheet, when it is up.
    var report: ReportRequest?
    /// An invite link the app was opened with, for the join sheet.
    public var invite: String?
    /// Whether the Open link or file sheet is asked for, from the File menu or the + button.
    public var openLinkShown = false
    /// The sheet the launch arguments ask for, for a screenshot, until a screen shows it.
    public var launchSheet: String?
    /// The title whose own settings are up in a sheet.
    public var showSettings: AnilistId?
    /// A selection asked of Continue's list from outside it, as the list itself would move it.
    public var continueSelection: AnilistId?
    /// Whether the title search field is up for typing. The launch route's "typing" sheet sets it.
    public var searchPresented = false
    /// The title whose "More about this show" sheet is open.
    public var moreAbout: AnilistId?
    /// The title whose source picker is open.
    public var sourcePicker: AnilistId?
    /// The title whose match sheet is open.
    public var matchPicker: MatchPickerRequest?
    /// Whether the sheet that adds a media server is up.
    public var addServerShown = false
    /// A screenshot's way to the Check this server step.
    public var certificateLaunch = false
    /// The row of Sources the launch arguments select, for a screenshot: a source key.
    public var launchSourceKey: String?
    /// The trailer Play trailer opened: a window on the Mac, a sheet on iPhone and iPad.
    var trailerRequest: TrailerRequest?
    /// The titles whose watched episodes are open in their episode lists, for the session.
    var watchedOpen: Set<AnilistId> = []
    /// The episodes whose still and synopsis the viewer asked to see past spoiler protection, per
    /// title, for the visit to its page.
    var synopsisShown: [AnilistId: Set<String>] = [:]
    /// The titles whose rows after spoiler protection's line are all shown, for the visit.
    var synopsisAllShown: Set<AnilistId> = []
    /// The range of a long series' episodes the viewer picked, per title, for the visit.
    var episodeRange: [AnilistId: Int] = [:]
    /// The pick the title page on screen was opened by, while it is: Another asks the same pool
    /// again. Gone once the page is, or another title is opened from it.
    var pickVisit: SearchModel.Pick?
    /// Another was asked, so the pick that comes replaces the page rather than pushing one.
    var anotherAsked = false
    /// On iPad and iPhone, the title a pushed page now shows in place of the one it was pushed
    /// for, after Another: Back still leaves the stack where the pick started from.
    var shownInPlace: [AnilistId: AnilistId] = [:]
    /// The range, and the number typed into Go to episode, set when Go to asks for a row: the
    /// list shows the range and lands the row at its top with the selection fill.
    var goToRow: [AnilistId: GoToLanding] = [:]
    /// Go to episode asked for from the View menu (⌘L) on the title page on screen.
    var goToAsked = 0
    /// The title whose episode list is on screen, which Go to episode (⌘L) acts on.
    var goToShown: AnilistId?
    /// The digits typed into the focused episode list, shown in a capsule until the jump.
    var typedEpisode: TypedEpisode?
    /// The pause after a typed digit, before the jump, and the capsule's second after it.
    @ObservationIgnored var typingPause: Task<Void, Never>?
    /// The ranges of the episode list on screen, which the View menu steps through.
    var episodeRangesShown: EpisodeRangesShown?
    /// Counts the asks to scroll the episode list on screen to its last row, for the motion
    /// audit's range switch with the pickers off screen.
    var episodeListToEnd = 0
    /// An episode handed over from another device, which its title's page offers to resume.
    public var handedOff: HandedOff?

    public init(route: LaunchRoute = .current) {
        section = route.section
        titleOnMac = route.title
        focusedTitle = route.title ?? route.selected
        query = route.query ?? ""
        librarySegment = route.library ?? .watching
        paletteShown = route.palette != nil
        settingsPane = route.pane.flatMap(SettingsPane.init(rawValue:)).flatMap { SettingsPane.shown.contains($0) ? $0 : nil }
            ?? .general
        launchSheet = route.sheet
        if let available = route.available {
            launchSearch = SearchQuery(
                text: route.query ?? "", available: available == "services" ? .onYourServices : .playsHere)
        }
        if route.filters == "set" {
            // Every row filter set, and two of the sheet's, which the fixtures' catalogue answers
            // with two titles (the shot route adds -KaeriCatalogue).
            launchSearch = SearchQuery(
                text: route.query ?? "", genres: ["Fantasy"], formats: [.tv], season: .spring, years: Bounds(min: 2024, max: 2024),
                minScore: 70, licensedOnly: true)
        }
        launchScroll = route.scroll
        if let genre = route.genre { launchSearch = SearchQuery(text: route.query ?? "", genres: [genre]) }
        if route.sheet == "join" { invite = "https://plugins.hotaru.example/join/fixture#key" }
        if route.sheet == "show" { showSettings = route.title }
        if route.sheet == "about" { moreAbout = route.title }
        if route.sheet == "source" { sourcePicker = route.title }
        if let shape = AppStore.matchSheetOpens(route.sheet), let title = route.title {
            matchPicker = MatchPickerRequest(id: title, fromMenu: shape == .fromMenu)
        }
        if route.sheet == "messages" { messagesShown = true }
        if route.sheet == "server" { addServerShown = true }
        if route.sheet == "certificate" { addServerShown = true; certificateLaunch = true }
        launchSourceKey = route.source
        libraryReading = route.reading
        searchReading = route.reading
        if let work = route.work {
            #if os(macOS)
            seriesOnMac = work
            #else
            paths[route.section] = [TitleRoute(work: work)]
            #endif
        }
        if route.fold == "open", let title = route.title { watchedOpen.insert(title) }
        launchCalendarMine = route.calendarMine
        if let day = route.day {
            calendarOpensOn = day
            calendarWeek = weekOffset(ofDaysAhead: day)
        }
        #if os(iOS)
        if UIDevice.current.userInterfaceIdiom == .phone, route.section == .settings {
            phoneSettings = route.pane.flatMap(PhoneSettingsPage.named) ?? .root
        }
        #endif
        if let title = route.title {
            paths[route.section] = [TitleRoute(id: title)]
            pageCovers = true
            pageSettled = true
        }
    }

    /// Open a section, as the sidebar, a tab or ⌘1 to ⌘7 does.
    public func open(_ section: AppSection) {
        self.section = section
    }

    /// A series' page: over the section on the Mac, pushed on the tab's stack elsewhere.
    public func openSeries(_ work: String) {
        #if os(macOS)
        titleOnMac = nil
        withAnimation(.standard) { seriesOnMac = work }
        #else
        paths[section, default: []].append(TitleRoute(work: work))
        #endif
    }

    /// A reading list in the Library.
    public func readingList(_ segment: LibrarySegmentRead) {
        libraryReading = true
        readingSegment = segment
        open(.library)
    }

    /// Reading's Search, browsing `genre` when one is given.
    public func openReadingSearch(genre: String? = nil) {
        searchReading = true
        readingGenre = genre
        open(.search)
    }

    /// Back from a series' page on the Mac.
    public func closeSeries() {
        withAnimation(.standard) { seriesOnMac = nil }
    }

    /// Providers, where a source is added: its section on the Mac and iPad, its page of the
    /// Settings sheet on iPhone.
    public func openProviders() {
        #if os(iOS)
        if UIDevice.current.userInterfaceIdiom == .phone {
            phoneSettings = .providers
            return
        }
        #endif
        open(.providers)
    }

    /// The Licensed services page, from a title page's Choose your services.
    public func openLicensedServices() {
        launchSourceKey = SourceKey.licensed
        #if os(iOS)
        if UIDevice.current.userInterfaceIdiom == .phone {
            phoneSettings = .licensed
            return
        }
        #endif
        open(.providers)
    }

    /// Open Settings at `pane`: the section on the Mac and iPad.
    public func openSettings(_ pane: SettingsPane) {
        settingsPane = pane
        section = .settings
    }

    /// Open a title's page over the section on screen, from `source`, the element clicked: the
    /// cover or banner the page grows out of. With none it comes in on its own.
    public func openTitle(_ id: AnilistId, from source: TitleSource? = nil) {
        focusedTitle = id
        pickVisit = nil
        #if os(macOS)
        // A second click on the title that is opening, or open, changes nothing: its art flies
        // on, and Back still takes one press. Only a page going back opens again.
        if titleOnMac == id, flight?.opening != false { return }
        openOnMac(id, from: source)
        #else
        withAnimation(.standard) { titleOnMac = id }
        #endif
        paths[section, default: []].append(TitleRoute(id: id, from: source))
    }

    /// Back from the title page, as its back button, ⌘[ and Escape do.
    public func back() {
        pickVisit = nil
        anotherAsked = false
        if let last = paths[section]?.last { shownInPlace[last.id] = nil }
        #if os(macOS)
        backOnMac()
        #else
        withAnimation(.standard) { titleOnMac = nil }
        #endif
        if var path = paths[section], !path.isEmpty {
            path.removeLast()
            paths[section] = path
        }
    }

    /// Whether Back has anywhere to go.
    public var canGoBack: Bool {
        titleOnMac != nil || !(paths[section] ?? []).isEmpty
    }

    /// Search for `query` now, in the Search section: a recent search, a rail's See All, a
    /// genre.
    public func search(_ query: SearchQuery, in store: AppStore) {
        section = .search
        store.search(query)
        // After the store has it, so the field's change finds nothing new to search for.
        self.query = query.text
    }

    /// Search for these words, with the filters already set.
    public func search(text: String, in store: AppStore) {
        var query = store.search.query
        query.text = text
        search(query, in: store)
    }
}
