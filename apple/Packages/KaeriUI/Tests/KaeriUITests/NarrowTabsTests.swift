// An iPad window made narrow keeps the place the person was on: Downloads, which has no tab there,
// goes under the Library as its row pushes it, never the first tab. Made wide again, it is its own
// section again.

import KaeriKit
@testable import KaeriUI
import Testing

struct NarrowTabsTests {
    @Test func downloadsGoesUnderTheLibraryInANarrowWindow() throws {
        for answer in [KaeriIsFor.watch, .read, .both] {
            let to = try #require(NarrowTabs.narrow(.downloads, answer))
            #expect(to.tab == .library)
            #expect(to.pushed == .downloads)
        }
    }

    @Test func placesWithATabStayPut() {
        #expect(NarrowTabs.narrow(.library, .both) == nil)
        #expect(NarrowTabs.narrow(.read, .read) == nil)
        #expect(NarrowTabs.narrow(.calendar, .watch) == nil)
    }

    @Test func placesWithoutATabArePushedOntoTheirTab() throws {
        let calendar = try #require(NarrowTabs.narrow(.calendar, .both))
        #expect(calendar.tab == .home)
        #expect(calendar.pushed == .place(.calendar))
        let updates = try #require(NarrowTabs.narrow(.updates, .both))
        #expect(updates.tab == .read)
    }

    @Test func aWideWindowGoesBackToThePlace() {
        #expect(NarrowTabs.widen(.library, downloads: true, place: nil, .both) == .downloads)
        #expect(NarrowTabs.widen(.home, downloads: false, place: .calendar, .both) == .calendar)
        #expect(NarrowTabs.widen(.library, downloads: false, place: nil, .both) == nil)
        // A place pushed onto another tab than its own is left where it is.
        #expect(NarrowTabs.widen(.library, downloads: false, place: .calendar, .both) == nil)
    }

    @Test func sidebarOnlySectionsArePushedOntoTheFirstTab() throws {
        for answer in [KaeriIsFor.watch, .read, .both] {
            let first = try #require(NarrowTabs.barTabs(answer).first)
            for section in [AppSection.providers, .pluginStores, .settings] {
                let to = try #require(NarrowTabs.narrow(section, answer), "\(section) has no place in a narrow window")
                #expect(to.tab == first)
                #expect(to.pushed == .place(section))
            }
        }
    }

    @Test func noSidebarSectionIsLeftWithoutAPlaceInANarrowWindow() {
        // Watch together goes to Home's people button (TabShell), and Search has its own tab.
        for answer in [KaeriIsFor.watch, .read, .both] {
            let sidebar = AppSection.groups(answer).flatMap { $0 } + [.providers, .pluginStores, .settings]
            for section in sidebar where ![.search, .together].contains(section) {
                #expect(NarrowTabs.barTabs(answer).contains(section) || NarrowTabs.narrow(section, answer) != nil,
                        "\(section) with \(answer) shows another tab in a narrow window")
            }
        }
    }

    @MainActor @Test func narrowThenWide() {
        for answer in [KaeriIsFor.watch, .read, .both] {
            for section in [AppSection.providers, .pluginStores, .settings, .downloads] {
                let navigator = Navigator()
                navigator.section = section
                NarrowTabs.narrow(navigator, answer)
                #expect(navigator.section != section, "\(section) kept as the selection with no tab for it")
                #expect(navigator.phoneSettings == nil, "a resize opened a sheet")
                NarrowTabs.widen(navigator, answer)
                #expect(navigator.section == section)
                #expect(navigator.placePushed == nil)
                #expect(!navigator.downloadsPushed)
            }
        }
    }

    @MainActor @Test func wideThenNarrowThenWideAgain() {
        let navigator = Navigator()
        navigator.section = .settings
        // Wide: nothing to undo.
        NarrowTabs.widen(navigator, .both)
        #expect(navigator.section == .settings)
        NarrowTabs.narrow(navigator, .both)
        #expect(navigator.section == .home)
        #expect(navigator.placePushed == .settings)
        NarrowTabs.widen(navigator, .both)
        #expect(navigator.section == .settings)
        // Narrow again, then Back pops it before the window widens: the first tab stays.
        NarrowTabs.narrow(navigator, .both)
        navigator.placePushed = nil
        NarrowTabs.widen(navigator, .both)
        #expect(navigator.section == .home)
        // Narrow again, then another tab: widening leaves that tab selected.
        navigator.section = .providers
        NarrowTabs.narrow(navigator, .both)
        navigator.section = .library
        NarrowTabs.widen(navigator, .both)
        #expect(navigator.section == .library)
    }

    @MainActor @Test func aSheetThePersonOpenedSurvivesWidening() {
        for page in [PhoneSettingsPage.root, .providers, .stores, .accounts] {
            let navigator = Navigator()
            navigator.section = .home
            navigator.phoneSettings = page
            NarrowTabs.widen(navigator, .both)
            #expect(navigator.phoneSettings == page, "widening closed the account sheet at \(page)")
            #expect(navigator.section == .home)
        }
        // With a section pushed too, widening brings the section back and leaves the sheet.
        let navigator = Navigator()
        navigator.section = .providers
        NarrowTabs.narrow(navigator, .watch)
        navigator.phoneSettings = .root
        NarrowTabs.widen(navigator, .watch)
        #expect(navigator.section == .providers)
        #expect(navigator.phoneSettings == .root)
    }
}
