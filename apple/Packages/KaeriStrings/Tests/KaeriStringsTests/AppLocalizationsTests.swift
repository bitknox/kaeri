import Foundation
import Testing

@testable import KaeriStrings

/// The apps' Info.plists name every shipped language. The app's own bundle holds no strings, so
/// the system takes this list for the languages the app offers and loads the strings in.
@Suite struct AppLocalizationsTests {
    @Test(arguments: ["Info.plist", "Fixtures-Info.plist"])
    func theAppListsEveryShippedLanguage(plist: String) throws {
        let app = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // KaeriStringsTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // KaeriStrings
            .deletingLastPathComponent() // Packages
            .deletingLastPathComponent() // apple
            .appending(path: "App/\(plist)")
        let info = try #require(try PropertyListSerialization.propertyList(from: Data(contentsOf: app), format: nil) as? [String: Any])
        let listed = try #require(info["CFBundleLocalizations"] as? [String], "\(plist) has no CFBundleLocalizations")
        #expect(listed == L.languages.map(\.tag), "list the languages of L.languages in \(plist), in the same order")
    }
}
