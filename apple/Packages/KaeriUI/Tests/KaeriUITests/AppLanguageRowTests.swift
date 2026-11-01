// The app-language row: a pick from its menu is stored and read back, following the system clears
// it, and the row says what the pick does. A suite of its own stands in for the app's defaults.

import Foundation
import KaeriStore
import KaeriStrings
@testable import KaeriUI
import SwiftUI
import Testing

@MainActor
struct AppLanguageRowTests {
    private func shipped(_ tag: String) -> L.Language {
        AppLanguage.shipped.first { $0.tag == tag }!
    }

    @Test func aPickFromTheMenuIsStoredAndFollowingTheSystemClearsIt() {
        let name = "app.kaeri.tests.language-row.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defer { defaults.removePersistentDomain(forName: name) }
        let language = AppLanguage(defaults: defaults, domain: name, systemLanguages: { ["en-GB"] })
        var chosen: L.Language?
        let menu = AppLanguageMenu(chosen: Binding(get: { chosen }, set: { chosen = $0 }), language: language) { EmptyView() }

        menu.pick("sv")
        #expect(chosen == shipped("sv"))
        #expect(language.chosen == shipped("sv"))
        #expect(AppLanguage(defaults: defaults, domain: name).chosen == shipped("sv"), "read back as the next start reads it")

        menu.pick(nil)
        #expect(chosen == nil)
        #expect(language.chosen == nil)
        #expect(defaults.persistentDomain(forName: name)?[AppLanguage.key] == nil, "the override is gone")
    }

    @Test func theMenuListsFollowingTheSystemThenEveryShippedLanguage() {
        #expect(AppLanguageWords.value(nil) == "Follow system language")
        #expect(AppLanguage.shipped.first == shipped("en"))
        #expect(AppLanguageWords.name(shipped("en")) == "English")
        #expect(AppLanguageWords.name(shipped("de")) == "Deutsch · Draft translation")
        #expect(AppLanguageWords.value(shipped("de"), labelDraft: false) == "Deutsch", "the iPhone leaves the label to the note")
        #expect(AppLanguage.shipped.allSatisfy { $0.flag.map { LanguageFlag.menuIcon($0) != nil } ?? true }, "every flag draws in the menu")
    }

    @Test func theNoteSaysWhenTheTranslationIsADraftAndWhenTheLanguageChanges() {
        #expect(AppLanguageWords.note(next: shipped("en"), restart: false) == nil)
        #expect(AppLanguageWords.note(next: shipped("ja"), restart: false) == L.languageDraftNote)
        #if os(macOS)
        #expect(AppLanguageWords.note(next: shipped("en"), restart: true) == "Restart Kaeri to use the new language.")
        #else
        #expect(AppLanguageWords.note(next: shipped("en"), restart: true) == "Kaeri uses the new language the next time it opens.")
        #endif
        #expect(AppLanguageWords.note(next: shipped("ja"), restart: true) == L.languageDraftNote + "\n" + AppLanguageWords.restartNote)
    }

    @Test func searchingSettingsFindsTheRow() {
        #expect(SettingsResults.index.contains { $0.title == "App language" && $0.pane == .general })
    }
}
