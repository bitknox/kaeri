// Downloads keep the process going: the engine downloads in the app's own process, and while
// anything is queued or running something holds that process, with its notification, a Live
// Update on Android 16 (DownloadNotifications.kt). From Android 14 that is a user-initiated data
// transfer job: Android 15 stops a dataSync foreground service after 6 hours, and a job the user
// started has no such cap. Below 14, which has no such jobs, it is a dataSync foreground service
// (DownloadService), which has no cap there either.

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
import kotlinx.coroutines.Job
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import app.kaeri.store.AppLanguage
import app.kaeri.store.AppStore
import app.kaeri.store.chaptersBusy
import kotlinx.coroutines.launch
import app.kaeri.engine.model.BookDownloadsView
import app.kaeri.engine.model.DownloadRow
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.DownloadState
import app.kaeri.store.DownloadsModel
import app.kaeri.store.DownloadSession
import app.kaeri.store.LiveDownloads

/** Whatever keeps the downloads going on this Android: the job from 14, the service below. */
object Downloads {
    /** Android 14, where user-initiated jobs are. */
    const val JOBS_SDK = Build.VERSION_CODES.UPSIDE_DOWN_CAKE

    /** The ongoing notification's id, the job's or the service's. */
    const val NOTIFICATION = 4101

    /** How long a queue paused from the notification keeps it, for Resume. */
    const val PAUSE_GRACE_MILLIS = 10 * 60_000L

    /**
     * Ask for it while anything waits or runs, from a screen the person is looking at, which is
     * when a user-initiated job or a foreground service may be started. Once it runs, asking
     * again does nothing.
     */
    fun ensure(context: Context, rows: List<DownloadRow>, wifiOnly: Boolean, chapters: Int = 0) {
        if (!DownloadsModel.busy(rows) && chapters == 0) return
        if (Build.VERSION.SDK_INT >= JOBS_SDK) DownloadJob.ensure(context, rows, wifiOnly) else DownloadService.ensure(context)
    }
}

/**
 * What runs while downloads wait or run, as the job or the service: it shows how far they are,
 * and ends once nothing is left. A queue paused from the notification keeps it for
 * [Downloads.PAUSE_GRACE_MILLIS], so its Resume stays to hand; each episode that finishes while
 * it runs gets its own notification, with Play.
 */
internal class DownloadRun(
    private val service: Service,
    /** Shows the notification at once as what runs, or the system stops it. */
    show: (Notification) -> Unit,
    /** Nothing is left to do: the job or the service goes. */
    private val finish: () -> Unit,
) {
    private val app get() = service.application as KaeriApp
    private val scope: CoroutineScope = MainScope()

    init {
        // Started again after the process went, the engine needs to be back first.
        app.start(Flavour.launch(service, Intent()))
        val manager = service.getSystemService(NotificationManager::class.java)
        val notifier = DownloadNotifier(service)
        show(notifier.starting())
        val session = DownloadSession()
        var grace: Job? = null
        val end = {
            Log.i(KaeriApp.TAG, "downloads done, the job ends")
            finish()
            scope.cancel()
        }
        // An engine that failed or stopped has nothing to download with: it ends rather than
        // holding its notification for ever.
        scope.launch {
            app.store.phase.first { it is AppStore.Phase.Failed || it is AppStore.Phase.Stopped }
            Log.w(KaeriApp.TAG, "the engine is not running, the download job ends")
            end()
        }
        scope.launch {
            app.store.loadDownloads(force = true)
            app.store.reading.loadDownloads()
            val reading = app.store.reading
            combine(app.store.downloads.rows, app.store.downloads.rates, app.store.downloads.loaded, reading.downloads, reading.failures) { rows, rates, loaded, books, failures ->
                Queue(rows, rates, loaded, books, books != null || ReadingAbout.DOWNLOADS in failures)
            }.collect { queue ->
                if (!queue.episodesLoaded) return@collect
                session.saw(queue.rows).forEach(notifier::postFinished)
                val live = LiveDownloads.of(service, queue.rows, queue.rates, session.finished)
                val step = RunStep.of(live, chaptersBusy(queue.books), queue.chaptersLoaded)
                if (step !is RunStep.Episodes || !step.grace) {
                    grace?.cancel()
                    grace = null
                }
                when (step) {
                    RunStep.Wait -> {}
                    RunStep.End -> end()
                    is RunStep.Chapters -> manager.notify(Downloads.NOTIFICATION, notifier.chapters(step.left))
                    is RunStep.Episodes -> {
                        // Paused, nothing runs; the notification stays a while for Resume, then goes.
                        if (step.grace && grace == null) grace = launch { delay(Downloads.PAUSE_GRACE_MILLIS); end() }
                        manager.notify(Downloads.NOTIFICATION, notifier.ongoing(step.live))
                    }
                }
            }
        }
    }

    /** The system took it back: the engine keeps the queue for the next time. */
    fun cancel() = scope.cancel()
}

/** The queue as the run sees it: the episodes, and the chapters once the engine answered. */
private class Queue(
    val rows: List<DownloadRow>,
    val rates: Map<Long, Double>,
    val episodesLoaded: Boolean,
    val books: BookDownloadsView?,
    /** The chapters answered, or reading said it has none to give. */
    val chaptersLoaded: Boolean,
)

/** What the download run does with the queue as it stands, once the episodes have loaded. */
internal sealed interface RunStep {
    /** The chapters have not answered yet: started again by the system, they may still wait. */
    data object Wait : RunStep

    /** Nothing is left: the run ends. */
    data object End : RunStep

    /** Chapters alone keep it, saying how many are left. */
    data class Chapters(val left: Int) : RunStep

    /** Episodes keep it; [grace] when they are all paused, so it goes after a while. */
    data class Episodes(val live: LiveDownloads, val grace: Boolean) : RunStep

    companion object {
        /**
         * [live] is the episodes' notification, `null` with none left; [chapters] how many
         * chapters wait or download, which counts only once [chaptersLoaded].
         */
        fun of(live: LiveDownloads?, chapters: Int, chaptersLoaded: Boolean): RunStep = when {
            live != null -> Episodes(live, grace = live.paused)
            chapters > 0 -> Chapters(chapters)
            // The engine says when the chapters are all done (Downloads finished).
            !chaptersLoaded -> Wait
            else -> End
        }
    }
}

/** The job's own service, from Android 14, which alone asks for the job. */
class DownloadJob : JobService() {
    private var run: DownloadRun? = null

    override fun onStartJob(params: JobParameters): Boolean {
        if (Build.VERSION.SDK_INT < Downloads.JOBS_SDK) return false
        run = DownloadRun(
            this,
            // A user-initiated job says what it does at once, or the system stops it.
            show = { setNotification(params, Downloads.NOTIFICATION, it, JOB_END_NOTIFICATION_POLICY_REMOVE) },
            finish = { jobFinished(params, false) },
        )
        return true
    }

    override fun onStopJob(params: JobParameters): Boolean {
        // The system took the job back, or the person stopped it from the task manager: the
        // engine keeps the queue, and the next time the app is open the job is asked for again.
        run?.cancel()
        run = null
        // Run it again when the system allows: the queue is still there to work through.
        return true
    }

    companion object {
        private const val JOB = 4101

        /** Ask for the job; once a job runs, asking again does nothing. */
        @RequiresApi(Downloads.JOBS_SDK)
        fun ensure(context: Context, rows: List<DownloadRow>, wifiOnly: Boolean) {
            val scheduler = context.getSystemService(JobScheduler::class.java)
            if (scheduler.getPendingJob(JOB) != null) return
            val left = rows.filter { it.state != DownloadState.DONE }.sumOf { (it.total - it.downloaded).toLong().coerceAtLeast(0) }
            val job = JobInfo.Builder(JOB, ComponentName(context, DownloadJob::class.java))
                .setUserInitiated(true)
                .setRequiredNetworkType(if (wifiOnly) JobInfo.NETWORK_TYPE_UNMETERED else JobInfo.NETWORK_TYPE_ANY)
                .setEstimatedNetworkBytes(left.coerceAtLeast(1), 0)
                .build()
            val result = runCatching { scheduler.schedule(job) }
            if (result.getOrNull() != JobScheduler.RESULT_SUCCESS) {
                Log.w(KaeriApp.TAG, "the download job was not scheduled: ${result.exceptionOrNull()?.message ?: "refused"}")
            }
        }
    }
}

/**
 * Below Android 14: the same run as a dataSync foreground service, started from a screen the
 * person is using. With Wi-Fi only set, the engine holds the downloads off cellular itself, as it
 * does under the job, and they go on once Wi-Fi is back. Enabled only below 14
 * (`@bool/foreground_fallback`), so a newer Android's manifest has no such service.
 */
class DownloadService : Service() {
    private var run: DownloadRun? = null

    // Its notifications in the chosen language, which Android 13 and below leave to the app.
    override fun attachBaseContext(base: Context) = super.attachBaseContext(AppLanguage.wrap(base))

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (run == null) {
            run = DownloadRun(
                this,
                show = { ServiceCompat.startForeground(this, Downloads.NOTIFICATION, it, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC) },
                finish = {
                    ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
                    stopSelf()
                },
            )
        }
        // Not started again by the system after the process went: the next time the app is open
        // it asks again, as it asks for the job.
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        running = false
        run?.cancel()
        run = null
        super.onDestroy()
    }

    companion object {
        /** Whether the service was started and has not ended: asking again then does nothing. */
        @Volatile
        private var running = false

        fun ensure(context: Context) {
            if (running) return
            running = true
            runCatching { ContextCompat.startForegroundService(context, Intent(context, DownloadService::class.java)) }
                .onFailure {
                    running = false
                    Log.w(KaeriApp.TAG, "the download service did not start: ${it.message}")
                }
        }
    }
}
