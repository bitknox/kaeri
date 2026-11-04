// What the sync screens say about joining a plugin store through a paired device.

import KaeriKit
import KaeriStrings

public enum SyncWords {
    /// The line for a store that was not joined: `store` is the store's name, `device` the paired one.
    public static func storeProblem(_ problem: SyncStoreProblem, store: String, device: String) -> String {
        switch problem {
        case .offline: L.syncStoreErrOffline(store: store, device: device)
        case .refused: L.syncStoreErrRefused(store: store)
        case .notAllowed: L.syncStoreErrNotAllowed(store: store, device: device)
        }
    }
}
