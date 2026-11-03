// What the sync model does with the engine's events: which cloud services it offers, how a status
// and a problem change what the screens show.

@testable import KaeriStore
import KaeriKit
import Testing

@MainActor
struct SyncModelTests {
    @Test func aServiceWithoutAClientIdIsHiddenAndOneThatNeedsAPhoneIsKept() {
        let sync = SyncModel()
        sync.apply(.syncStatus(SyncStatus(enabled: false, syncing: false, clouds: [
            SyncCloudView(service: .dropbox, way: .unavailable(reason: .noClientId), signedIn: false),
            SyncCloudView(service: .googleDrive, way: .unavailable(reason: .needsPhone), signedIn: false),
            SyncCloudView(service: .oneDrive, way: .sheet, signedIn: false),
        ])))
        #expect(sync.offered.map(\.service) == [.googleDrive, .oneDrive])
    }

    @Test func aFolderThatHoldsAnotherGroupOpensTheRestoreSheet() {
        let sync = SyncModel()
        sync.choose(.googleDrive, deviceName: "Kim’s Mac")
        sync.apply(.syncProblem(.groupExists))
        #expect(sync.sheet == .restore(.googleDrive))
    }

    @Test func aStagedStatusIsNotReplacedByTheEngine() {
        let sync = SyncModel()
        sync.staged = true
        sync.stage(status: SyncStatus(enabled: true, syncing: false))
        sync.apply(.syncStatus(SyncStatus(enabled: false, syncing: false)))
        #expect(sync.enabled)
    }

    @Test func theKeySheetOpensWithTheKeyAndForgetsItWhenDone() {
        let sync = SyncModel()
        sync.apply(.syncRecoveryKey("K7QM4XR9TB2HWN6CP3ZD8FLA"))
        #expect(sync.sheet == .recoveryKey)
        sync.keySaved()
        #expect(sync.recoveryKey == nil && sync.sheet == nil)
    }
}
