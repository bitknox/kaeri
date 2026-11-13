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

    @Test func sidebarOnlySectionsOpenTheAccountSheetInANarrowWindow() throws {
        for answer in [KaeriIsFor.watch, .read, .both] {
            let first = try #require(NarrowTabs.barTabs(answer).first)
            let pages: [(AppSection, PhoneSettingsPage)] = [(.providers, .providers), (.pluginStores, .stores), (.settings, .root)]
            for (section, page) in pages {
                let to = try #require(NarrowTabs.narrow(section, answer), "\(section) has no place in a narrow window")
                #expect(to.tab == first)
                #expect(to.pushed == .settings(page))
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

    @Test func aWideWindowGoesBackFromTheAccountSheet() {
        #expect(NarrowTabs.widen(.home, downloads: false, place: nil, settings: .providers, .both) == .providers)
        #expect(NarrowTabs.widen(.read, downloads: false, place: nil, settings: .stores, .read) == .pluginStores)
        #expect(NarrowTabs.widen(.home, downloads: false, place: nil, settings: .root, .watch) == .settings)
        // A page the sidebar has no section for stays in the sheet.
        #expect(NarrowTabs.widen(.home, downloads: false, place: nil, settings: .accounts, .both) == nil)
    }
}
