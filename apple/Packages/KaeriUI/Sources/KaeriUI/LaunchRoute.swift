// Where the app opens, as the launch arguments say: for screenshots and for coming back to a
// screen while working on it.

import KaeriKit
import Foundation

/// The screen to open at launch.
///
///     -KaeriSection calendar    a section by its name: continue, calendar, seasonal, search,
///                                   library, downloads, providers, pluginStores, settings
///     -KaeriSelect 900005       the title selected in a list
///     -KaeriTitle 900001        a title's page, over the section
///     -KaeriQuery hoshikuzu     what Search searches for
///     -KaeriGenre Fantasy       Search browses a genre, with any query's words
///     -KaeriAvailable here      Search's Available filter: here or services
///     -KaeriFilters set         Search with a row of filters and the sheet's set: Fantasy, 2026, Fall, TV,
///                                   score 70% and Hide obscure titles
///     -KaeriScroll 420          Search's results scrolled this many points once they are drawn
///     -KaeriLibrary planning    the Library's list
///     -KaeriPalette hoshi       the ⌘K palette, open on these words
///     -KaeriPane accounts       Settings' pane, or on iPhone the Settings sheet at its page
///     -KaeriSheet torrent       a sheet over the screen: torrent, proxy (Edit proxy), join, kitsu, update, show,
///                                   source (the source picker), filters (Search's filters), typing (the search field focused),
///                                   welcome (first launch), messages, discussion-7 (episode 7's discussion of the
///                                   title, read from the testing engine's fixture thread)
///     -KaeriSource licensed     the row Sources has selected: licensed, server or folder
///     -KaeriFold open           the title page's watched episodes, opened
///     -KaeriDay 2               the calendar opened on the day this many days after today,
///                                   for a day with broadcasts: the fixtures' own day can have none
///     -KaeriCalendarMine 1      the calendar on My list (it opens on All airing)
///     -KaeriShowcase stores     invented rows beside the testing engine's: see `Showcase`
///     -KaeriCatalogue           Search's browse with the test kit's eight more titles beside the six
public struct LaunchRoute: Equatable, Sendable {
    public var section: AppSection = .home
    public var selected: AnilistId?
    public var title: AnilistId?
    public var query: String?
    public var genre: String?
    public var available: String?
    public var filters: String?
    public var scroll: Int?
    public var library: LibrarySegment?
    public var palette: String?
    public var pane: String?
    public var sheet: String?
    public var fold: String?
    public var source: String?
    /// Days after today the calendar opens on.
    public var day: Int?
    /// The calendar on My list.
    public var calendarMine = false

    public init() {}

    public init(arguments: [String]) {
        func value(_ name: String) -> String? {
            guard let index = arguments.firstIndex(of: name), arguments.indices.contains(index + 1)
            else { return nil }
            return arguments[index + 1]
        }
        // "continue" was Home's name, and still opens it.
        section = value("-KaeriSection").map { $0 == "continue" ? "home" : $0 }.flatMap(AppSection.init(rawValue:)) ?? .home
        selected = value("-KaeriSelect").flatMap { AnilistId($0) }
        title = value("-KaeriTitle").flatMap { AnilistId($0) }
        query = value("-KaeriQuery")
        genre = value("-KaeriGenre")
        available = value("-KaeriAvailable")
        filters = value("-KaeriFilters")
        scroll = value("-KaeriScroll").flatMap { Int($0) }
        palette = value("-KaeriPalette")
        pane = value("-KaeriPane")
        sheet = value("-KaeriSheet")
        fold = value("-KaeriFold")
        source = value("-KaeriSource")
        day = value("-KaeriDay").flatMap { Int($0) }
        calendarMine = value("-KaeriCalendarMine") == "1"
        library = value("-KaeriLibrary").flatMap { name in
            LibraryModelOrder.segments.first { "\($0)" == name }
        }
    }

    /// This process's.
    public static let current = LaunchRoute(arguments: ProcessInfo.processInfo.arguments)
}

/// The tracker's lists by the names the arguments use.
enum LibraryModelOrder {
    static let segments: [LibrarySegment] = [.watching, .planning, .completed, .paused, .dropped]
}
