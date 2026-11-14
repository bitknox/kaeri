// The periodic check for new episodes, which the system runs about every six hours on a network
// with the battery not low, and the Mark watched action its notifications carry.

package app.kaeri.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequest
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout
import app.kaeri.engine.core.AnilistId
import app.kaeri.store.BackgroundEngine
import app.kaeri.store.ChapterCheck
import app.kaeri.store.EpisodeCheck

/** What the work and the action need of the application. */
interface NewEpisodesHost {
    /** The check, or `null` while the engine has none. */
    val episodeCheck: EpisodeCheck?

    val episodeNotifier: EpisodeNotifier

    /** The check for new chapters, or `null` where reading has none. */
    val chapterCheck: ChapterCheck? get() = null

    /** Posts the new chapters the check finds, or `null` where reading has none. */
    val readingNotifier: ReadingNotifier? get() = null

    /** Mark [episode] of [id] watched, on the app's engine or one started for it. */
    suspend fun markWatched(id: AnilistId, episode: UInt)

    /**
     * Make the widgets' record, while a widget is placed: what a run does when no notification
     * could show, which the check would otherwise make it after.
     */
    suspend fun recordWidgets() {}
}

/** When the check runs: about every six hours, on a network, with the battery not low. */
object NewEpisodesSchedule {
    /** The unique work's name. */
    const val NAME = "new-episodes"

    const val INTERVAL_HOURS = 6L

    /** The last hour of each interval, which the system picks a moment in. */
    const val FLEX_HOURS = 1L

    val constraints: Constraints
        get() = Constraints.Builder()
            .setRequiredNetworkType(NetworkType.CONNECTED)
            .setRequiresBatteryNotLow(true)
            .build()

    fun request(): PeriodicWorkRequest =
        PeriodicWorkRequestBuilder<NewEpisodesWorker>(INTERVAL_HOURS, TimeUnit.HOURS, FLEX_HOURS, TimeUnit.HOURS)
            .setConstraints(constraints)
            .build()

    /**
     * Check from now on, or stop. Asked again on every launch, which keeps the schedule's place
     * and takes this version's constraints.
     */
    fun set(context: Context, on: Boolean) {
        val work = WorkManager.getInstance(context)
        if (on) {
            work.enqueueUniquePeriodicWork(NAME, ExistingPeriodicWorkPolicy.UPDATE, request())
        } else {
            work.cancelUniqueWork(NAME)
        }
    }
}

/**
 * One check: ask the engine for the episodes that aired since the last one, and post them, then
 * keep the widgets' record; and for the chapters out since the last check of the Reading list,
 * and post them. Nothing is asked of a check while its notification could not show, so no
 * episode or chapter is spent on a check that cannot say it: only the widgets' record is made
 * then. With nothing notifying, the engine's check says nothing, and a run only keeps the widgets'
 * record. A check that fails is tried again later, three times at most.
 */
class NewEpisodesWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result {
        val host = applicationContext as? NewEpisodesHost ?: return Result.failure()
        var failed = false
        val episodes = host.episodeCheck
        if (episodes != null) {
            if (!host.episodeNotifier.canPost) {
                runCatching { host.recordWidgets() }.onFailure { if (it is CancellationException) throw it }
            } else {
                when (val outcome = attempt("episodes") { episodes.newEpisodes() }) {
                    is Outcome.Done -> host.episodeNotifier.post(outcome.value)
                    Outcome.Yielded -> return Result.retry()
                    Outcome.Failed -> failed = true
                }
            }
        }
        val chapters = host.chapterCheck
        val reading = host.readingNotifier
        if (chapters != null && reading != null && reading.canPost(ReadingNotifier.CHAPTERS)) {
            when (val outcome = attempt("chapters") { chapters.newChapters() }) {
                is Outcome.Done -> reading.chapters(outcome.value)
                Outcome.Yielded -> return Result.retry()
                Outcome.Failed -> failed = true
            }
        }
        return if (failed && runAttemptCount < RETRIES) Result.retry() else Result.success()
    }

    private sealed interface Outcome<out T> {
        data class Done<T>(val value: T) : Outcome<T>

        /** The app started as it ran: the next run asks the app's engine. */
        data object Yielded : Outcome<Nothing>

        data object Failed : Outcome<Nothing>
    }

    private suspend fun <T> attempt(what: String, check: suspend () -> T): Outcome<T> = try {
        Outcome.Done(check())
    } catch (e: BackgroundEngine.Yielded) {
        Outcome.Yielded
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        Log.w(KaeriApp.TAG, "the check for new $what failed: ${e.message}")
        Outcome.Failed
    }

    companion object {
        const val RETRIES = 3
    }
}

/**
 * A notification's Mark watched: the notification goes at once, and the episode is marked on the
 * app's engine, or on one started for it while the app is not running.
 */
class EpisodeActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != EpisodeNotifier.ACTION_MARK_WATCHED) return
        val (id, episode) = EpisodeNotifier.watchedOf(intent) ?: return
        val host = context.applicationContext as? NewEpisodesHost ?: return
        host.episodeNotifier.dismiss(EpisodePlan.tagOf(id, episode))
        val pending = goAsync()
        MainScope().launch {
            try {
                // Within the time a receiver has, an engine started for it included.
                withTimeout(MARK_MILLIS) { host.markWatched(id, episode) }
            } catch (e: Exception) {
                Log.w(KaeriApp.TAG, "episode $episode of $id was not marked watched: ${e.message}")
            } finally {
                pending.finish()
                cancel()
            }
        }
    }

    private companion object {
        const val MARK_MILLIS = 9_000L
    }
}
