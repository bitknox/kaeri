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

    @Test func aTypedCodeIsSentAsPairWithCodeAndShowsTheTryUnderWay() {
        let sync = SyncModel()
        var sent: [Command] = []
        sync.send = { sent.append($0) }
        sync.pairWithCode("042917", deviceName: "Kim’s Mac")
        #expect(sync.pair == .working)
        #expect(sent == [.syncGroup(.pairWithCode(deviceName: "Kim’s Mac", code: "042917"))])
    }

    @Test func aWrongCodeNamesTheDeviceThatTurnedItDownAndNotFoundDoesNot() {
        let sync = SyncModel()
        sync.pairWithCode("042917", deviceName: "Kim’s Mac")
        sync.apply(.syncPairFailed(problem: .wrongCode, device: "Kim’s iPhone"))
        #expect(sync.pair == .failed(.code))
        #expect(sync.pairFailedDevice == "Kim’s iPhone")
        sync.apply(.syncProblem(.unreachable))
        #expect(sync.pair == .failed(.code))
        sync.pairWithCode("042917", deviceName: "Kim’s Mac")
        #expect(sync.pairFailedDevice == nil)
        sync.apply(.syncPairFailed(problem: .notFound, device: nil))
        #expect(sync.pair == .failed(.notFound))
    }

    @Test func closingTheCodeSheetSendsPairStopAndForgetsTheCode() {
        let sync = SyncModel()
        var sent: [Command] = []
        sync.send = { sent.append($0) }
        sync.apply(.syncInvite(code: "042917", port: 47_812, seconds: 120))
        #expect(sync.invite != nil)
        sync.pairSheetClosed()
        #expect(sync.invite == nil)
        #expect(sent == [.pairStop])
    }

    @Test func renamingSendsTheTrimmedNameAndIgnoresAnEmptyOne() {
        let sync = SyncModel()
        var sent: [Command] = []
        sync.send = { sent.append($0) }
        sync.rename("   ")
        #expect(sent.isEmpty)
        sync.rename("  Kim’s Mac mini ")
        #expect(sent == [.syncGroup(.renameDevice(name: "Kim’s Mac mini"))])
        #expect(sync.deviceName == "Kim’s Mac mini")
    }

    @Test func joiningStoresThroughAPairedDeviceSendsItsAddressesAndTheNextPairingAsksWithoutOne() {
        let sync = SyncModel()
        var sent: [Command] = []
        sync.send = { sent.append($0) }
        sync.joinStores(from: nil)
        sync.joinStores(from: "Kitchen iPad", ["store.example"])
        #expect(sent == [
            .syncGroup(.joinStoresFrom(device: nil, stores: [], address: nil)),
            .syncGroup(.joinStoresFrom(device: "Kitchen iPad", stores: ["store.example"], address: nil)),
        ])
    }

    @Test func eachStoreProblemHasItsOwnLineNamingTheStoreAndTheDevice() {
        let lines = [SyncStoreProblem.offline, .refused, .notAllowed].map { SyncWords.storeProblem($0, store: "Hotaru", device: "Kitchen iPad") }
        #expect(Set(lines).count == 3)
        #expect(lines.allSatisfy { $0.contains("Hotaru") })
        #expect(lines[0].contains("Kitchen iPad") && lines[2].contains("Kitchen iPad"))
    }

    @Test func theSwitchesSendTheDevicesNameAndTheNewValue() {
        let sync = SyncModel()
        var sent: [Command] = []
        sync.send = { sent.append($0) }
        let tv = SyncDeviceView(id: "tv", name: "Living room TV", thisDevice: false, remoteKey: "ab12")
        sync.setSync(tv, on: false)
        sync.setRemote(tv, on: false)
        #expect(sent == [
            .syncGroup(.setDeviceSync(device: "Living room TV", on: false)),
            .syncGroup(.setDeviceRemote(device: "Living room TV", on: false)),
        ])
    }

    @Test func theRemoteKeysOfTVsWhoseRemoteIsOffAreCollected() {
        let sync = SyncModel()
        sync.apply(.syncStatus(SyncStatus(enabled: true, devices: [
            SyncDeviceView(id: "a", name: "Living room TV", thisDevice: false, remoteKey: "ab12", remoteOff: true),
            SyncDeviceView(id: "b", name: "Den TV", thisDevice: false, remoteKey: "cd34"),
            SyncDeviceView(id: "c", name: "Kim’s iPhone", thisDevice: false),
        ], syncing: false)))
        #expect(sync.remoteOff == ["ab12"])
    }

    @Test func aPausedDeviceKeepsItsRowAndASyncPassIsWorded() {
        let sync = SyncModel()
        sync.apply(.syncStatus(SyncStatus(enabled: true, devices: [
            SyncDeviceView(id: "me", name: "Kim’s Mac", thisDevice: true),
            SyncDeviceView(id: "b", name: "Kitchen iPad", thisDevice: false, paused: true),
        ], syncing: false)))
        #expect(sync.others.map(\.paused) == [true])
        #expect(sync.syncingPartner == "Kitchen iPad")
    }
}
