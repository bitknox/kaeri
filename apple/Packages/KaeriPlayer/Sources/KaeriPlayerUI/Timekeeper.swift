// The player's time: what it reads as now, and how it waits for a while to pass. The apps run on
// the wall clock; a test runs the model on a clock of its own and moves it by hand, so what shows
// after a while is checked without waiting for it, however slow the machine running the test.

import Foundation

public struct Timekeeper: Sendable {
    /// Now, as the cards' timing reads it.
    public let now: @Sendable () -> ContinuousClock.Instant
    /// A wait of the given length, which throws once cancelled.
    public let sleep: @Sendable (Duration) async throws -> Void

    public init(
        now: @escaping @Sendable () -> ContinuousClock.Instant,
        sleep: @escaping @Sendable (Duration) async throws -> Void
    ) {
        self.now = now
        self.sleep = sleep
    }

    /// The wall clock.
    public static let wall = Timekeeper(now: { .now }, sleep: { try await Task.sleep(for: $0) })
}

#if FIXTURES
import KaeriKit

extension Timekeeper {
    /// The engine's still clock, for a still shot (`-KaeriStillAt <seconds>`, `KAERI_STILL_AT`):
    /// now is where the shot holds the clock, and a wait ends once the clock has moved past it,
    /// however long that takes on the wall clock. Before the engine turns the clock on it reads
    /// nought.
    public static func still() -> Timekeeper {
        let base = ContinuousClock.now
        @Sendable func now() -> ContinuousClock.Instant {
            base + .milliseconds(Int64(clamping: testingStillClockNowMs() ?? 0))
        }
        return Timekeeper(now: now) { length in
            let end = now() + length
            while now() < end {
                try await Task.sleep(for: .milliseconds(20))
            }
        }
    }

    /// The still clock, held at the seconds the launch names (`-KaeriStillAt` or
    /// `KAERI_STILL_AT`), or nil for a launch on the wall clock. Call it before the engine starts,
    /// so the scene's made-up waits run on the clock from the first.
    public static func stillFromLaunch() -> Timekeeper? {
        let arguments = ProcessInfo.processInfo.arguments
        let seconds = arguments.firstIndex(of: "-KaeriStillAt").flatMap {
            arguments.indices.contains($0 + 1) ? arguments[$0 + 1] : nil
        } ?? ProcessInfo.processInfo.environment["KAERI_STILL_AT"]
        guard let seconds, testingStillClockHoldSeconds(seconds: seconds) else { return nil }
        return .still()
    }
}
#endif
