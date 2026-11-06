// The reader staged for a screenshot: the Fixtures app and the dev harness open the reading
// scene's work, wait for its pages, and put the chrome or a sheet up as the arguments say.

#if FIXTURES
import Foundation

extension ReaderModel {
    /// `-KaeriReader chrome|settings|chapters|ask` after a reader scene.
    public static let argument = "-KaeriReader"

    /// The work a reader scene reads.
    public static func shotWork(arguments: [String]) -> String? {
        guard let at = arguments.firstIndex(where: { $0 == "-KaeriScene" || $0 == "--scene" }), arguments.indices.contains(at + 1) else { return nil }
        switch arguments[at + 1].lowercased() {
        case "readermangartl", "readermoved": return "a:900301"
        case "readernovel": return "a:900302"
        case "readerstrip": return "a:900303"
        default: return nil
        }
    }

    /// Open the scene's work and wait until it has pages to show. False when it never did.
    public func stage(arguments: [String] = ProcessInfo.processInfo.arguments) async -> Bool {
        guard let work = Self.shotWork(arguments: arguments) else { return true }
        open(work: work)
        let deadline = ContinuousClock.now + .seconds(15)
        while ContinuousClock.now < deadline {
            if phase == .open, novel != nil || !pages.isEmpty { break }
            try? await Task.sleep(for: .milliseconds(100))
        }
        try? await Task.sleep(for: .milliseconds(1500))
        let shown = phase == .open
        if let at = arguments.firstIndex(of: Self.argument), arguments.indices.contains(at + 1) {
            switch arguments[at + 1] {
            case "chrome": chromeShown = true
            case "settings": chromeShown = true; sheet = .settings
            case "chapters": chromeShown = true; sheet = .chapters
            case "ask": chromeShown = true; askingPage = true
            default: break
            }
            try? await Task.sleep(for: .milliseconds(900))
        }
        return shown
    }
}
#endif
