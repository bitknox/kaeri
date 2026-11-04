// The sync and profile states a shot or recording asks for by name, set up on the testing engine's
// sync scenes: the Fixtures app's `sync` extra. The sign-in and pairing steps are the engine's own
// events with made-up addresses and codes; no service is ever asked.

package app.kaeri.store

import app.kaeri.engine.model.Event
import app.kaeri.engine.model.SyncFolderKind
import app.kaeri.engine.model.SyncFolderSpec
import app.kaeri.engine.model.SyncProblem
import app.kaeri.engine.model.SyncSignInStep
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withTimeoutOrNull

object FixtureSync {
    /** The states [stage] knows, for the log's sake. */
    val STATES = listOf(
        "road", "picker", "nextcloud", "nextcloud-wait", "webdav", "waiting", "finishing", "key", "key-wrong", "recovery-key",
        "add", "code", "name", "offline", "signed-out", "denied", "other-group", "switch", "setting-up", "pair-ok", "first-code", "first-ok",
        "google-code", "onedrive-code", "code-expired", "dropbox-phone", "dropbox-nophone", "nextcloud-qr", "who",
    )

    private const val CODE_URL = "https://activate.example/device"
    private const val PAGE_URL = "https://cloud.example/index.php/login/v2/flow/0000"

    /** Set up [state] once the engine has said how sync stands. False for a state there is none of. */
    suspend fun stage(store: AppStore, state: String): Boolean {
        when (state) {
            "first-code" -> {
                store.sync.first(true)
                store.sync.show(SyncSheet.CODE)
                store.apply(Event.SyncInvite("428917", 41234u, 105u))
                return true
            }
            "first-ok" -> {
                store.sync.first(true)
                store.apply(Event.SyncSignInsCopied("Kim's phone", listOf("anilist"), listOf(SyncFolderKind.DROPBOX), emptyList()))
                return true
            }
        }
        store.loadSync()
        withTimeoutOrNull(30_000) { store.sync.status.first { it != null } } ?: return false
        val now = System.currentTimeMillis() / 1000
        val spec = SyncFolderSpec(SyncFolderKind.GOOGLE_DRIVE, null, null, null)
        when (state) {
            "road" -> store.sync.show(SyncSheet.ROAD)
            "picker" -> store.sync.show(SyncSheet.PICKER)
            "nextcloud" -> store.sync.show(SyncSheet.NEXTCLOUD)
            "nextcloud-wait" -> {
                store.sync.show(SyncSheet.NEXTCLOUD)
                store.apply(Event.SyncSignIn(SyncFolderKind.NEXTCLOUD, SyncSignInStep.Page(PAGE_URL)))
            }
            "webdav" -> store.sync.show(SyncSheet.WEBDAV)
            "waiting" -> {
                store.sync.show(SyncSheet.PICKER)
                store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, SyncSignInStep.Browser(PAGE_URL)))
            }
            "finishing" -> store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, SyncSignInStep.Finishing))
            "setting-up" -> store.sync.settingUp(true)
            "key" -> store.sync.askKey(spec)
            "key-wrong" -> {
                store.sync.askKey(spec)
                store.apply(Event.SyncProblem(SyncProblem.WrongRecoveryKey))
            }
            "other-group" -> {
                store.sync.show(SyncSheet.PICKER)
                store.apply(Event.SyncProblem(SyncProblem.GroupExists))
            }
            "recovery-key" -> store.apply(Event.SyncRecoveryKey("K7QM2WXD9PLR4ZTN8VBH3GFC"))
            "add" -> store.sync.show(SyncSheet.ADD)
            "code" -> {
                store.sync.show(SyncSheet.CODE)
                store.apply(Event.SyncInvite("428917", 41234u, 105u))
            }
            "name" -> store.sync.show(SyncSheet.NAME)
            "offline" -> store.apply(Event.SyncStatus(store.sync.status.value!!.copy(error = "offline", problem = SyncProblem.Unreachable)))
            "signed-out" -> store.apply(
                Event.SyncStatus(store.sync.status.value!!.copy(error = "signed out", problem = SyncProblem.SignInNeeded(SyncFolderKind.GOOGLE_DRIVE))),
            )
            "denied" -> {
                store.sync.show(SyncSheet.PICKER)
                store.apply(Event.SyncProblem(SyncProblem.SignInDenied(SyncFolderKind.GOOGLE_DRIVE)))
            }
            "switch" -> store.sync.show(SyncSheet.SWITCH)
            "pair-ok" -> store.apply(Event.SyncSignInsCopied("Kim's phone", listOf("anilist"), listOf(SyncFolderKind.DROPBOX), emptyList()))
            "first-code" -> {
                store.sync.first(true)
                store.sync.show(SyncSheet.CODE)
                store.apply(Event.SyncInvite("428917", 41234u, 105u))
            }
            "first-ok" -> {
                store.sync.first(true)
                store.apply(Event.SyncSignInsCopied("Kim's phone", listOf("anilist"), listOf(SyncFolderKind.DROPBOX), emptyList()))
            }
            "google-code" -> store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, SyncSignInStep.Code("WXRTKLMN", CODE_URL, (now + 899).toLong(), 5u)))
            "onedrive-code" -> store.apply(Event.SyncSignIn(SyncFolderKind.ONE_DRIVE, SyncSignInStep.Code("B4NQ7XLK", CODE_URL, (now + 899).toLong(), 5u)))
            "code-expired" -> store.apply(Event.SyncSignIn(SyncFolderKind.GOOGLE_DRIVE, SyncSignInStep.Code("WXRTKLMN", CODE_URL, (now - 5).toLong(), 5u)))
            "dropbox-phone", "dropbox-nophone" -> store.sync.show(SyncSheet.KEY)
            "nextcloud-qr" -> store.apply(Event.SyncSignIn(SyncFolderKind.NEXTCLOUD, SyncSignInStep.Page(PAGE_URL)))
            "who" -> store.sync.show(SyncSheet.SWITCH)
            else -> return false
        }
        // The sheet's own animation in.
        delay(800)
        return true
    }
}
