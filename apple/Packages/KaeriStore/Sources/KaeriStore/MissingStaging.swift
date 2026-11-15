// The missing-episode scene's shots: Check again pressed before the Fixtures app or the dev
// harness says it is ready.

#if FIXTURES
import Foundation
import KaeriKit

extension AppStore {
    /// `-KaeriCheckAgain <1|2|checking|clip>` on the missing-episode scene's title page
    /// (`-KaeriTitle 900002`): Check again pressed once or twice, each answer waited for, or once
    /// and held on Checking, or (`clip`) pressed twice after the scene is ready, 2 s apart from
    /// each answer, for the recording of the episode being found.
    public static let checkAgainArgument = "-KaeriCheckAgain"

    /// Press Check again as the arguments say. False when the page never showed a missing
    /// episode or a check never came back.
    public func stageCheckAgain(arguments: [String] = ProcessInfo.processInfo.arguments) async -> Bool {
        guard let at = arguments.firstIndex(of: Self.checkAgainArgument), arguments.indices.contains(at + 1),
              let titleAt = arguments.firstIndex(of: "-KaeriTitle"), arguments.indices.contains(titleAt + 1),
              let id = AnilistId(arguments[titleAt + 1]) else { return true }
        let mode = arguments[at + 1]
        guard let episode = await missingEpisode(of: id) else { return false }
        switch mode {
        case "checking":
            // Checking stays until the shot is taken.
            missing.holdFor = .seconds(120)
            missing.checkAgain(id, episode: episode)
            return await waitFor { self.missing.showsChecking(id) }
        case "clip":
            Task { @MainActor in
                for _ in 0..<2 {
                    try? await Task.sleep(for: .seconds(2))
                    missing.checkAgain(id, episode: episode)
                    _ = await waitFor { !self.missing.isRunning(id) }
                }
            }
            return true
        default:
            for _ in 0..<(Int(mode) ?? 1) {
                missing.checkAgain(id, episode: episode)
                guard await waitFor({ !self.missing.isRunning(id) }) else { return false }
            }
            // The crossfade to Play, or the tail's.
            try? await Task.sleep(for: .milliseconds(400))
            return true
        }
    }

    /// The first aired episode of `id` no source lists, once its table is in.
    private func missingEpisode(of id: AnilistId) async -> UInt32? {
        var number: UInt32?
        _ = await waitFor {
            number = self.titles.episodes[id]?.first { $0.unsourced }.flatMap { UInt32($0.number) }
            return number != nil
        }
        return number
    }

    private func waitFor(seconds: Double = 15, _ done: () -> Bool) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(seconds)
        while !done() {
            guard ContinuousClock.now < deadline else { return false }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return true
    }
}
#endif
