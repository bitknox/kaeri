// The app's language. Apple keeps one language for each app, `AppleLanguages` in the app's own
// defaults, which the system's own per-app setting writes too. The system reads it when the app
// starts: the strings, and the dates and numbers through `Locale.current`, follow it from the next
// start (docs/native-ux.md, Language).

import Foundation
import KaeriStrings

public struct AppLanguage {
    /// The key the system reads the app's languages from.
    public static let key = "AppleLanguages"

    private let defaults: UserDefaults
    private let domain: String?
    private let systemLanguages: () -> [String]

    /// The app's own defaults by default. Tests pass a suite of their own and its name.
    /// `systemLanguages` is the person's list for the whole device, read when following it.
    public init(
        defaults: UserDefaults = .standard,
        domain: String? = Bundle.main.bundleIdentifier,
        systemLanguages: @escaping () -> [String] = { Locale.preferredLanguages }
    ) {
        self.defaults = defaults
        self.domain = domain
        self.systemLanguages = systemLanguages
    }

    /// The languages this build ships, source first. A draft one is labelled as such.
    public static var shipped: [L.Language] { L.languages }

    /// The chosen language, or `nil` to follow the system's. Only the app's own domain counts, so a
    /// launch argument or the device's list is never taken for a choice. Only a shipped language counts.
    public var chosen: L.Language? {
        guard let domain,
              let tags = defaults.persistentDomain(forName: domain)?[Self.key] as? [String],
              let tag = tags.first
        else { return nil }
        return Self.match(tag)
    }

    /// Choose `language`, or `nil` to follow the system and clear the app's own choice.
    public func choose(_ language: L.Language?) {
        if let language {
            defaults.set([language.tag], forKey: Self.key)
        } else {
            defaults.removeObject(forKey: Self.key)
        }
    }

    /// The language the text is in from the next start: the chosen one, or the system's best
    /// match among the shipped.
    public var next: L.Language {
        chosen ?? Self.best(systemLanguages())
    }

    /// Whether the text changes language at the next start.
    public func needsRestart(showing: L.Language = AppLanguage.showing) -> Bool {
        next != showing
    }

    /// The language the text is in now, which the system picked when the app started.
    public static var showing: L.Language {
        best(Bundle.main.preferredLocalizations)
    }

    /// The shipped language the system would pick for a list of preferred languages.
    public static func best(_ preferences: [String]) -> L.Language {
        Bundle.preferredLocalizations(from: shipped.map(\.tag), forPreferences: preferences)
            .lazy.compactMap(match).first ?? shipped[0]
    }

    /// The shipped language a tag names: the same tag, else the same language (`de-AT` is `de`).
    public static func match(_ tag: String) -> L.Language? {
        let wanted = Locale.Language(identifier: tag)
        return shipped.first { Locale.Language(identifier: $0.tag) == wanted }
            ?? shipped.first { Locale.Language(identifier: $0.tag).languageCode == wanted.languageCode }
    }
}
