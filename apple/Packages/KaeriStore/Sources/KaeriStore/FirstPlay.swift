// First play: where the demo stands, and the crash reports kept on this device. The engine owns
// both; these models hold what it last said so the screens can read it.

import KaeriKit
import Foundation
import Observation

/// Where the built-in demo stands: on or off, offered, the only source, played.
@Observable @MainActor
public final class DemoModel {
    public private(set) var state = DemoState()

    public init() {}

    func apply(_ state: DemoState) {
        self.state = state
    }

    /// Who made the demo, its licence and the hosts it reaches. The engine's constants, so the
    /// screens can name nothing the demo does not reach.
    public var info: DemoInfo { demoInfo() }
}

/// The crash reports: the saved ones, the ones the system still holds, and the one to mention.
@Observable @MainActor
public final class CrashModel {
    /// The saved reports, newest first.
    public private(set) var saved: [CrashSummary] = []
    /// What the system kept from before saving was switched on, offered and not yet added.
    public private(set) var held: [CrashSummary] = []
    /// The crash the notice mentions, once.
    public private(set) var pending: CrashSummary?
    /// The crash the last notice named, which Report… opens the sheet with ticked.
    public private(set) var noticed: CrashSummary?

    /// Reads the system's own records for the held offer: set by the app, which owns MetricKit.
    @ObservationIgnored public var readSystemRecords: (@MainActor () -> [CrashRecord])?

    public init() {}

    /// Read the saved list again.
    public func refresh() {
        saved = crashList().filter { $0.origin == .saved }
    }

    /// At launch, once the first content is up: the crash to mention, when saving and asking are on.
    public func checkNotice() -> CrashSummary? {
        pending = crashPendingNotice()
        if let pending { noticed = pending }
        refresh()
        return pending
    }

    public func clearPending() {
        pending = nil
    }

    /// Saving was just switched on: offer what the system holds from the week before.
    public func offerHeld() {
        guard let records = readSystemRecords?(), !records.isEmpty else {
            held = []
            return
        }
        held = crashOfferHeld(records: records)
    }

    public func forgetHeld() {
        held = []
    }

    public func add(_ summary: CrashSummary) {
        if crashAddHeld(id: summary.id) {
            held.removeAll { $0.id == summary.id }
            refresh()
        }
    }

    public func delete(_ id: String) {
        crashDelete(id: id)
        refresh()
    }

    public func deleteAll() {
        crashDeleteAll()
        refresh()
    }

    /// The whole text of a saved report, as the view shows it.
    public func text(_ id: String) -> String {
        crashText(id: id) ?? ""
    }
}
