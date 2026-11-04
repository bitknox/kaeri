// A source or store removed while its page shows goes Back to its list. A page that opened before
// its thing loaded, or for a thing this device does not have, stays.

@testable import KaeriUI
import KaeriKit
import KaeriStore
import Testing

@MainActor
struct SourcePageRemovedTests {
    private func provider(_ id: String) -> ProviderRow {
        ProviderRow(id: id, kind: "plugin", state: "ready", latencyMs: 320, lastError: nil, usable: true,
                    heldBack: false, name: id, version: nil, hosts: [], origin: .byHand, disabled: false,
                    update: nil)
    }

    @Test func removingTheShownThingLeavesItsPage() {
        let shown = PagePresence(key: "alpha", present: true)
        #expect(PagePresence.removed(from: shown, to: PagePresence(key: "alpha", present: false)))
    }

    @Test func aPageWaitingForItsThingStays() {
        // Opened from a launch argument or a store's list before the thing is there.
        let waiting = PagePresence(key: "alpha", present: false)
        #expect(!PagePresence.removed(from: waiting, to: waiting))
        #expect(!PagePresence.removed(from: waiting, to: PagePresence(key: "alpha", present: true)))
    }

    @Test func openingAnotherPageIsNotARemoval() {
        let shown = PagePresence(key: "alpha", present: true)
        #expect(!PagePresence.removed(from: shown, to: PagePresence(key: "beta", present: false)))
        #expect(!PagePresence.removed(from: shown, to: PagePresence(key: nil, present: false)))
    }

    @Test func aPluginPageSeesItsPluginGo() {
        let store = AppStore()
        store.apply(.providers([provider("alpha"), provider("beta")]))
        #expect(store.sourcePageExists("alpha"))
        let before = PagePresence(key: "alpha", present: store.sourcePageExists("alpha"))
        store.apply(.providers([provider("beta")]))
        let after = PagePresence(key: "alpha", present: store.sourcePageExists("alpha"))
        #expect(PagePresence.removed(from: before, to: after))
        #expect(store.sourcePageExists(SourceKey.licensed), "Licensed services has no Remove")
        #expect(!store.sourcePageExists(SourceKey.server("gone")))
        #expect(!store.sourcePageExists(SourceKey.folder("/Volumes/Gone")))
    }
}
