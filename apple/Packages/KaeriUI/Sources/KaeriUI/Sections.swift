// The app's sections, as the Mac sidebar and the iPhone tab bar list them.

import KaeriKit
import KaeriStrings
import Foundation
import Observation

/// Where the app can be. The Mac sidebar lists them all; the iPhone's tabs are Home,
/// Calendar, Seasonal and Library, with Search on its own.
public enum AppSection: String, CaseIterable, Identifiable, Hashable, Sendable {
    case home, calendar, seasonal, search, library, downloads, together
    /// Reading's Home and its new chapters.
    case read, updates
    case providers, pluginStores
    case settings

    public var id: String { rawValue }

    /// What Kaeri is for, as the shell last saw it: Home is Watch beside Read with both, and
    /// Read is Home with reading alone. Set by the shell on the main actor; observed, so the
    /// names a view drew follow it.
    static var answer: KaeriIsFor {
        get { SectionNames.shared.answer }
        set { SectionNames.shared.answer = newValue }
    }

    /// The name, as the Mac sidebar, the toolbar and the iPhone's tabs say it.
    public var title: String {
        switch self {
        case .home: Self.answer == .both ? L.navWatch : L.homeTitle
        case .read: Self.answer == .both ? L.navRead : L.homeTitle
        case .updates: L.navUpdates
        case .calendar: L.navCalendar
        case .seasonal: L.navSeasonal
        case .search: L.navSearch
        case .library: L.navLibrary
        case .downloads: L.navDownloads
        case .together: L.navWatchTogether
        case .providers: L.sourcesYour
        case .pluginStores: L.navPluginStores
        case .settings: L.navSettings
        }
    }

    /// Its SF Symbol.
    public var symbol: String {
        switch self {
        case .home: Self.answer == .both ? "tv" : "house"
        case .read: Self.answer == .both ? "book" : "house"
        case .updates: "bell"
        case .calendar: "calendar"
        case .seasonal: "leaf"
        case .search: "magnifyingglass"
        case .library: "books.vertical"
        case .downloads: "arrow.down.to.line"
        case .together: "person.2"
        case .providers: "powerplug"
        case .pluginStores: "bag"
        case .settings: "gearshape"
        }
    }

    /// The sidebar's first group, with watching alone.
    static let browse: [AppSection] = [.home, .calendar, .seasonal, .search, .library, .downloads, .together]

    /// The sidebar's groups above Sources for an answer: watching alone is today's list; reading
    /// alone has Home, Updates, Search, Library and Downloads; both puts Watch with Calendar and
    /// Seasonal, then Read with Updates, then the shared places, with no headings.
    static func groups(_ answer: KaeriIsFor) -> [[AppSection]] {
        switch answer {
        case .watch: [browse]
        case .read: [[.read, .updates, .search, .library, .downloads]]
        case .both: [[.home, .calendar, .seasonal], [.read, .updates], [.search, .library, .downloads, .together]]
        }
    }

    /// Whether the section is one of `answer`'s places.
    func belongs(to answer: KaeriIsFor) -> Bool {
        switch self {
        case .home, .calendar, .seasonal, .together: answer != .read
        case .read, .updates: answer != .watch
        default: true
        }
    }

    /// The section to show in place of this one when `answer` has no such place.
    func standIn(for answer: KaeriIsFor) -> AppSection {
        belongs(to: answer) ? self : (answer == .read ? .read : .home)
    }
    /// The sidebar's Sources group.
    static let sources: [AppSection] = [.providers, .pluginStores]
}

/// The answer the sections' names follow, observed so the sidebar, the toolbar and the tabs draw
/// again when it changes.
@Observable
final class SectionNames: @unchecked Sendable {
    static let shared = SectionNames()
    var answer: KaeriIsFor = .watch
}
