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

/// What the cloud folder's sheet shows after a service is chosen, in place of the list: the wait
/// for its page, the sign-in finishing, the folder being made, what went wrong, or the new
/// recovery key.
public enum CloudStep: Equatable, Sendable {
    case waiting(SyncFolderKind)
    case connecting(SyncFolderKind)
    case settingUp(SyncFolderKind)
    /// `folder` when the sign-in had worked and the folder could not be made.
    case failed(SyncFolderKind, SyncProblem, folder: Bool)
    case key

    /// A step under way, which the engine moves on by itself.
    public var underWay: Bool {
        switch self {
        case .waiting, .connecting, .settingUp: true
        case .failed, .key: false
        }
    }

    /// Connecting and setting up show only once they have run 600 ms.
    var quiet: Bool {
        switch self {
        case .connecting, .settingUp: true
        default: false
        }
    }
}

/// The sheets sync opens. One at a time: a sheet over another is a step of the first.
public enum SyncSheet: Equatable, Identifiable, Sendable {
    /// Choose a cloud folder, then its sign-in and set-up in the same sheet (`cloudStep`).
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
    /// The device that turned a typed code down, when only one did: the error names it.
    public private(set) var pairFailedDevice: String?
    /// What the last Copy sign-ins brought.
    public private(set) var copied: SyncSignInsCopiedView?
    /// What the device just paired with has to bring: the medium it answered, its book servers
    /// and plugin stores, and the first-run pages it already answered.
    public private(set) var setup: JoinedSetup?
    /// The cloud service signed in to, until its folder is made: the row says it is setting up.
    public private(set) var settingUp: SyncFolderKind?
    /// Whether the folder is being set up long enough to say so (past 600 ms).
    public internal(set) var settingUpShown = false
    public var sheet: SyncSheet?
    /// What the cloud folder's sheet shows in place of its list, nil for the list. A step shows
    /// past its quiet time and stays at least 400 ms (`aim`).
    public private(set) var cloudStep: CloudStep?
    /// The picker was opened to move the group to another service: the one in use is dimmed, and
    /// a choice changes the folder instead of setting sync up.
    public private(set) var switching = false
    /// The Remove cloud folder confirmation is up.
    public var removingFolder = false
    /// How many times the person asked for the sign-in page again; the presenter opens it anew.
    public private(set) var pageOpens = 0
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
    /// When the pass shown as Syncing began, for the popover's elapsed time.
    public internal(set) var syncingSince: ContinuousClock.Instant?
    /// The other device a pass talks with: the first one that is not this device.
    public var syncingPartner: String? { status.devices.first { !$0.thisDevice }?.name }
    @ObservationIgnored public var send: (Command) -> Void = { _ in }
    /// The remote keys the Fixtures app counts as paired, in place of the sender's.
    @ObservationIgnored public var stagedRemoteKeys: Set<String>?
    /// Set by the Fixtures app: states stay as staged.
    @ObservationIgnored public var staged = false
    @ObservationIgnored private var settleTask: Task<Void, Never>?
    @ObservationIgnored private var syncingTask: Task<Void, Never>?
    /// The step the engine is at, which `cloudStep` shows once its time comes.
    private var cloudTarget: CloudStep?
    @ObservationIgnored private var cloudShownAt: ContinuousClock.Instant?
    @ObservationIgnored private var cloudTask: Task<Void, Never>?

    public init() {}

    // MARK: Reading

    public var enabled: Bool { status.enabled }
    /// Sync as the pages show it: on with another device or a folder. On, alone and without a
    /// folder (a code shown and never entered) reads as off.
    public var inUse: Bool { status.enabled && (status.folder != nil || !others.isEmpty) }
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
        if !inUse { return .off }
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

    /// Add a cloud folder: the picker, setting sync up.
    public func addCloudFolder() {
        switching = false
        resetCloudStep()
        sheet = .picker
    }

    /// Use another service: the picker, moving the group's folder.
    public func useAnotherService() {
        switching = true
        resetCloudStep()
        sheet = .picker
    }

    /// A service from the picker: the three cloud drives go straight into their sign-in, and the
    /// sheet waits for its page at once.
    public func choose(_ kind: SyncFolderKind, deviceName: String) {
        problem = nil
        lastService = kind
        switch kind {
        case .googleDrive, .dropbox, .oneDrive:
            aim(.waiting(kind), atOnce: true)
            begin(SyncFolderSpec(kind: kind), deviceName: deviceName)
        case .nextcloud: sheet = .nextcloud
        case .webDav: sheet = .webDav
        case .directory: break
        }
    }

    public func startNextcloud(address: String, deviceName: String) {
        problem = nil
        lastService = .nextcloud
        begin(SyncFolderSpec(kind: .nextcloud, address: address), deviceName: deviceName)
    }

    public func startWebDav(address: String, user: String, password: String, deviceName: String) {
        problem = nil
        lastService = .webDav
        begin(SyncFolderSpec(kind: .webDav, address: address, user: user, password: password), deviceName: deviceName)
    }

    public func startDirectory(_ path: String, deviceName: String) {
        problem = nil
        lastService = .directory
        aim(.settingUp(.directory), atOnce: true)
        begin(SyncFolderSpec(kind: .directory, address: path), deviceName: deviceName)
    }

    /// Set sync up with `folder`, or move the group to it under Use another service.
    private func begin(_ folder: SyncFolderSpec, deviceName: String) {
        if switching {
            changeFolder(folder)
        } else {
            send(.startSync(deviceName: deviceName, folder: folder))
        }
    }

    /// The sign-in page came back with its answer, which the app hands the engine now.
    public func signInReturned(_ service: SyncFolderKind) {
        if cloudTarget == .waiting(service) { aim(.connecting(service)) }
    }

    /// The sign-in page was closed by the person: the sign-in ends quietly, and a sheet that
    /// waited for it goes back to its list.
    public func signInClosed() {
        cancelSignIn()
        if case .waiting = cloudTarget { aim(nil) }
    }

    /// The sign-in page failed other than by being closed: the service refused.
    public func signInRefused(_ service: SyncFolderKind) {
        let waiting = cloudTarget?.underWay == true
        cancelSignIn()
        if waiting, sheet == .picker {
            aim(.failed(service, .signInDenied(service: service), folder: false))
        } else {
            problem = .signInDenied(service: service)
        }
    }

    /// The step aimed at is past the page's answer, so Cancel can't stop it: dimmed at once, while
    /// the step shown may still be the wait.
    public var cloudCancelDimmed: Bool {
        switch cloudTarget {
        case .connecting?, .settingUp?: true
        default: false
        }
    }

    /// Open the page again: the presenter closes the one it has and opens the same page.
    public func openPageAgain() { pageOpens += 1 }

    /// Cancel in the cloud folder's sheet: back to the list from the wait, closed from an error.
    public func cancelCloudStep() {
        switch cloudTarget {
        case .waiting:
            cancelSignIn()
            aim(nil)
        case .connecting, .settingUp:
            break
        case .failed, .key, nil:
            sheet = nil
            resetCloudStep()
        }
    }

    /// Try again after an error: the same choice again. A folder of this device has its own
    /// picker, so that one goes back to the list.
    public func tryAgain(deviceName: String) {
        guard case .failed(let kind, _, _) = cloudTarget else { return }
        if [.googleDrive, .dropbox, .oneDrive].contains(kind) {
            choose(kind, deviceName: deviceName)
        } else {
            aim(nil)
        }
    }

    /// The cloud folder's sheet went: a sign-in still waiting for its page goes with it.
    public func pickerClosed() {
        if case .waiting = cloudTarget { cancelSignIn() }
        if sheet != .picker {
            resetCloudStep()
            switching = false
        }
    }

    /// Remove cloud folder, after its confirmation: the engine signs out of the service too.
    public func removeFolder() {
        removingFolder = false
        signedOutByHand = false
        problem = nil
        changeFolder(nil)
    }

    /// Whether the sheet may not be swiped away: the folder is being made, or the key shows.
    public var holdsSheet: Bool {
        switch sheet {
        case .recoveryKey: return true
        case .picker:
            switch cloudStep {
            case .connecting, .settingUp, .key: return true
            default: return false
            }
        default: return false
        }
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
    /// A TV paired as a remote already trusts this device: swap histories with it, no new code.
    public func alsoSync(with device: String, address: String) {
        send(.syncWithDevice(device: device, address: address))
    }

    public func dismissNote(_ id: Int64) { send(.dismissSyncNote(id: id)) }

    /// The conflicts the rules decided against someone's action, newest first, kept 7 days.
    public var notes: [SyncNoteView] { status.notes.sorted { $0.at > $1.at } }

    /// A note's undo: the episode is marked unwatched here, and the note goes.
    public func undo(_ note: SyncNoteView) {
        send(.setWatched(id: note.anilistId, episodes: [note.episode], watched: false))
        dismissNote(note.id)
    }
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

    /// A scanned code, which carries the address of the device showing it.
    public func pairWith(address: String, code: String, deviceName: String) {
        pair = .working
        pairFailedDevice = nil
        send(.pairJoin(deviceName: deviceName, address: address, code: code))
    }

    /// A typed code: the engine finds the device showing it on the network.
    public func pairWithCode(_ code: String, deviceName: String) {
        pair = .working
        pairFailedDevice = nil
        problem = nil
        send(.syncGroup(.pairWithCode(deviceName: deviceName, code: code)))
    }

    public func resetPair() {
        pair = .idle
        setup = nil
        pairFailedDevice = nil
    }

    /// The Add a device sheet closed: the code it showed stops being offered.
    public func pairSheetClosed() {
        send(.pairStop)
        invite = nil
    }

    /// Rename this device, in the group when sync is on. Kept for the next start of Play on TV too.
    public func rename(_ name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        deviceName = trimmed
        Self.keepName(trimmed)
        send(.syncGroup(.renameDevice(name: trimmed)))
    }

    /// The Sync switch of another device: off holds the group back from it.
    public func setSync(_ device: SyncDeviceView, on: Bool) {
        send(.syncGroup(.setDeviceSync(device: device.name, on: on)))
    }

    /// The Remote switch of this device and `device`, as a pair.
    public func setRemote(_ device: SyncDeviceView, on: Bool) {
        send(.syncGroup(.setDeviceRemote(device: device.name, on: on)))
    }

    /// The name this device keeps for Play on TV, from its last rename or the group's.
    public static let nameKey = "syncDeviceName"

    static func keepName(_ name: String) {
        UserDefaults.standard.set(name, forKey: nameKey)
    }

    /// The remote keys (hex fingerprints) of the devices whose Remote switch is off.
    public var remoteOff: Set<String> {
        Set(status.devices.filter(\.remoteOff).compactMap(\.remoteKey))
    }

    /// Ask the other device for its sign-ins when the next session between them opens.
    public func copySignIns(from device: String?, _ copy: [SyncCopy], address: String? = nil) {
        send(.syncGroup(.copySignIns(device: device, copy: copy, address: address)))
    }

    /// Join plugin stores through a paired device: it makes this device its own invite, so this
    /// device gets a token of its own. With no `device`, asks the next device to pair for its
    /// invites, inside the pairing session. An empty `stores` means every store `device` is in.
    /// Ask the paired device for its setup: stores, book servers, sign-ins and the medium. Without
    /// a device, the next one to pair.
    public func askSetup(from device: String?, address: String? = nil) {
        send(.syncGroup(.askSetupFrom(device: device, address: address)))
    }

    /// Add the book servers picked from a paired device; none picked adds every one held.
    public func addBookServers(from device: String, _ ids: [String]) {
        send(.syncGroup(.addBookServersFrom(device: device, servers: ids)))
    }

    public func joinStores(from device: String?, _ stores: [String] = [], address: String? = nil) {
        send(.syncGroup(.joinStoresFrom(device: device, stores: stores, address: address)))
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
            // The picker's sheet turns into the key; any other sheet gives way to it.
            if sheet == .picker, cloudTarget != nil {
                aim(.key)
            } else {
                sheet = .recoveryKey
            }
        case .syncSignIn(let service, let step):
            lastService = service
            if let step {
                signIn = SyncSignInRequest(service: service, step: step)
                if case .finishing = step { beginSettingUp(service) }
                if sheet == .picker, cloudTarget?.underWay == true {
                    switch step {
                    case .connecting: aim(.connecting(service))
                    case .finishing: aim(.settingUp(service))
                    default: if case .waiting = cloudTarget {} else { aim(.waiting(service)) }
                    }
                }
            } else {
                signIn = nil
                if sheet == .picker {
                    // A sign-in that ended while its sheet waited on it ended well: the sheet
                    // closes and the row says the rest. A key or an error keeps the sheet.
                    if cloudTarget?.underWay == true, problem == nil {
                        sheet = nil
                        resetCloudStep()
                    }
                } else if problem == nil, sheet == .nextcloud || sheet == .webDav {
                    sheet = nil
                }
            }
        case .syncPairFailed(let why, let device):
            pair = .failed(why == .notFound ? .notFound : .code)
            pairFailedDevice = device
        case .syncProblem(let found):
            problem = found
            if case .groupExists = found {
                resetCloudStep()
                sheet = .restore(lastService ?? .googleDrive)
            } else if sheet == .picker, let target = cloudTarget, target.underWay {
                let folder: Bool = { if case .settingUp = target { return true } else { return false } }()
                aim(.failed(lastService ?? .googleDrive, found, folder: folder))
            }
            settingUp = nil
            settingUpShown = false
            signIn = nil
            if case .failed = pair {} else if pair == .working { pair = .failed(.code) }
        case .syncSignInsCopied(let device, let trackers, let clouds, let lacking):
            copied = SyncSignInsCopiedView(device: device, trackers: trackers, clouds: clouds, lacking: lacking)
        case .profiles(let view):
            profiles = view.profiles
        case .setupFrom(let joined):
            setup = joined
        default:
            break
        }
    }

    private func statusChanged(_ new: SyncStatus) {
        let before = status
        status = new
        loaded = true
        if let name = new.deviceName {
            if deviceName.isEmpty { deviceName = name }
            if new.enabled { Self.keepName(name) }
        }
        // A folder of this device set up with no new key: the picker's work is done.
        if sheet == .picker, case .settingUp(.directory) = cloudTarget, new.folder?.kind == .directory,
           before.folder?.kind != .directory || before.folder?.label != new.folder?.label {
            sheet = nil
            resetCloudStep()
        }
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
                self?.syncingSince = .now
            }
        } else if !new.syncing {
            syncingTask?.cancel()
            syncingShown = false
            syncingSince = nil
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
        resetCloudStep()
    }

    // MARK: The cloud folder's sheet

    /// Move the sheet to `step`: at once when nothing shows, else once the step shown has had
    /// its 400 ms; connecting and setting up wait 600 ms first, so a quick step never shows.
    /// The list (nil), and a step the person's own tap began, come at once.
    private func aim(_ step: CloudStep?, atOnce: Bool = false) {
        cloudTarget = step
        cloudTask?.cancel()
        let now = ContinuousClock.now
        guard let step, !staged, !atOnce else { return show(step) }
        var due = step.quiet ? now + .milliseconds(600) : now
        if cloudStep != nil, let at = cloudShownAt { due = max(due, at + .milliseconds(400)) }
        guard due > now else { return show(step) }
        cloudTask = Task { [weak self] in
            try? await Task.sleep(until: due, clock: .continuous)
            guard !Task.isCancelled, let self, self.cloudTarget == step else { return }
            self.show(step)
        }
    }

    private func show(_ step: CloudStep?) {
        cloudStep = step
        cloudShownAt = step == nil ? nil : .now
    }

    private func resetCloudStep() {
        cloudTask?.cancel()
        cloudTarget = nil
        show(nil)
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
    public func stage(pair new: PairStage, device: String? = nil) {
        pair = new
        pairFailedDevice = device
    }
    public func stage(settingUp service: SyncFolderKind?) {
        settingUp = service
        settingUpShown = service != nil
    }
    public func stage(copied new: SyncSignInsCopiedView?) { copied = new }
    public func stage(setup new: JoinedSetup?) { setup = new }
    public func stage(cloudStep step: CloudStep?, switching: Bool = false) {
        self.switching = switching
        cloudTarget = step
        show(step)
    }
}

/// Sign-ins that came over the link.
public struct SyncSignInsCopiedView: Equatable, Sendable {
    public var device: String
    public var trackers: [String]
    public var clouds: [SyncFolderKind]
    public var lacking: [SyncFolderKind]
}
