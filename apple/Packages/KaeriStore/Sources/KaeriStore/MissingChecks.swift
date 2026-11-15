// Check again on a title page whose next episode aired and no source lists yet: the press, the
// wait before Checking shows, and what the answer leaves on the page.
//
// The engine relists the title and answers with `Event::EpisodeChecked`. Nothing shows for the
// first 600 ms after a press; past that the button says Checking, and once it has it stays at
// least 0.3 s, so the answer is held until then. The answer and the end of Checking land
// together.

import KaeriKit
import Foundation
import Observation

/// What a check the person asked for came back with, while the episode is still missing.
public enum CheckAnswer: Equatable, Sendable {
    /// The sources answered and none lists it: "Your sources still don't have it."
    case still
    /// No source answered: "Your sources didn't answer."
    case noAnswer
}

/// Something a screen reader hears once, from the page of title `id` when it is open.
public struct CheckSpoken: Equatable, Sendable {
    public enum Kind: Equatable, Sendable {
        /// Checking showed in the button.
        case checking
        case still
        case noAnswer
        /// A check, asked for or quiet, found the episode.
        case found(episode: UInt32)
        /// Pressed while offline: nothing was asked.
        case offline
    }

    public let id: AnilistId
    public let kind: Kind
    /// Tells two of the same words apart, so each one is said.
    public let serial: Int
}

/// The checks of missing episodes: one at a time per title.
@Observable @MainActor
public final class MissingChecks {
    struct Running: Equatable {
        let episode: UInt32
        /// When Checking showed in the button, once it has.
        var shownAt: ContinuousClock.Instant?
    }

    /// The checks under way, by title.
    private(set) var running: [AnilistId: Running] = [:]
    /// What the last check asked of each title came to, and of which episode.
    private(set) var answers: [AnilistId: (episode: UInt32, answer: CheckAnswer)] = [:]
    /// The newest thing to say. The title's page says it, when it is open.
    public private(set) var spoken: CheckSpoken?

    /// How the command reaches the engine: the store's send.
    @ObservationIgnored var send: ((Command) -> Void)?
    /// How long a check runs before Checking shows.
    @ObservationIgnored var showAfter: Duration = .milliseconds(600)
    /// How long Checking stays at least, once it shows.
    @ObservationIgnored var holdFor: Duration = .milliseconds(300)
    @ObservationIgnored var now: () -> ContinuousClock.Instant = { ContinuousClock.now }
    @ObservationIgnored private var serial = 0

    public init() {}

    /// Whether a check of `id` is under way, shown or not yet.
    public func isRunning(_ id: AnilistId) -> Bool {
        running[id] != nil
    }

    /// Whether Check again says Checking now.
    public func showsChecking(_ id: AnilistId) -> Bool {
        running[id]?.shownAt != nil
    }

    /// What the last check asked of `id`'s `episode` came to, if one has come back.
    public func answer(_ id: AnilistId, episode: UInt32) -> CheckAnswer? {
        guard let kept = answers[id], kept.episode == episode else { return nil }
        return kept.answer
    }

    /// Ask the sources again for `id`'s `episode`. A press while a check of the title runs adds
    /// nothing.
    public func checkAgain(_ id: AnilistId, episode: UInt32) {
        guard running[id] == nil else { return }
        running[id] = Running(episode: episode)
        send?(.missing(.checkAgain(id: id, episode: episode)))
        let wait = showAfter
        Task { [weak self] in
            try? await Task.sleep(for: wait)
            guard let self, var run = running[id], run.episode == episode, run.shownAt == nil else { return }
            run.shownAt = now()
            running[id] = run
            say(id, .checking)
        }
    }

    /// The engine's answer: at once, or once Checking has stayed its 0.3 s.
    func apply(_ check: EpisodeCheck) {
        let id = check.id
        guard check.asked else {
            // A quiet recheck: an empty one changes nothing on screen; a find is said.
            if check.outcome == .found {
                answers[id] = nil
                say(id, .found(episode: check.episode))
            }
            return
        }
        if let shownAt = running[id]?.shownAt {
            let left = holdFor - (now() - shownAt)
            if left > .zero {
                Task { [weak self] in
                    try? await Task.sleep(for: left)
                    self?.settle(check)
                }
                return
            }
        }
        settle(check)
    }

    private func settle(_ check: EpisodeCheck) {
        let id = check.id
        running[id] = nil
        switch check.outcome {
        case .found:
            answers[id] = nil
            say(id, .found(episode: check.episode))
        case .missing:
            answers[id] = (check.episode, .still)
            say(id, .still)
        case .noAnswer:
            answers[id] = (check.episode, .noAnswer)
            say(id, .noAnswer)
        case .offline:
            // Nothing was asked: the words stay as they were.
            say(id, .offline)
        }
    }

    private func say(_ id: AnilistId, _ kind: CheckSpoken.Kind) {
        serial += 1
        spoken = CheckSpoken(id: id, kind: kind, serial: serial)
    }
}
