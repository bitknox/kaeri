// Sync between this person's own devices, and the profiles on this one: what the engine last said
// (`SyncStatus`, `ProfilesView`), the cloud sign-in under way, the recovery key to show once, and
// the sheets that ask for each step. The engine holds every rule; this keeps its answers on the
// main actor for the screens and turns a tap into one of its commands.

import KaeriKit
import KaeriStrings
import Foundation
import Observation

/// A cloud sign-in under way: the service and the step the engine said last.
public struct SyncSignInRequest: Equatable, Sendable {
    public var service: SyncFolderKind
    public var step: SyncSignInStep
}

/// The sheets sync opens. One at a time: a sheet over another is a step of the first.
public enum SyncSheet: Equatable, Identifiable, Sendable {
    /// With a device nearby, or through a cloud folder.
    case road
    /// Choose a cloud folder.
    case picker
    /// A Nextcloud server's address, then its login page.
    case nextcloud
    /// A WebDAV server's address and login.
    case webDav
    /// The recovery key, shown when the first cloud folder is set.
    case recoveryKey
    /// A folder that holds history asks for its recovery key.
    case restore(SyncFolderKind)
    /// Add a device: enter the code the other one shows.
    case pairEnter
    /// Add a device: show a code for the other one to enter.
    case pairShow
    /// Add a profile.
    case profileAdd
    /// A profile's page.
    case profileEdit(String)

    public var id: String {
        switch self {
        case .road: "road"
        case .picker: "picker"
        case .nextcloud: "nextcloud"
        case .webDav: "webdav"
        case .recoveryKey: "key"
        case .restore: "restore"
        case .pairEnter: "pair-enter"
        case .pairShow: "pair-show"
        case .profileAdd: "profile-add"
        case .profileEdit(let id): "profile-\(id)"
        }
    }
}

/// How an attempt to pair went wrong.
public enum PairProblem: Equatable, Sendable {
    /// The code did not match.
    case code
    /// Nothing on the network answered.
    case notFound
    /// The code ran out.
    case expired
}

/// Where adding a device stands.
public enum PairStage: Equatable, Sendable {
    case idle
    /// A try is out.
    case working
    /// Paired: the device's name. The sheet's content crossfades to this.
    case paired(String)
    case failed(PairProblem)
}

/// A pairing code this device shows, and the address and port a QR code for it carries.
public struct SyncInviteView: Equatable, Sendable {
    public var code: String
    public var port: UInt16
    /// When it stops working.
    public var until: Date
}

@Observable @MainActor
public final class SyncModel {
    public private(set) var status = SyncStatus(enabled: false, syncing: false)
    /// Whether the engine has said how sync stands.
    public private(set) var loaded = false
    public private(set) var profiles: [ProfileView] = []
    public private(set) var signIn: SyncSignInRequest?
    /// What went wrong with the last set-up, join or sign-in, in the engine's words.
    public private(set) var problem: SyncProblem?
    /// The recovery key the engine said, shown once and then forgotten.
    public private(set) var recoveryKey: String?
    public private(set) var invite: SyncInviteView?
    public private(set) var pair: PairStage = .idle
    /// What the last Copy sign-ins brought.
    public private(set) var copied: SyncSignInsCopiedView?
    /// The cloud service signed in to, until its folder is made: the row says it is setting up.
    public private(set) var settingUp: SyncFolderKind?
    /// Whether the folder is being set up long enough to say so (past 600 ms).
    public internal(set) var settingUpShown = false
    public var sheet: SyncSheet?
    /// A service whose sign-in page was closed or refused, for the line under the picker.
    public var denied: SyncFolderKind?
    /// What the recovery key sheet was opened for: set up, or seen again from Devices.
    public private(set) var keyCameFromDevices = false
    /// A device's name that was typed for this one, until the engine says it back.
    public var deviceName = ""
    /// Which service the last sign-in step belongs to, for the error under the picker.
    public private(set) var lastService: SyncFolderKind?
    /// The person pressed Sign out, so a sign-in that is needed is theirs and no error.
    public private(set) var signedOutByHand = false
    /// A short quiet while a pass runs, so the toolbar capsule says Syncing only past 600 ms.
    public internal(set) var syncingShown = false
    @ObservationIgnored public var send: (Command) -> Void = { _ in }
    /// Set by the Fixtures app: states stay as staged.
    @ObservationIgnored public var staged = false
    @ObservationIgnored private var settleTask: Task<Void, Never>?
    @ObservationIgnored private var syncingTask: Task<Void, Never>?

    public init() {}

    // MARK: Reading

    public var enabled: Bool { status.enabled }
    public var hasProfiles: Bool { profiles.count >= 2 }
    public var activeProfile: ProfileView? { profiles.first(where: \.active) }

    /// The cloud services this build can sign in to, in alphabetical order, with the ones it
    /// cannot hidden (a service without a client id, or without a way on this device).
    public var offered: [SyncCloudView] {
        status.clouds.filter { view in
            if case .unavailable(let reason) = view.way { return reason == .needsPhone }
            return true
        }
        .sorted { Self.name($0.service) < Self.name($1.service) }
    }

    /// Whether any cloud service can be signed in to here.
    public var offersCloud: Bool { !offered.isEmpty }

    public var devices: [SyncDeviceView] { status.devices }
    public var others: [SyncDeviceView] { status.devices.filter { !$0.thisDevice } }

    /// What the Settings row's value says: Sync off, Synced 2 min ago, or Couldn't sync.
    public enum RowState: Equatable { case off, synced(Date?), error }

    public var rowState: RowState {
        if !status.enabled { return .off }
        if status.problem != nil || status.error != nil { return .error }
        return .synced(status.lastSynced.map { Date(timeIntervalSince1970: TimeInterval($0)) })
    }

    /// The service name a person reads.
    public static func name(_ kind: SyncFolderKind) -> String {
        switch kind {
        case .directory: L.cloudFolder(kind: "Mac")
        case .webDav: L.cloudWebdav
        case .nextcloud: "Nextcloud" // i18n-skip
        case .googleDrive: "Google Drive" // i18n-skip
        case .dropbox: "Dropbox" // i18n-skip
        case .oneDrive: "OneDrive" // i18n-skip
        }
    }

    // MARK: Commands

    public func load() {
        send(.loadSync)
        send(.loadProfiles)
    }

    public func turnOnNearby(deviceName: String) {
        send(.pairInvite(deviceName: deviceName))
    }

    /// A service from the picker: the three cloud drives go straight into their sign-in.
    public func choose(_ kind: SyncFolderKind, deviceName: String) {
        problem = nil
        denied = nil
        lastService = kind
        switch kind {
        case .googleDrive, .dropbox, .oneDrive:
            send(.startSync(deviceName: deviceName, folder: SyncFolderSpec(kind: kind)))
        case .nextcloud: sheet = .nextcloud
        case .webDav: sheet = .webDav
        case .directory: break
        }
    }

    public func startNextcloud(address: String, deviceName: String) {
        problem = nil
        lastService = .nextcloud
        send(.startSync(deviceName: deviceName, folder: SyncFolderSpec(kind: .nextcloud, address: address)))
    }

    public func startWebDav(address: String, user: String, password: String, deviceName: String) {
        problem = nil
        lastService = .webDav
        send(.startSync(deviceName: deviceName, folder: SyncFolderSpec(
            kind: .webDav, address: address, user: user, password: password)))
    }

    public func startDirectory(_ path: String, deviceName: String) {
        problem = nil
        lastService = .directory
        send(.startSync(deviceName: deviceName, folder: SyncFolderSpec(kind: .directory, address: path)))
    }

    /// A folder that already holds a group, joined with its recovery key.
    public func join(_ kind: SyncFolderKind, key: String, deviceName: String) {
        problem = nil
        send(.joinSync(deviceName: deviceName, folder: SyncFolderSpec(kind: kind), recoveryKey: key))
    }

    public func cancelSignIn() {
        send(.cancelSyncSignIn)
        signIn = nil
        settingUp = nil
    }

    public func signInAgain(_ kind: SyncFolderKind) {
        problem = nil
        signedOutByHand = false
        send(.syncSignIn(service: kind))
    }

    public func signOut(_ kind: SyncFolderKind) {
        signedOutByHand = true
        send(.syncSignOut(service: kind))
    }
    public func syncNow() { send(.syncFolderNow) }
    public func turnOff() { send(.leaveSync) }
    public func remove(_ device: String) { send(.removeSyncDevice(device: device)) }
    public func dismissNote(_ id: Int64) { send(.dismissSyncNote(id: id)) }
    public func showRecoveryKey() {
        keyCameFromDevices = true
        send(.showRecoveryKey)
    }
    public func newRecoveryKey() {
        keyCameFromDevices = true
        send(.newRecoveryKey)
    }
    public func invite(deviceName: String) { send(.pairInvite(deviceName: deviceName)) }
    public func stopInvite() {
        send(.pairStop)
        invite = nil
    }

    public func pairWith(address: String, code: String, deviceName: String) {
        pair = .working
        send(.pairJoin(deviceName: deviceName, address: address, code: code))
    }

    public func resetPair() { pair = .idle }

    /// Ask the other device for its sign-ins when the next session between them opens.
    public func copySignIns(from device: String?, _ copy: [SyncCopy], address: String? = nil) {
        send(.syncGroup(.copySignIns(device: device, copy: copy, address: address)))
    }

    public func changeFolder(_ spec: SyncFolderSpec?) {
        send(.syncGroup(.changeSyncFolder(folder: spec)))
    }

    public func addProfile(name: String, kids: Bool, firstName: String?) {
        send(.addProfile(name: name, kids: kids, firstName: firstName))
    }
    public func editProfile(_ id: String, name: String, kids: Bool) { send(.editProfile(id: id, name: name, kids: kids)) }
    public func switchProfile(_ id: String) { send(.switchProfile(id: id)) }
    public func deleteProfile(_ id: String) { send(.deleteProfile(id: id)) }

    // MARK: Events

    public func apply(_ event: Event) {
        switch event {
        case .syncStatus(let new):
            if !staged { statusChanged(new) }
        case .syncInvite(let code, let port, let seconds):
            invite = SyncInviteView(code: code, port: port, until: .now.addingTimeInterval(TimeInterval(seconds)))
        case .syncRecoveryKey(let key):
            recoveryKey = key
            settingUp = nil
            settingUpShown = false
            settleTask?.cancel()
            signIn = nil
            sheet = .recoveryKey
        case .syncSignIn(let service, let step):
            lastService = service
            if let step {
                signIn = SyncSignInRequest(service: service, step: step)
                if case .finishing = step { beginSettingUp(service) }
            } else {
                signIn = nil
                // The picker and its sign-in close together; the row says it is setting up.
                if problem == nil, sheet == .picker || sheet == .nextcloud || sheet == .webDav {
                    sheet = nil
                }
            }
        case .syncProblem(let found):
            problem = found
            if case .groupExists = found { sheet = .restore(lastService ?? .googleDrive) }
            settingUp = nil
            settingUpShown = false
            signIn = nil
            if case .failed = pair {} else if pair == .working { pair = .failed(.code) }
        case .syncSignInsCopied(let device, let trackers, let clouds, let lacking):
            copied = SyncSignInsCopiedView(device: device, trackers: trackers, clouds: clouds, lacking: lacking)
        case .profiles(let view):
            profiles = view.profiles
        default:
            break
        }
    }

    private func statusChanged(_ new: SyncStatus) {
        let before = status
        status = new
        loaded = true
        if let name = new.deviceName, deviceName.isEmpty { deviceName = name }
        if settingUp != nil, new.folder != nil, !new.syncing {
            settingUp = nil
            settingUpShown = false
            settleTask?.cancel()
        }
        // A pairing finished when the group has a device it had not.
        if pair == .working, new.devices.count > before.devices.count || (!before.enabled && new.enabled) {
            let other = new.devices.filter { !$0.thisDevice }.first { device in !before.devices.contains { $0.id == device.id } } ?? new.devices.first { !$0.thisDevice }
            pair = .paired(other?.name ?? "")
        }
        // The pass shows as Syncing in the toolbar only once it has run past 600 ms.
        if new.syncing, !before.syncing {
            syncingTask?.cancel()
            syncingTask = Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(600))
                guard !Task.isCancelled else { return }
                self?.syncingShown = true
            }
        } else if !new.syncing {
            syncingTask?.cancel()
            syncingShown = false
        }
        if new.problem == nil { problem = problem.flatMap { Self.stays($0) ? $0 : nil } }
    }

    /// A problem the status clears by itself when it says none, or one only a new try clears.
    private static func stays(_ problem: SyncProblem) -> Bool {
        switch problem {
        case .wrongRecoveryKey, .groupExists, .noGroup: true
        default: false
        }
    }

    private func beginSettingUp(_ service: SyncFolderKind) {
        settingUp = service
        settleTask?.cancel()
        settleTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(600))
            guard !Task.isCancelled else { return }
            self?.settingUpShown = true
        }
    }

    /// The recovery key sheet's Done: the key is forgotten.
    public func keySaved() {
        recoveryKey = nil
        sheet = nil
        keyCameFromDevices = false
    }

    /// The code the other device showed ran out.
    public func inviteEnded() { invite = nil }

    // MARK: Staging

    /// Put a state on screen without the engine, for the Fixtures app's shots.
    public func stage(status new: SyncStatus, profiles list: [ProfileView]? = nil) {
        staged = true
        status = new
        loaded = true
        if let list { profiles = list }
    }

    public func stage(problem new: SyncProblem?, byHand: Bool = false) {
        problem = new
        signedOutByHand = byHand
    }
    public func stage(signIn new: SyncSignInRequest?) { signIn = new }
    public func stage(key: String?) { recoveryKey = key }
    public func stage(invite new: SyncInviteView?) { invite = new }
    public func stage(pair new: PairStage) { pair = new }
    public func stage(settingUp service: SyncFolderKind?) {
        settingUp = service
        settingUpShown = service != nil
    }
    public func stage(copied new: SyncSignInsCopiedView?) { copied = new }
}

/// Sign-ins that came over the link.
public struct SyncSignInsCopiedView: Equatable, Sendable {
    public var device: String
    public var trackers: [String]
    public var clouds: [SyncFolderKind]
    public var lacking: [SyncFolderKind]
}
