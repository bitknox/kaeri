// Asking the engine a question and waiting for the event that answers it: what the notifications
// and the widgets' record need of the engine, whether the app's store holds it or a background
// task started one of its own.

import KaeriKit
import Foundation
import Synchronization

/// Hands each event the engine says to whoever is waiting on one like it.
public final class EngineAnswers: Sendable {
    private let waiting = Mutex<[UUID: @Sendable (Event) -> Bool]>([:])

    public init() {}

    /// Hand `events` to the questions waiting on them.
    public func deliver(_ events: [Event]) {
        for event in events {
            let waiters = waiting.withLock { $0 }
            for (id, take) in waiters where take(event) {
                _ = waiting.withLock { $0.removeValue(forKey: id) }
            }
        }
    }

    /// Hand every event to `take` until `stop` is called with what this returns.
    public func listen(_ take: @escaping @Sendable (Event) -> Void) -> UUID {
        let id = UUID()
        waiting.withLock { $0[id] = { event in take(event); return false } }
        return id
    }

    /// Stop a listener `listen` started.
    public func stop(_ id: UUID) {
        _ = waiting.withLock { $0.removeValue(forKey: id) }
    }

    /// Send what `send` sends, and wait for the first event `match` answers, for at most
    /// `timeout`: `nil` when none came in time, or the task asking was cancelled, as a background
    /// task running out of time is.
    public func ask<Answer: Sendable>(
        timeout: Duration = .seconds(20),
        send: @Sendable () -> Void,
        match: @escaping @Sendable (Event) -> Answer?
    ) async -> Answer? {
        let id = UUID()
        let once = Once<Answer>()
        return await withTaskCancellationHandler {
            await withCheckedContinuation { continuation in
                once.set(continuation)
                // Cancelled before the continuation was there to resume.
                if Task.isCancelled {
                    once.resume(nil)
                    return
                }
                waiting.withLock { waiters in
                    waiters[id] = { event in
                        guard let answer = match(event) else { return false }
                        once.resume(answer)
                        return true
                    }
                }
                send()
                Task { [weak self] in
                    try? await Task.sleep(for: timeout)
                    _ = self?.waiting.withLock { $0.removeValue(forKey: id) }
                    once.resume(nil)
                }
            }
        } onCancel: { [weak self] in
            _ = self?.waiting.withLock { $0.removeValue(forKey: id) }
            once.resume(nil)
        }
    }
}

/// A continuation resumed once, by whichever comes first: the answer or the timeout.
private final class Once<Answer: Sendable>: Sendable {
    private let continuation = Mutex<CheckedContinuation<Answer?, Never>?>(nil)

    func set(_ continuation: CheckedContinuation<Answer?, Never>) {
        self.continuation.withLock { $0 = continuation }
    }

    func resume(_ answer: Answer?) {
        let waiting = continuation.withLock { value -> CheckedContinuation<Answer?, Never>? in
            defer { value = nil }
            return value
        }
        waiting?.resume(returning: answer)
    }
}

/// The questions the notifications and the widgets ask the engine, over `send` and the answers
/// `answers` is handed.
public struct EngineQuestions: Sendable {
    let answers: EngineAnswers
    let send: @Sendable (Command) -> Void

    public init(answers: EngineAnswers, send: @escaping @Sendable (Command) -> Void) {
        self.answers = answers
        self.send = send
    }

    /// What aired since the last check, each said once (`CheckNewEpisodes`). The engine answers
    /// within 25 seconds, 15 for AniList and 8 for the dubs, and keeps what it said as said before
    /// it answers, so the wait outlasts that: an answer after it would be lost for good.
    public func newEpisodes() async -> [NewEpisode]? {
        await answers.ask(timeout: .seconds(30), send: { send(.checkNewEpisodes) }) { event in
            if case .newEpisodes(let said) = event { return said }
            return nil
        }
    }

    /// The chapters to notify of, from the paced check of the followed series
    /// (`ReadingCommand::CheckUpdates`): `nil` when it did not finish within `timeout`. The
    /// engine says Updates with `checking` while it runs and again once it is done, and the
    /// chapters right after that; only while the person reads and New chapters is on, so with
    /// either off this is empty, and at once when reading is off.
    public func newChapters(timeout: Duration = .seconds(120), settle: Duration = .seconds(1)) async -> [NewChapter]? {
        let found = Mutex<[NewChapter]>([])
        let listener = answers.listen { event in
            if case .reading(.newChapters(let chapters)) = event {
                found.withLock { $0 += chapters }
            }
        }
        defer { answers.stop(listener) }
        let started = Mutex(false)
        let ran = await answers.ask(timeout: timeout, send: { send(.reading(.checkUpdates)) }) { event -> Bool? in
            guard case .reading(.updates(let view)) = event else { return nil }
            if view.checking {
                started.withLock { $0 = true }
                return nil
            }
            // Done, or never started: reading is off.
            return started.withLock { $0 }
        }
        guard let ran else { return nil }
        // What it found is sent next, from the same place.
        if ran { try? await Task.sleep(for: settle) }
        return found.withLock { $0 }
    }

    /// The week's notifications (`UpcomingNotices`).
    public func week() async -> UpcomingNotices? {
        await answers.ask(send: { send(.upcomingNotices) }) { event in
            if case .upcomingNotices(let week) = event { return week }
            return nil
        }
    }

    /// The widgets' record (`WidgetSnapshot`).
    public func widgetRecord() async -> WidgetData? {
        await answers.ask(send: { send(.widgetSnapshot) }) { event in
            if case .widgetSnapshot(let data) = event { return data }
            return nil
        }
    }

    /// The bytes of the picture at `url`, from the engine's image cache or fetched into it.
    public func art(_ url: String) async -> Data? {
        await answers.ask(timeout: .seconds(10), send: { send(.fetchArtwork(url: url)) }) { event in
            switch event {
            case .artworkReady(let ready, let bytes) where ready == url: .some(bytes)
            case .imageFailed(let failed, _) where failed == url: .some(nil)
            default: nil
            }
        } ?? nil
    }
}
