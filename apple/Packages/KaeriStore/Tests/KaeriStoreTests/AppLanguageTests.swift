// The app's language: a pick is kept in the app's own defaults, as the system keeps a per-app
// language, and following the system clears it. Each test has a suite of its own, never the app's.

@testable import KaeriStore
import Foundation
import KaeriStrings
import Testing

struct AppLanguageTests {
    /// A store over a fresh suite, the device's languages as given, and the suite to clear after.
    private func language(system: [String] = ["en-GB"]) -> (AppLanguage, UserDefaults, String) {
        let name = "app.kaeri.tests.language.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        return (AppLanguage(defaults: defaults, domain: name, systemLanguages: { system }), defaults, name)
    }

    private func shipped(_ tag: String) -> L.Language {
        AppLanguage.shipped.first { $0.tag == tag }!
    }

    @Test func nothingIsChosenUntilOneIsPicked() {
        let (language, defaults, name) = language()
        defer { defaults.removePersistentDomain(forName: name) }
        #expect(language.chosen == nil)
    }

    @Test func aPickIsStoredAndReadBack() {
        let (language, defaults, name) = language()
        defer { defaults.removePersistentDomain(forName: name) }
        language.choose(shipped("de"))
        #expect(language.chosen == shipped("de"))
        #expect(defaults.persistentDomain(forName: name)?[AppLanguage.key] as? [String] == ["de"])
        // Another reader of the same defaults sees it, as the next start does.
        #expect(AppLanguage(defaults: UserDefaults(suiteName: name)!, domain: name).chosen == shipped("de"))
        language.choose(shipped("pt-BR"))
        #expect(language.chosen == shipped("pt-BR"))
    }

    @Test func followingTheSystemClearsThePick() {
        let (language, defaults, name) = language(system: ["sv-SE", "en-GB"])
        defer { defaults.removePersistentDomain(forName: name) }
        language.choose(shipped("ja"))
        #expect(language.next == shipped("ja"))
        language.choose(nil)
        #expect(language.chosen == nil)
        #expect(defaults.persistentDomain(forName: name)?[AppLanguage.key] == nil)
        #expect(language.next == shipped("sv"), "the device's own language again")
    }

    @Test func aTagTheSystemWroteMatchesByLanguage() {
        #expect(AppLanguage.match("de-AT") == shipped("de"))
        #expect(AppLanguage.match("pt") == shipped("pt-BR"))
        #expect(AppLanguage.match("es-419") == shipped("es-419"))
        #expect(AppLanguage.match("da") == nil, "only a shipped language counts")
    }

    @Test func anUnshippedLanguageInTheDefaultsIsNotAChoice() {
        let (language, defaults, name) = language()
        defer { defaults.removePersistentDomain(forName: name) }
        defaults.set(["da"], forKey: AppLanguage.key)
        #expect(language.chosen == nil)
        #expect(language.next == shipped("en"))
    }

    @Test func aRestartIsNeededOnlyWhenTheTextWouldChange() {
        let (language, defaults, name) = language(system: ["en-GB"])
        defer { defaults.removePersistentDomain(forName: name) }
        let english = shipped("en")
        #expect(!language.needsRestart(showing: english))
        language.choose(shipped("fr"))
        #expect(language.needsRestart(showing: english))
        language.choose(english)
        #expect(!language.needsRestart(showing: english), "English picked while English shows")
        language.choose(nil)
        #expect(!language.needsRestart(showing: english))
        #expect(language.needsRestart(showing: shipped("fr")), "back to the system's English after French")
    }
}
