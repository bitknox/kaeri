// Google Drive's sign-in on a phone, through Play services' authorization: Google's own sheet in
// place of a browser tab, and a short-lived token each time the engine asks for one. The grant
// stays with Play services, so the engine keeps no refresh token for Drive here.

package app.kaeri.app

import android.accounts.Account
import android.app.Activity
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.IntentSenderRequest
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import app.kaeri.engine.ffi.FfiCloudAuthorizer
import app.kaeri.engine.ffi.FfiCloudGrant
import app.kaeri.engine.model.SyncFolderKind
import com.google.android.gms.auth.api.identity.AuthorizationClient
import com.google.android.gms.auth.api.identity.AuthorizationRequest
import com.google.android.gms.auth.api.identity.AuthorizationResult
import com.google.android.gms.auth.api.identity.ClearTokenRequest
import com.google.android.gms.auth.api.identity.Identity
import com.google.android.gms.auth.api.identity.RevokeAccessRequest
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability
import com.google.android.gms.common.api.ApiException
import com.google.android.gms.common.api.CommonStatusCodes
import com.google.android.gms.common.api.Scope
import com.google.android.gms.tasks.Task
import com.google.android.gms.tasks.Tasks
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.mapNotNull
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull

/**
 * The engine's way to Google Drive on a phone. Called on the engine's blocking threads, never the
 * main thread: a call without a sheet waits on Play services, and one with a sheet waits for the
 * person. The sheet itself is the activity's to show ([showIn]), since only an activity in front
 * can start it. Tokens are handed to the engine and never logged or kept.
 */
internal class GoogleDriveAccess(context: Context) : FfiCloudAuthorizer {
    private val app = context.applicationContext
    private val client: AuthorizationClient by lazy { Identity.getAuthorizationClient(app) }

    /** Google's consent sheet, waiting for the activity to show it, and the answer it came back with. */
    private class Consent(val intent: PendingIntent) {
        val answer = CompletableDeferred<Intent?>()
        val shown = AtomicBoolean(false)
    }

    private val consent = MutableStateFlow<Consent?>(null)
    private val again = MutableSharedFlow<Unit>(extraBufferCapacity = 1)

    /** The account the last grant was for, to take it back when the engine names none. */
    @Volatile
    private var lastAccount: String? = null

    override fun serves(service: SyncFolderKind): Boolean = service == SyncFolderKind.GOOGLE_DRIVE

    override fun authorize(service: SyncFolderKind, interactive: Boolean, account: String?, rejected: String?): FfiCloudGrant {
        if (!serves(service)) return FfiCloudGrant.Unavailable
        if (GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(app) != ConnectionResult.SUCCESS) {
            return FfiCloudGrant.Unavailable
        }
        return try {
            // A token Drive refused leaves Play services' cache, so the next one is fresh.
            rejected?.let { token -> runCatching { await(client.clearToken(ClearTokenRequest.builder().setToken(token).build())) } }
            val result = await(client.authorize(request(account)))
            when {
                !result.hasResolution() -> granted(result)
                !interactive -> FfiCloudGrant.ConsentNeeded
                else -> result.pendingIntent?.let(::consented) ?: FfiCloudGrant.ConsentNeeded
            }
        } catch (e: ExecutionException) {
            failure(e.cause)
        } catch (_: TimeoutException) {
            FfiCloudGrant.Unreachable
        } catch (_: InterruptedException) {
            Thread.currentThread().interrupt()
            FfiCloudGrant.Cancelled
        }
    }

    override fun signedOut(service: SyncFolderKind, account: String?) {
        if (!serves(service)) return
        val email = account?.takeIf(::isEmail) ?: lastAccount ?: return
        lastAccount = null
        val request = RevokeAccessRequest.builder()
            .setAccount(Account(email, GOOGLE_ACCOUNT))
            .setScopes(listOf(Scope(DRIVE_APP_DATA)))
            .build()
        runCatching { client.revokeAccess(request) }
            .onSuccess { task -> task.addOnFailureListener { Log.i(TAG, "Google Drive's grant could not be taken back") } }
            .onFailure { Log.i(TAG, "Google Drive's grant could not be taken back") }
    }

    /**
     * Show Google's sheet over [activity] whenever a sign-in waits for it, through [launcher],
     * which hands the answer to [answered]. Once per sheet, unless Open the page again asks.
     */
    fun showIn(activity: ComponentActivity, launcher: ActivityResultLauncher<IntentSenderRequest>) {
        activity.lifecycleScope.launch {
            activity.repeatOnLifecycle(Lifecycle.State.STARTED) {
                merge(consent.filterNotNull(), again.mapNotNull { consent.value }).collect { waiting ->
                    if (waiting.shown.compareAndSet(false, true)) {
                        runCatching { launcher.launch(IntentSenderRequest.Builder(waiting.intent).build()) }
                            .onFailure { waiting.answer.complete(null) }
                    }
                }
            }
        }
    }

    /** What Google's sheet came back with: the grant in [data] when the person allowed access. */
    fun answered(resultCode: Int, data: Intent?) {
        consent.value?.answer?.complete(data.takeIf { resultCode == Activity.RESULT_OK })
    }

    /** Show the waiting sheet once more. False when no sign-in waits for one. */
    fun showAgain(): Boolean {
        val waiting = consent.value ?: return false
        waiting.shown.set(false)
        return again.tryEmit(Unit)
    }

    private fun request(account: String?): AuthorizationRequest =
        AuthorizationRequest.builder()
            .setRequestedScopes(listOf(Scope(DRIVE_APP_DATA)))
            .apply { account?.takeIf(::isEmail)?.let { setAccount(Account(it, GOOGLE_ACCOUNT)) } }
            .build()

    /** Wait for the person on Google's sheet, then read the grant it came back with. */
    private fun consented(intent: PendingIntent): FfiCloudGrant {
        val waiting = Consent(intent)
        consent.value = waiting
        val data = try {
            runBlocking { withTimeoutOrNull(SHEET_WAIT_MILLIS) { waiting.answer.await() } }
        } finally {
            consent.compareAndSet(waiting, null)
        }
        data ?: return FfiCloudGrant.Cancelled
        return try {
            granted(client.getAuthorizationResultFromIntent(data))
        } catch (e: ApiException) {
            failure(e)
        }
    }

    private fun granted(result: AuthorizationResult): FfiCloudGrant {
        if (DRIVE_APP_DATA !in result.grantedScopes) return FfiCloudGrant.Denied
        val token = result.accessToken?.takeIf { it.isNotEmpty() } ?: return FfiCloudGrant.Failed("Play services gave no token")
        val account = result.toGoogleSignInAccount()?.email
        if (account != null) lastAccount = account
        return FfiCloudGrant.Granted(token, account)
    }

    private fun failure(cause: Throwable?): FfiCloudGrant {
        val code = (cause as? ApiException)?.statusCode ?: return FfiCloudGrant.Failed("Play services failed")
        return when (code) {
            CommonStatusCodes.NETWORK_ERROR, CommonStatusCodes.TIMEOUT -> FfiCloudGrant.Unreachable
            CommonStatusCodes.CANCELED -> FfiCloudGrant.Cancelled
            ConnectionResult.SERVICE_MISSING, ConnectionResult.SERVICE_VERSION_UPDATE_REQUIRED,
            ConnectionResult.SERVICE_DISABLED, ConnectionResult.SERVICE_INVALID,
            -> FfiCloudGrant.Unavailable
            else -> FfiCloudGrant.Failed("Play services answered ${CommonStatusCodes.getStatusCodeString(code)}")
        }
    }

    private fun <T> await(task: Task<T>): T = Tasks.await(task, CALL_WAIT_SECONDS, TimeUnit.SECONDS)

    private companion object {
        const val TAG = "kaeri"
        /** The scope Kaeri asks of Drive on every platform: its own hidden app data folder. */
        const val DRIVE_APP_DATA = "https://www.googleapis.com/auth/drive.appdata"
        const val GOOGLE_ACCOUNT = "com.google"
        const val CALL_WAIT_SECONDS = 30L
        /** As long as the engine waits for a sign-in page. */
        const val SHEET_WAIT_MILLIS = 600_000L

        fun isEmail(account: String) = '@' in account
    }
}
