// The app's update downloads as episodes do: from Android 14 a user-initiated data transfer job,
// below it a dataSync foreground service. Either is asked for as the viewer presses Update, with
// its notification, and stopped by Cancel.

package app.kaeri.app

import android.app.Notification
import android.app.NotificationManager
import android.app.Service
import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.annotation.RequiresApi
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import app.kaeri.store.AppLanguage
import app.kaeri.store.HttpApkFetch
import app.kaeri.store.UpdateOffer
import app.kaeri.store.UpdateState

/** Whatever downloads the update on this Android: the job from 14, the service below. */
object UpdateDownload {
    /** The download's notification, the job's or the service's. */
    const val NOTIFICATION = 4201

    /** Ask for it, as the viewer presses Update: whether Android took it. */
    fun start(context: Context, offer: UpdateOffer): Boolean =
        if (Build.VERSION.SDK_INT >= Downloads.JOBS_SDK) UpdateJob.schedule(context, offer) else UpdateService.start(context)

    /** The viewer's Cancel. */
    fun cancel(context: Context) {
        if (Build.VERSION.SDK_INT >= Downloads.JOBS_SDK) UpdateJob.cancel(context) else UpdateService.cancel(context)
    }
}

/**
 * The update's download, its checks and the hand-over to the installer (`UpdateModel.run`), as
 * the job or the service: it shows how far the download is, and ends once the installer has the
 * file or the update stopped.
 */
internal class UpdateRun(
    service: Service,
    /** Shows the notification at once as what runs, or the system stops it. */
    show: (Notification) -> Unit,
    /** The update is with the installer, failed or was cancelled: the job or the service goes. */
    finish: () -> Unit,
) {
    private val scope: CoroutineScope = MainScope()

    init {
        val app = service.application as KaeriApp
        val updates = app.store.updates
        val notifier = UpdateNotifier(service)
        show(notifier.progress(updates.state.value))
        val manager = service.getSystemService(NotificationManager::class.java)
        scope.launch {
            val shown = launch {
                updates.state.collect { state -> if (state is UpdateState.Downloading) manager.notify(UpdateDownload.NOTIFICATION, notifier.progress(state)) }
            }
            // A run started again after the process went finds nothing pressed, and ends.
            updates.run(HttpApkFetch(plainHttp = BuildConfig.UPDATE_TEST))
            shown.cancel()
            val end = when (val state = updates.state.value) {
                is UpdateState.Installing -> "with the installer" // i18n-skip
                is UpdateState.Failed -> "failed: ${state.failure}" // i18n-skip
                else -> "cancelled" // i18n-skip
            }
            Log.i(KaeriApp.TAG, "update: the job ends, $end")
            finish()
        }
    }

    fun cancel() = scope.cancel()
}

/** The job's own service, from Android 14, which alone asks for the job. */
class UpdateJob : JobService() {
    private val app get() = application as KaeriApp
    private var run: UpdateRun? = null

    override fun onStartJob(params: JobParameters): Boolean {
        if (Build.VERSION.SDK_INT < Downloads.JOBS_SDK) return false
        run = UpdateRun(
            this,
            // A user-initiated job says what it does at once, or the system stops it.
            show = { setNotification(params, UpdateDownload.NOTIFICATION, it, JOB_END_NOTIFICATION_POLICY_REMOVE) },
            finish = { jobFinished(params, false) },
        )
        return true
    }

    override fun onStopJob(params: JobParameters): Boolean {
        // The app's own Cancel has said so already; Android stopping it, as from the task manager,
        // fails the update. Either way the run deletes what it fetched.
        if (Build.VERSION.SDK_INT >= Downloads.JOBS_SDK && params.stopReason != JobParameters.STOP_REASON_CANCELLED_BY_APP) {
            app.store.updates.stopped()
        }
        run?.cancel()
        run = null
        return false
    }

    companion object {
        private const val JOB = 4201

        /** Ask for the job, as the viewer presses Update: whether Android took it. */
        @RequiresApi(Downloads.JOBS_SDK)
        fun schedule(context: Context, offer: UpdateOffer): Boolean {
            val scheduler = context.getSystemService(JobScheduler::class.java)
            val job = JobInfo.Builder(JOB, ComponentName(context, UpdateJob::class.java))
                .setUserInitiated(true)
                .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                .setEstimatedNetworkBytes(offer.bytes.coerceAtLeast(1), 0)
                .build()
            val result = runCatching { scheduler.schedule(job) }
            if (result.getOrNull() != JobScheduler.RESULT_SUCCESS) {
                Log.w(KaeriApp.TAG, "the update job was not scheduled: ${result.exceptionOrNull()?.message ?: "refused"}")
                return false
            }
            return true
        }

        @RequiresApi(Downloads.JOBS_SDK)
        fun cancel(context: Context) {
            context.getSystemService(JobScheduler::class.java).cancel(JOB)
        }
    }
}

/**
 * Below Android 14: the same run as a dataSync foreground service. Only the app's own Cancel
 * stops it from outside, and that has told the update already; Android 13's task manager stops
 * the whole process, and the next start clears what it left. Enabled only below 14
 * (`@bool/foreground_fallback`).
 */
class UpdateService : Service() {
    private var run: UpdateRun? = null

    // Its notification in the chosen language, which Android 13 and below leave to the app.
    override fun attachBaseContext(base: Context) = super.attachBaseContext(AppLanguage.wrap(base))

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (run == null) {
            run = UpdateRun(
                this,
                show = { ServiceCompat.startForeground(this, UpdateDownload.NOTIFICATION, it, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC) },
                finish = {
                    ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
                    stopSelf()
                },
            )
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        run?.cancel()
        run = null
        super.onDestroy()
    }

    companion object {
        fun start(context: Context): Boolean =
            runCatching { ContextCompat.startForegroundService(context, Intent(context, UpdateService::class.java)) }
                .onFailure { Log.w(KaeriApp.TAG, "the update service did not start: ${it.message}") }
                .isSuccess

        fun cancel(context: Context) {
            context.stopService(Intent(context, UpdateService::class.java))
        }
    }
}
