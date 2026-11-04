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

/** The sheets the Devices page opens, one at a time. */
enum class SyncSheet { ROAD, PICKER, NEXTCLOUD, WEBDAV, KEY, ADD, CODE, NAME, SWITCH }

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
        if (status.folder != null) _settingUp.value = false
        if (joined) _invite.value = null
    }

    internal fun profiles(view: ProfilesView) {
        _profiles.value = view
    }

    internal fun signIn(service: SyncFolderKind, step: SyncSignInStep?) {
        _signIn.value = if (step == null) null else CloudSignIn(service, step)
        if (step is SyncSignInStep.Finishing) _settingUp.value = true
        if (step != null) _problem.value = null
    }

    internal fun problem(problem: SyncProblem) {
        _problem.value = problem
        _signIn.value = null
        _settingUp.value = false
    }

    internal fun recoveryKey(key: String) {
        _recoveryKey.value = key
    }

    internal fun invite(code: String, port: Int, seconds: Int) {
        _invite.value = PairInvite(code, port, clock() + seconds * 1000L)
    }

    internal fun copied(copied: SignInsCopied) {
        _copied.value = copied
    }

    /** Done with the recovery key sheet. */
    fun recoveryKeySeen() {
        _recoveryKey.value = null
    }

    /** The problem was read, or the sheet that showed it was left. */
    fun problemSeen() {
        _problem.value = null
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
