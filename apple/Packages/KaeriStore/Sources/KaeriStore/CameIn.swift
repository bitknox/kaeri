// What came in after a source was added: a book server, a folder or a plugin store, or why
// nothing did. The engine says it with `Event.cameIn`; this holds the last answer for the sheet
// that asked, and the 600 ms rule for the waiting line.

import KaeriKit
import Foundation
import Observation

@Observable @MainActor
public final class CameInModel {
    /// The last answer: what came in, or why nothing did.
    public private(set) var outcome: CameIn?
    /// A look at an address is under way.
    public private(set) var looking = false
    /// The look has taken 600 ms: the waiting line shows, and then stays at least 0.3 s.
    public private(set) var slow = false
    /// Plugins asked to install one by one, to say Installed beside them.
    public private(set) var installed: Set<String> = []
    /// Servers added this session: their row says so for a while.
    public private(set) var justAdded: Set<String> = []
    @ObservationIgnored var send: ((Command) -> Void)?
    @ObservationIgnored private var slowTask: Task<Void, Never>?

    public init() {}

    /// Look at an address, a pasted Kavita link included, and sign in when a user is given.
    public func probe(address: String, user: String?, password: String?) {
        outcome = nil
        looking = true
        slow = false
        slowTask?.cancel()
        slowTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(600))
            guard !Task.isCancelled else { return }
            self?.slow = self?.looking ?? false
        }
        send?(.bookSources(.probe(address: address, user: user, password: password)))
    }

    /// Add the server the look found, with the libraries ticked.
    public func addServer(_ found: ServerFound, libraries: [String], reportProgress: Bool) {
        send?(.bookSources(.addServer(pending: found.pending, libraries: libraries, reportProgress: reportProgress)))
    }

    public func install(store: String, plugin: String) {
        installed.insert(plugin)
        send?(.installPlugin(store: store, plugin: plugin))
    }

    public func installAll(store: String) { send?(.installAllForReading(store: store)) }

    /// Look at servers that showed no series, now.
    public func recheck() { send?(.bookSources(.recheck)) }

    /// Forget the answer: a sheet opens, or closes.
    public func reset() {
        outcome = nil
        looking = false
        slow = false
        slowTask?.cancel()
        installed = []
    }

    func apply(_ event: Event) {
        guard case .cameIn(let answer) = event else { return }
        if case .serverAdded(let added) = answer { justAdded.insert(added.id) }
        // The waiting line, once shown, stays 0.3 s: the answer waits for it.
        let held = slow
        looking = false
        slowTask?.cancel()
        if held {
            Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(300))
                self?.outcome = answer
                self?.slow = false
            }
        } else {
            outcome = answer
            slow = false
        }
    }

    /// For shots: an answer as the engine would send it.
    public func stage(_ answer: CameIn?, looking: Bool = false) {
        outcome = answer
        if case .serverAdded(let added)? = answer { justAdded.insert(added.id) }
        self.looking = looking
        slow = looking
    }
}
