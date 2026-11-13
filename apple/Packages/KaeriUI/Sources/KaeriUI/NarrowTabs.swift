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
    /// Sources, Plugin stores and Settings go onto the first tab, whose account button leads to
    /// them on iPhone.
    static func parent(of section: AppSection, _ answer: KaeriIsFor) -> AppSection? {
        if [.providers, .pluginStores, .settings].contains(section) { return barTabs(answer).first }
        guard [.calendar, .seasonal, .updates].contains(section), !barTabs(answer).contains(section) else { return nil }
        return section == .updates ? .read : .home
    }

    /// What a narrow window pushes onto a tab in place of a section of its own.
    enum Pushed: Equatable { case downloads, place(AppSection) }

    /// Where `section` goes in a narrow window, when it has no tab there: Downloads under the
    /// Library, as its first row pushes it, and the rest onto their tab with a Back button. A
    /// resize never opens a sheet.
    static func narrow(_ section: AppSection, _ answer: KaeriIsFor) -> (tab: AppSection, pushed: Pushed)? {
        if section == .downloads { return (.library, .downloads) }
        return parent(of: section, answer).map { ($0, .place(section)) }
    }

    /// The section a wide window goes back to from what a narrow one pushed, if it is still
    /// pushed on its tab. A sheet the person opened is theirs and stays as it is.
    static func widen(_ section: AppSection, downloads: Bool, place: AppSection?, _ answer: KaeriIsFor) -> AppSection? {
        if section == .library, downloads { return .downloads }
        if let place, parent(of: place, answer) == section { return place }
        return nil
    }

    /// The window made narrow: a section with no tab goes onto the tab that leads to it.
    @MainActor static func narrow(_ navigator: Navigator, _ answer: KaeriIsFor) {
        guard let to = narrow(navigator.section, answer) else { return }
        switch to.pushed {
        case .downloads: navigator.downloadsPushed = true
        case .place(let place): navigator.placePushed = place
        }
        navigator.section = to.tab
    }

    /// The window made wide: what the narrow one pushed is its own section again. The account
    /// sheet is never touched, so one the person opened stays at its page.
    @MainActor static func widen(_ navigator: Navigator, _ answer: KaeriIsFor) {
        guard let to = widen(navigator.section, downloads: navigator.downloadsPushed, place: navigator.placePushed, answer)
        else { return }
        navigator.downloadsPushed = false
        navigator.placePushed = nil
        navigator.section = to
    }
}
