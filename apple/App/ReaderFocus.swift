// The reader's Focus filter. A person's Focus can turn on Paper mode in Kaeri: the filter is
// added to a Focus in Settings, and the reader reads its switch the next time it draws a page.
// App Intents reads its titles at build time, so they are literals here, as in Intents.swift.

#if os(iOS)
import AppIntents
import KaeriStore

struct ReaderFocusFilter: SetFocusFilterIntent {
    static let title: LocalizedStringResource = "Reading"
    static let description = IntentDescription("Turns on Paper mode in the reader while this Focus is on.")

    /// Off by default, so a Focus that ends puts the reader back as it was.
    @Parameter(title: "Paper mode", default: false) // i18n-skip
    var paper: Bool

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "Reading") // i18n-skip
    }

    func perform() async throws -> some IntentResult {
        UserDefaults.standard.set(paper, forKey: ReaderModel.focusKey)
        return .result()
    }
}
#endif
