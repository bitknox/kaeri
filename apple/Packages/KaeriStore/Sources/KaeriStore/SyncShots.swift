// States of sync for screenshots, staged without an account or a network: the Fixtures app and the
// dev harness say `-KaeriSync <state>`, and the model takes the status, problem and sheet named.

import Foundation
import KaeriKit

/// The states `-KaeriSync` stages. Settings opens at Devices with `-KaeriPane devices`.
///
///     off            sync off (the pane's own start)
///     on             a group with three devices and Google Drive, synced
///     syncing        the same, past 600 ms of a pass
///     settingup      the folder being made after sign-in
///     signedout      Google Drive signed out by the person
///     signedout-err  Google Drive signed the app out
///     offline        the folder cannot be reached
///     folder-gone    a local folder that is gone
///     road           the road sheet; picker, picker-denied, nextcloud, nextcloud-wait, nextcloud-err,
///                    webdav, webdav-err, key, key-qr, restore, restore-wrong, pair-show, pair-enter,
///                    pair-err, pair-nf, paired, profile-add, profile-edit are the sheets, over `on`
///     first-setup    first launch's code page (with sheet=welcome-setup); first-paired its paired page
///     pair-motion, first-motion  the code page, then the paired page after 4 s, for recordings
///     alone          sync on with no other device yet
///     paused         the group with Kitchen iPad's Sync off and the TV's Remote off
///     pair-wrong     a typed code a named device turned down; pair-working the try under way
///     first-paired-store  first launch's paired page with a plugin store to join
///     profiles       two profiles
public enum SyncScene: String, Sendable {
    case off, on, syncing, settingup, signedout, signedoutErr = "signedout-err", offline
    case folderGone = "folder-gone", road, picker, pickerDenied = "picker-denied", nextcloud
    case nextcloudWait = "nextcloud-wait", nextcloudErr = "nextcloud-err", webdav, webdavErr = "webdav-err"
    case key, keyQr = "key-qr", restore, restoreWrong = "restore-wrong"
    case pairShow = "pair-show", pairEnter = "pair-enter", pairErr = "pair-err", pairNf = "pair-nf", paired
    case firstSetup = "first-setup", firstPaired = "first-paired"
    case profiles, profileAdd = "profile-add", profileEdit = "profile-edit"
    case pairMotion = "pair-motion", firstMotion = "first-motion", alone, paused, pairWrong = "pair-wrong", pairWorking = "pair-working", firstPairedStore = "first-paired-store"

    public static let argument = "-KaeriSync"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> SyncScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1)
        else { return nil }
        return SyncScene(rawValue: arguments[index + 1])
    }
}

extension SyncModel {
    private static func clouds(offered: Bool = true) -> [SyncCloudView] {
        [
            SyncCloudView(service: .dropbox, way: .sheet, signedIn: false),
            SyncCloudView(service: .googleDrive, way: .sheet, signedIn: false),
            SyncCloudView(service: .oneDrive, way: .sheet, signedIn: false),
            SyncCloudView(service: .nextcloud, way: .browser, signedIn: false),
        ]
    }

    /// The staged TV's remote key, which the Fixtures app counts as paired.
    static let tvKey = "ab12cd34ef56ab12cd34ef56ab12cd34"

    private static func devices(now: Int64, paused: Bool = false, remoteOff: Bool = false) -> [SyncDeviceView] {
        [
            SyncDeviceView(id: "me", name: DeviceName.this, thisDevice: true, lastSeen: now),
            SyncDeviceView(id: "kim-phone", name: "Kim’s iPhone", thisDevice: false, lastSeen: now - 20, kind: .phone), // i18n-skip
            SyncDeviceView(
                id: "living-room", name: "Living room TV", thisDevice: false, lastSeen: now - 120, kind: .tv, // i18n-skip
                paused: false, remoteKey: Self.tvKey, remoteOff: remoteOff),
            SyncDeviceView(
                id: "kitchen", name: "Kitchen iPad", thisDevice: false, lastSeen: now - 31 * 86_400, // i18n-skip
                kind: .tablet, paused: paused),
        ]
    }

    private static func group(
        now: Int64, syncing: Bool = false, problem: SyncProblem? = nil, folder: SyncFolderKind = .googleDrive,
        account: String? = "kim@mail.example", notes: Bool = false, held: Bool = false,
        stores: [SyncStoreOffer] = []
    ) -> SyncStatus {
        SyncStatus(
            enabled: true, deviceName: DeviceName.this,
            folder: SyncFolderView(kind: folder, label: folder == .directory ? "Kaeri sync" : "", account: account), // i18n-skip
            devices: devices(now: now, paused: held, remoteOff: held), lastSynced: now - 240, syncing: syncing,
            notes: notes ? [SyncNoteView(id: 1, anilistId: 900001, episode: "5", watchDevice: "Living room TV", unwatchDevice: "Kim’s MacBook Air", at: now - 3600)] : [], // i18n-skip
            storesToJoin: stores,
            clouds: clouds(), problem: problem, groupFolder: folder, recoveryKey: true,
            recoveryKeyMade: now - 86_400
        )
    }

    /// Put `scene` on screen: the status it names, and the sheet.
    public func stage(_ scene: SyncScene, store: AppStore) async -> Bool {
        staged = true
        let now = Int64(Date.now.timeIntervalSince1970)
        deviceName = DeviceName.this
        var status = SyncStatus(enabled: false, syncing: false, clouds: Self.clouds())
        var sheet: SyncSheet?
        switch scene {
        case .off, .firstSetup, .firstPaired, .firstMotion: break
        case .paused:
            status = Self.group(now: now, notes: true, held: true)
        case .alone:
            status = Self.group(now: now)
            status.devices = status.devices.filter(\.thisDevice)
            status.notes = []
        case .firstPairedStore:
            status = Self.group(now: now, stores: [SyncStoreOffer(address: "store.example", name: "Hotaru", inviteFrom: "Kitchen iPad")]) // i18n-skip
        case .pairWrong, .pairWorking, .pairMotion:
            status = Self.group(now: now, notes: true)
        case .on, .key, .keyQr, .pairShow, .pairEnter, .pairErr, .pairNf, .paired, .profiles,
             .profileAdd, .profileEdit, .picker, .pickerDenied, .nextcloud, .nextcloudWait, .nextcloudErr,
             .webdav, .webdavErr, .restore, .restoreWrong, .road:
            status = Self.group(now: now, notes: true)
        case .syncing:
            status = Self.group(now: now, syncing: true)
            syncingShown = true
            syncingSince = .now - .seconds(4)
        case .settingup:
            status = Self.group(now: now, syncing: true)
            stage(settingUp: .googleDrive)
            settingUpShown = true
        case .signedout:
            status = Self.group(now: now, problem: .signInNeeded(service: .googleDrive))
            stage(problem: .signInNeeded(service: .googleDrive), byHand: true)
        case .signedoutErr:
            status = Self.group(now: now, problem: .signInNeeded(service: .googleDrive))
            stage(problem: .signInNeeded(service: .googleDrive))
        case .offline:
            status = Self.group(now: now, problem: .unreachable)
        case .folderGone:
            status = Self.group(now: now, problem: .unreachable, folder: .directory, account: nil)
        }
        let people = [
            ProfileView(id: "kim", name: "Kim", initials: "K", kids: false, active: true), // i18n-skip
            ProfileView(id: "sam", name: "Sam", initials: "S", kids: true, active: false), // i18n-skip
        ]
        let isProfile = [.profiles, .profileAdd, .profileEdit].contains(scene)
        stage(status: status, profiles: isProfile ? people : [])
        stagedRemoteKeys = [Self.tvKey]
        switch scene {
        case .road: sheet = .road
        case .picker: sheet = .picker
        case .pickerDenied:
            denied = .googleDrive
            sheet = .picker
        case .nextcloud: sheet = .nextcloud
        case .nextcloudWait:
            stage(signIn: SyncSignInRequest(service: .nextcloud, step: .browser(url: "https://cloud.example/login/flow")))
            sheet = .nextcloud
        case .nextcloudErr:
            stage(problem: .unreachable)
            sheet = .nextcloud
        case .webdav: sheet = .webDav
        case .webdavErr:
            stage(problem: .unreachable)
            sheet = .webDav
        case .key:
            stage(key: "K7QM4XR9TB2HWN6CP3ZD8FLA")
            sheet = .recoveryKey
        case .keyQr:
            stage(key: "K7QM4XR9TB2HWN6CP3ZD8FLA")
            sheet = .recoveryKey
        case .restore:
            sheet = .restore(.googleDrive)
        case .restoreWrong:
            stage(problem: .wrongRecoveryKey)
            sheet = .restore(.googleDrive)
        case .pairShow:
            stage(invite: SyncInviteView(code: "042917", port: 4510, until: .now.addingTimeInterval(108)))
            sheet = .pairShow
        case .pairMotion:
            stage(invite: SyncInviteView(code: "042917", port: 4510, until: .now.addingTimeInterval(108)))
            sheet = .pairShow
            Task { [weak self] in
                try? await Task.sleep(for: .seconds(4))
                self?.stage(pair: .paired("Kim’s iPhone")) // i18n-skip
            }
        case .firstMotion:
            stage(invite: SyncInviteView(code: "042917", port: 4510, until: .now.addingTimeInterval(108)))
            Task { [weak self] in
                try? await Task.sleep(for: .seconds(4))
                self?.stage(pair: .paired("Kim’s iPhone")) // i18n-skip
            }
        case .firstSetup:
            stage(invite: SyncInviteView(code: "042917", port: 4510, until: .now.addingTimeInterval(108)))
        case .firstPaired:
            stage(pair: .paired("Kim’s iPhone")) // i18n-skip
        case .firstPairedStore:
            stage(pair: .paired("Kim’s iPhone")) // i18n-skip
        case .pairWrong:
            stage(pair: .failed(.code), device: "Kim’s iPhone") // i18n-skip
            sheet = .pairEnter
        case .pairWorking:
            stage(pair: .working)
            sheet = .pairEnter
        case .pairEnter: sheet = .pairEnter
        case .pairErr:
            stage(pair: .failed(.code))
            sheet = .pairEnter
        case .pairNf:
            stage(pair: .failed(.notFound))
            sheet = .pairEnter
        case .paired:
            stage(pair: .paired("Living room TV")) // i18n-skip
            sheet = .pairEnter
        case .profileAdd: sheet = .profileAdd
        case .profileEdit: sheet = .profileEdit("sam")
        default: break
        }
        self.sheet = sheet
        return true
    }
}

/// What this device is called in the staged group.
enum DeviceName {
    static var this: String {
        #if os(macOS)
        "Kim’s Mac" // i18n-skip
        #else
        "Kim’s iPad" // i18n-skip
        #endif
    }
}
