// What the Devices page and the profiles ask of the engine, and the cloud sign-in's browser half.

package app.kaeri.store

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.SyncCommand
import app.kaeri.engine.model.SyncCopy
import app.kaeri.engine.model.SyncFolderKind
import app.kaeri.engine.model.SyncFolderSpec
import app.kaeri.engine.model.SyncSignInStep

/** The services the cloud sign-in's redirects name: `kaeri://auth/dropbox`, `kaeri://auth/onedrive` and Google's reversed client id. */
internal val SYNC_SERVICES = mapOf(
    "google" to SyncFolderKind.GOOGLE_DRIVE,
    "dropbox" to SyncFolderKind.DROPBOX,
    "onedrive" to SyncFolderKind.ONE_DRIVE,
)

/** The id a redirect names [service] by. */
internal fun syncServiceId(service: SyncFolderKind): String? = SYNC_SERVICES.entries.firstOrNull { it.value == service }?.key

/** A cloud sign-in moved on: the sheet step opens the browser, any other is the wait the page shows. */
internal fun AppStore.syncSignIn(service: SyncFolderKind, step: SyncSignInStep?) {
    sync.signIn(service, step)
    val id = syncServiceId(service)
    when {
        step is SyncSignInStep.Sheet && id != null -> accounts.authorize(SignInRequest(id, step.url, step.callbackScheme))
        // Over, or past the browser: the tab and its request are done with.
        id != null -> accounts.signIn.value?.takeIf { it.tracker == id }?.let(accounts::signInEnded)
    }
}

/** Ask how sync and the profiles stand. */
fun AppStore.loadSync() {
    send(Command.LoadSync)
    send(Command.LoadProfiles)
}

/** Set sync up in [folder], naming this device [deviceName]: a cloud drive signs in first. */
fun AppStore.startSync(deviceName: String, folder: SyncFolderSpec) {
    sync.problemSeen()
    sync.begin(folder)
    send(Command.StartSync(deviceName, folder))
}

/**
 * Begin a cloud folder from the picker: a device with a folder in use moves to the new one and
 * keeps the old until it is ready, one without sets sync up in it.
 */
fun AppStore.beginSyncFolder(deviceName: String, folder: SyncFolderSpec) {
    if (sync.status.value?.folder != null) changeSyncFolder(folder) else startSync(deviceName, folder)
}

/** Join the group [folder] holds, with its recovery key. */
fun AppStore.joinSync(deviceName: String, folder: SyncFolderSpec, recoveryKey: String) {
    sync.problemSeen()
    sync.begin(folder)
    send(Command.JoinSync(deviceName, folder, recoveryKey))
}

/** Sign in to [service] again, as Sign in again asks. */
fun AppStore.syncSignInAgain(service: SyncFolderKind) {
    sync.problemSeen()
    send(Command.SyncSignIn(service))
}

/** Sign out of [service]: at once, with no question. */
fun AppStore.syncSignOut(service: SyncFolderKind) {
    send(Command.SyncSignOut(service))
}

/** Stop the cloud sign-in under way, at any step. Quiet. */
fun AppStore.cancelSyncSignIn() {
    accounts.signIn.value?.takeIf { it.tracker in SYNC_SERVICES }?.let(accounts::signInEnded)
    send(Command.CancelSyncSignIn)
    sync.cancelled()
}

/** The sign-in's page again, as Open the page again asks: the tab the person closed, or lost behind another app. */
fun AppStore.openSyncPageAgain() {
    accounts.signIn.value?.takeIf { it.tracker in SYNC_SERVICES }?.let { accounts.reopen(it) }
}

/** Whether [request] is a cloud folder's sign-in rather than a tracker's. */
fun AppStore.isSyncSignIn(request: SignInRequest): Boolean = request.tracker in SYNC_SERVICES

fun AppStore.syncFolderNow() {
    send(Command.SyncFolderNow)
}

/** Turn sync off on this device: it keeps its data and stops sharing. */
fun AppStore.leaveSync() {
    send(Command.LeaveSync)
}

fun AppStore.removeSyncDevice(device: String) {
    send(Command.RemoveSyncDevice(device))
}

/** Ask for the recovery key again, after the device's own lock. */
fun AppStore.showRecoveryKey() {
    send(Command.ShowRecoveryKey)
}

fun AppStore.newRecoveryKey() {
    send(Command.NewRecoveryKey)
}

/** Show a pairing code and wait for a device to enter it. */
fun AppStore.pairInvite(deviceName: String) {
    send(Command.PairInvite(deviceName))
}

fun AppStore.pairStop() {
    sync.inviteEnded()
    send(Command.PairStop)
}

/** Pair with the device showing [code] at [address]. */
fun AppStore.pairJoin(deviceName: String, address: String, code: String) {
    send(Command.PairJoin(deviceName, address, code))
}

/** Ask the next device this one pairs with, or [device], for its sign-ins. An empty [copy] withdraws. */
fun AppStore.copySignIns(device: String?, copy: List<SyncCopy>, address: String? = null) {
    send(Command.SyncGroup(SyncCommand.CopySignIns(device, copy, address)))
}

fun AppStore.changeSyncFolder(folder: SyncFolderSpec?) {
    if (folder != null) {
        sync.problemSeen()
        sync.begin(folder)
    }
    send(Command.SyncGroup(SyncCommand.ChangeSyncFolder(folder)))
}

fun AppStore.dismissSyncNote(id: Long) {
    send(Command.DismissSyncNote(id))
}

fun AppStore.addProfile(name: String, kids: Boolean, firstName: String? = null) {
    send(Command.AddProfile(name, kids, firstName))
}

fun AppStore.editProfile(id: String, name: String, kids: Boolean) {
    send(Command.EditProfile(id, name, kids))
}

fun AppStore.switchProfile(id: String) {
    send(Command.SwitchProfile(id))
}

fun AppStore.deleteProfile(id: String) {
    send(Command.DeleteProfile(id))
}

/** Ask for every sign-in the next paired device holds: the trackers and each cloud drive. First launch's default. */
fun AppStore.copyAllSignIns() {
    val clouds = listOf(SyncFolderKind.GOOGLE_DRIVE, SyncFolderKind.DROPBOX, SyncFolderKind.ONE_DRIVE).map { SyncCopy.Cloud(it) }
    copySignIns(null, listOf(SyncCopy.Trackers) + clouds)
}

/** The success page's Copy sign-ins switch left off: nothing is copied from [device], whatever first launch asked. */
fun AppStore.copyNoSignIns(device: String) {
    copySignIns(device, emptyList())
}

/**
 * Join plugin stores through the paired device [device], which makes this device an invite of its own to each.
 * [stores] are addresses from the status offers; empty means every store it holds that this device doesn't.
 * With a null [device] the ask goes to the next device this one pairs with, inside the pairing: first launch's default.
 */
fun AppStore.joinStoresFrom(device: String?, stores: List<String> = emptyList(), address: String? = null) {
    send(Command.SyncGroup(SyncCommand.JoinStoresFrom(device, stores, address)))
}

/** Pair with whichever device on this network shows [code]. */
fun AppStore.pairWithCode(deviceName: String, code: String) {
    sync.problemSeen()
    sync.pairingFrom()
    send(Command.SyncGroup(SyncCommand.PairWithCode(deviceName, code)))
}

/** Rename this device, in the group when sync is on. */
fun AppStore.renameDevice(name: String) {
    sync.rename(name)
    send(Command.SyncGroup(SyncCommand.RenameDevice(name.trim())))
}

/** The Sync switch of [device]: off holds the group back from it. */
fun AppStore.setDeviceSync(device: String, on: Boolean) {
    send(Command.SyncGroup(SyncCommand.SetDeviceSync(device, on)))
}

/** The Remote switch of this device and [device], as a pair. */
fun AppStore.setDeviceRemote(device: String, on: Boolean) {
    send(Command.SyncGroup(SyncCommand.SetDeviceRemote(device, on)))
}

/** Hand the engine the platform's DNS-SD for typed-code pairing, once it runs. */
fun AppStore.setSyncNetwork(network: app.kaeri.engine.ffi.FfiSyncNetwork) {
    sync.network = network
    engine?.setSyncNetwork(network)
}

/** Ask [device], or with null the next device this one pairs with, for its setup: stores, book servers, the medium and sign-ins. */
fun AppStore.askSetupFrom(device: String?, address: String? = null) {
    send(Command.SyncGroup(SyncCommand.AskSetupFrom(device, address)))
}

/** Add the book servers [device] handed over, by offer id. Never empty: the engine reads empty as every one. */
fun AppStore.addBookServersFrom(device: String, servers: List<String>) {
    if (servers.isNotEmpty()) send(Command.SyncGroup(SyncCommand.AddBookServersFrom(device, servers)))
}

/** First run's join page shows a code: sign-ins, stores and the setup are asked of whichever device pairs. */
fun AppStore.joinShowCode(deviceName: String) {
    sync.join.begin()
    sync.first(true)
    copyAllSignIns()
    askSetupFrom(null)
    pairInvite(deviceName)
}

/** Back from the join page's code: the code stops and nothing is held. */
fun AppStore.joinStop() {
    pairStop()
    sync.first(false)
    sync.copiedSeen()
    sync.join.reset()
}

/**
 * The join page's Continue: what the switches left on comes over from the paired device. Sign-ins
 * switched off are withdrawn, the servers on are added, and the stores on are joined; returns the
 * stores asked for now, by name, whose answers the page waits on.
 */
fun AppStore.joinContinue(signIns: Boolean, stores: List<app.kaeri.engine.model.SyncStoreOffer>, servers: List<String>): Set<String> {
    val state = sync.join.state.value
    val setup = state.setup
    val first = state.presses == 0
    val asking = sync.join.proceed(stores.map { it.name })
    if (setup == null) {
        if (first && !signIns) sync.status.value?.pairedDevice(state.deviceName.orEmpty())?.let { copyNoSignIns(it.id) }
        return asking
    }
    if (first) {
        if (!signIns) copyNoSignIns(setup.device)
        addBookServersFrom(setup.device, servers)
    }
    val addresses = stores.filter { it.name in asking }.map { it.address }
    if (addresses.isNotEmpty()) joinStoresFrom(setup.device, addresses)
    return asking
}
