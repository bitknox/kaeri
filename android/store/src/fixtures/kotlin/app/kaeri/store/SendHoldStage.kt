// A send held for Wi-Fi, staged for shots on the fixtures engine. Its scenes' chapters are files
// on the device, which go at once, and no source in them fetches over a network, so no send of
// theirs ever waits. The engine's hold is covered by its own tests.

package app.kaeri.store

object SendHoldStage {
    /** Every send prepared from now on is held for Wi-Fi rather than asked of the engine; false puts the engine's back. */
    fun stage(store: AppStore, on: Boolean = true) {
        store.export.holdShot = on
    }
}
