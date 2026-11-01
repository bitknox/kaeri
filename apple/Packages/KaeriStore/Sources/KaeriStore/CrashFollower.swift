// Decides when the system's crash records are subscribed to: only while saving is on.

import Foundation
import Observation

/// Calls `subscribe` while `isOn` is true and `unsubscribe` when it turns false, each once per
/// change. With saving off, as it is by default, neither is ever called.
@MainActor
public final class CrashFollower {
    private let isOn: () -> Bool
    private let subscribe: () -> Void
    private let unsubscribe: () -> Void
    public private(set) var subscribed = false

    public init(isOn: @escaping () -> Bool, subscribe: @escaping () -> Void, unsubscribe: @escaping () -> Void) {
        self.isOn = isOn
        self.subscribe = subscribe
        self.unsubscribe = unsubscribe
        follow()
    }

    private func follow() {
        let on = withObservationTracking {
            isOn()
        } onChange: { [weak self] in
            Task { @MainActor in self?.follow() }
        }
        if on, !subscribed {
            subscribe()
            subscribed = true
        } else if !on, subscribed {
            unsubscribe()
            subscribed = false
        }
    }
}
