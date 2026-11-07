// Sync between devices and the profiles beside it, as the engine says them: the group's status,
// the cloud sign-in under way, the recovery key to show, the pairing code, and the profiles.

package app.kaeri.store

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import app.kaeri.engine.model.CloudUnavailable
import app.kaeri.engine.model.ProfilesView
import app.kaeri.engine.model.SyncCloudView
import app.kaeri.engine.model.SyncCloudWay
import app.kaeri.engine.model.SyncFolderKind
import app.kaeri.engine.model.SyncProblem
import app.kaeri.engine.model.SyncSignInStep
import app.kaeri.engine.model.SyncStatus

/** A pairing code showing on this device: [code] is good until [until], on the clock's milliseconds. */
data class PairInvite(val code: String, val port: Int, val until: Long)

/** The cloud sign-in under way: which service, and where it stands. */
data class CloudSignIn(val service: SyncFolderKind, val step: SyncSignInStep?)

/** What a pairing from this device came to: who it paired with, and what the other side gave. */
data class SignInsCopied(val device: String, val trackers: List<String>, val clouds: List<SyncFolderKind>, val lacking: List<SyncFolderKind>)

/** What joining a plugin store through a paired device came to: [store] is its name, [device] the paired device's id. */
sealed interface StoreJoin {
    val device: String
    val store: String

    data class Joined(override val device: String, override val store: String) : StoreJoin
    data class Failed(override val device: String, override val store: String, val problem: app.kaeri.engine.model.SyncStoreProblem) : StoreJoin
}

/** The sheets the Devices page opens, one at a time. */
enum class SyncSheet { PICKER, NEXTCLOUD, WEBDAV, KEY, ADD, CODE, NAME, SWITCH }

/** A question about the cloud folder in use: its actions on a TV, or whether to remove it. */
enum class FolderAsk { MENU, REMOVE }

/** What the Devices page and its sheets know of sync and profiles. */
class SyncModel(private val clock: () -> Long = { System.currentTimeMillis() }) {
    private val _status = MutableStateFlow<SyncStatus?>(null)

    /** How sync stands, from the first answer to `LoadSync` on. */
    val status: StateFlow<SyncStatus?> = _status.asStateFlow()

    private val _profiles = MutableStateFlow(ProfilesView())

    /** The profiles: empty for one person, who never meets a picker or a name. */
    val profiles: StateFlow<ProfilesView> = _profiles.asStateFlow()

    private val _signIn = MutableStateFlow<CloudSignIn?>(null)

    /** The cloud sign-in under way, until it is over. */
    val signIn: StateFlow<CloudSignIn?> = _signIn.asStateFlow()

    private val _problem = MutableStateFlow<SyncProblem?>(null)

    /** The last set-up or join problem, until the next try or the sheet is dismissed. */
    val problem: StateFlow<SyncProblem?> = _problem.asStateFlow()

    private val _recoveryKey = MutableStateFlow<String?>(null)

    /** The recovery key to show, until Done. */
    val recoveryKey: StateFlow<String?> = _recoveryKey.asStateFlow()

    private val _invite = MutableStateFlow<PairInvite?>(null)

    /** The pairing code this device shows, until it runs out or a device pairs. */
    val invite: StateFlow<PairInvite?> = _invite.asStateFlow()

    private val _copied = MutableStateFlow<SignInsCopied?>(null)

    /** The last sign-ins a paired device gave. */
    val copied: StateFlow<SignInsCopied?> = _copied.asStateFlow()

    private val _sheet = MutableStateFlow<SyncSheet?>(null)

    /** The sheet open over the Devices page, if one is. */
    val sheet: StateFlow<SyncSheet?> = _sheet.asStateFlow()

    private val _folderAsk = MutableStateFlow<FolderAsk?>(null)

    /** The question open about the cloud folder in use, if one is. */
    val folderAsk: StateFlow<FolderAsk?> = _folderAsk.asStateFlow()

    fun askFolder(ask: FolderAsk?) {
        _folderAsk.value = ask
    }

    /** The folder a join waits on a recovery key for: the one whose group another set of devices holds. */
    var joining: app.kaeri.engine.model.SyncFolderSpec? = null
        private set

    fun show(sheet: SyncSheet?) {
        _sheet.value = sheet
        if (sheet != SyncSheet.KEY) joining = null
    }

    /** Ask for the recovery key that opens [folder]'s group. */
    fun askKey(folder: app.kaeri.engine.model.SyncFolderSpec) {
        joining = folder
        _sheet.value = SyncSheet.KEY
    }

    /** The folder the last set-up asked for, kept so a group that already exists can be joined with its key. */
    var starting: app.kaeri.engine.model.SyncFolderSpec? = null
        private set

    private val _lastCode = MutableStateFlow<Pair<SyncFolderKind, SyncSignInStep.Code>?>(null)

    /** The last code a device sign-in showed, kept so a refusal or lapse can stay on that code's page. */
    val lastCode: StateFlow<Pair<SyncFolderKind, SyncSignInStep.Code>?> = _lastCode.asStateFlow()

    /** A set-up or a change of folder is under way: begun, and neither failed, cancelled nor come through. */
    var working: Boolean = false
        private set

    /** The sign-in came back and the folder is being made: a failure after this is the folder's, not the sign-in's. */
    var passedSignIn: Boolean = false
        private set

    private val _finished = MutableStateFlow(0)

    /** Counts the set-ups that came through: the folder asked for is now the one in use. */
    val finished: StateFlow<Int> = _finished.asStateFlow()

    private val _keyInSheet = MutableStateFlow(false)

    /** The recovery key on show came from a set-up, so the set-up's own sheet carries it, not a full-screen dialog. */
    val keyInSheet: StateFlow<Boolean> = _keyInSheet.asStateFlow()

    /** A set-up through [folder] begins. */
    internal fun begin(folder: app.kaeri.engine.model.SyncFolderSpec) {
        starting = folder
        working = true
        passedSignIn = false
        _keyInSheet.value = false
        _problem.value = null
        _lastCode.value = null
        _pairFailed.value = null
    }

    /** The set-up was cancelled by the person: nothing is under way. */
    internal fun cancelled() {
        working = false
        _settingUp.value = false
        _signIn.value = null
        _lastCode.value = null
    }

    private val _name = MutableStateFlow<String?>(null)

    /** The name this device was given in this session, before the engine has one. */
    val name: StateFlow<String?> = _name.asStateFlow()

    fun rename(name: String) {
        _name.value = name.trim().ifEmpty { null }
    }

    private val _settingUp = MutableStateFlow(false)

    /** A first write to the folder is running: the cloud row says so past 600 ms. */
    val settingUp: StateFlow<Boolean> = _settingUp.asStateFlow()

    internal fun status(status: SyncStatus) {
        val joined = _status.value?.devices?.size?.let { status.devices.size > it } == true
        _status.value = status
        // The folder asked for is the one in use. While a service is being changed the old one
        // is in use still, so any folder says nothing then.
        val asked = starting?.kind
        val inUse = if (asked == null) status.folder != null else status.folder?.kind == asked
        if (inUse) _settingUp.value = false
        if (working && asked != null && inUse) {
            working = false
            _finished.value += 1
        }
        if (joined) _invite.value = null
    }

    internal fun profiles(view: ProfilesView) {
        _profiles.value = view
    }

    internal fun signIn(service: SyncFolderKind, step: SyncSignInStep?) {
        _signIn.value = if (step == null) null else CloudSignIn(service, step)
        if (step is SyncSignInStep.Finishing) _settingUp.value = true
        if (step is SyncSignInStep.Finishing || step is SyncSignInStep.Connecting) passedSignIn = true
        if (step != null) _problem.value = null
        if (step is SyncSignInStep.Code) _lastCode.value = service to step else if (step != null) _lastCode.value = null
    }

    internal fun problem(problem: SyncProblem) {
        _problem.value = problem
        _signIn.value = null
        _settingUp.value = false
        working = false
    }

    internal fun recoveryKey(key: String) {
        _recoveryKey.value = key
        _keyInSheet.value = working
    }

    internal fun invite(code: String, port: Int, seconds: Int) {
        _invite.value = PairInvite(code, port, clock() + seconds * 1000L)
    }

    internal fun copied(copied: SignInsCopied) {
        _copied.value = copied
    }

    /** The pairing success was read: Done on its sheet or page. */
    fun copiedSeen() {
        _copied.value = null
    }

    private val _pairFailed = MutableStateFlow<Pair<app.kaeri.engine.model.SyncPairProblem, String?>?>(null)

    /** Why the last typed code did not pair, and the device that turned it down when one did. */
    val pairFailed: StateFlow<Pair<app.kaeri.engine.model.SyncPairProblem, String?>?> = _pairFailed.asStateFlow()

    /** The DNS-SD the engine was handed for typed-code pairing. */
    internal var network: app.kaeri.engine.ffi.FfiSyncNetwork? = null

    internal fun pairFailed(problem: app.kaeri.engine.model.SyncPairProblem, device: String?) {
        _pairFailed.value = problem to device
        // No device showing a code answered: one may have moved, which a browse below Android 14
        // finds only by resolving it again.
        if (problem == app.kaeri.engine.model.SyncPairProblem.NOT_FOUND) (network as? NsdRelook)?.relook()
    }

    private val _storeJoins = MutableStateFlow<List<StoreJoin>>(emptyList())

    /** Stores joined or not through a paired device, in the order they were answered, until [storeJoinsSeen]. */
    val storeJoins: StateFlow<List<StoreJoin>> = _storeJoins.asStateFlow()

    internal fun storeJoin(join: StoreJoin) {
        _storeJoins.value = _storeJoins.value + join
    }

    /** The answers were shown. */
    fun storeJoinsSeen() {
        _storeJoins.value = emptyList()
    }

    private val _first = MutableStateFlow(false)

    /** True while a pairing runs from the first-launch pages, which word the code and the success as theirs. */
    val first: StateFlow<Boolean> = _first.asStateFlow()

    fun first(on: Boolean) {
        _first.value = on
    }

    /** Done with the recovery key sheet. */
    fun recoveryKeySeen() {
        _recoveryKey.value = null
        _keyInSheet.value = false
    }

    /** The problem was read, or the sheet that showed it was left. */
    fun problemSeen() {
        _pairFailed.value = null
        _problem.value = null
        _lastCode.value = null
    }

    /** The pairing code was stopped or has run out. */
    fun inviteEnded() {
        _invite.value = null
    }

    /** Set up the folder: said by the caller before the first write. */
    fun settingUp(on: Boolean) {
        _settingUp.value = on
    }

    /**
     * The cloud drives this build can sign in to on this device: the ones with a way in, and the
     * ones only a phone can sign in to. A service without a client id, or without a platform, is
     * left out, as the board's decision 14 says.
     */
    fun clouds(): List<SyncCloudView> =
        _status.value?.clouds.orEmpty().filter { val way = it.way; way !is SyncCloudWay.Unavailable || way.reason == CloudUnavailable.NEEDS_PHONE }

    /** The cloud service [kind] in the picker, if this build has it. */
    fun cloud(kind: SyncFolderKind): SyncCloudView? = clouds().firstOrNull { it.service == kind }
}

/** The device that just paired, by [deviceName], with the stores it holds an invite to that this device can join: null when it holds none. */
fun SyncStatus?.storeOffersFrom(deviceName: String): Pair<app.kaeri.engine.model.SyncDeviceView, List<app.kaeri.engine.model.SyncStoreOffer>>? {
    val device = this?.devices?.firstOrNull { !it.thisDevice && it.name == deviceName } ?: return null
    val offers = storesToJoin.filter { it.inviteFrom == device.id }
    return if (offers.isEmpty()) null else device to offers
}

/** Why [this] store was not joined, in words, with [device] the paired device's name. */
fun StoreJoin.Failed.text(context: android.content.Context, device: String): String = when (problem) {
    app.kaeri.engine.model.SyncStoreProblem.OFFLINE -> app.kaeri.i18n.Strings.syncStoreErrOffline(context, store, device)
    app.kaeri.engine.model.SyncStoreProblem.REFUSED -> app.kaeri.i18n.Strings.syncStoreErrRefused(context, store)
    app.kaeri.engine.model.SyncStoreProblem.NOT_ALLOWED -> app.kaeri.i18n.Strings.syncStoreErrNotAllowed(context, store, device)
}

/** The device that just paired, by [deviceName], as the group lists it: null until the status has it. */
fun SyncStatus?.pairedDevice(deviceName: String): app.kaeri.engine.model.SyncDeviceView? =
    this?.devices?.firstOrNull { !it.thisDevice && it.name == deviceName }

/** The sign-ins this device holds, by name: its trackers, then its cloud drives and servers. Empty when it holds none. */
fun app.kaeri.engine.model.SyncDeviceView.signInNames(): List<String> =
    trackers.map { AccountsModel.name(it) } + signIns.mapNotNull { kind ->
        when (kind) {
            SyncFolderKind.GOOGLE_DRIVE -> "Google Drive" // i18n-skip
            SyncFolderKind.DROPBOX -> "Dropbox" // i18n-skip
            SyncFolderKind.ONE_DRIVE -> "OneDrive" // i18n-skip
            SyncFolderKind.NEXTCLOUD -> "Nextcloud" // i18n-skip
            SyncFolderKind.WEB_DAV -> "WebDAV" // i18n-skip
            SyncFolderKind.DIRECTORY -> null
        }
    }
