// The iPhone's tab bar and an iPad window made narrow: the places with a tab there, and where the
// rest go. A narrow window pushes a place with no tab onto the tab that leads to it, and a wide
// iPad window makes it its own section again.

import KaeriKit

enum NarrowTabs {
    /// The tabs the iPhone's bar shows.
    static func barTabs(_ answer: KaeriIsFor) -> [AppSection] {
        switch answer {
        case .watch: [.home, .calendar, .seasonal, .library]
        case .read: [.read, .updates, .library]
        case .both: [.home, .read, .library]
        }
    }

    /// The tab a place with no tab of its own is pushed onto, on iPhone and in a narrow window.
    static func parent(of section: AppSection, _ answer: KaeriIsFor) -> AppSection? {
        guard [.calendar, .seasonal, .updates].contains(section), !barTabs(answer).contains(section) else { return nil }
        return section == .updates ? .read : .home
    }

    /// What a narrow window pushes onto a tab in place of a section of its own.
    enum Pushed: Equatable { case downloads, place(AppSection) }

    /// Where `section` goes in a narrow window, when it has no tab there: Downloads under the
    /// Library, as its first row pushes it, and Calendar, Seasonal and Updates onto their tab.
    static func narrow(_ section: AppSection, _ answer: KaeriIsFor) -> (tab: AppSection, pushed: Pushed)? {
        if section == .downloads { return (.library, .downloads) }
        return parent(of: section, answer).map { ($0, .place(section)) }
    }

    /// The section a wide window goes back to from what a narrow one pushed, if anything.
    static func widen(_ section: AppSection, downloads: Bool, place: AppSection?, _ answer: KaeriIsFor) -> AppSection? {
        if section == .library, downloads { return .downloads }
        if let place, parent(of: place, answer) == section { return place }
        return nil
    }
}
