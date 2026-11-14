// How often a page on an e-ink panel is painted again while something runs. Each paint is a panel
// update that builds ghosting until the next full refresh, so a change of state paints at once and
// a change of counts alone waits its turn.

package app.kaeri.ui.reading

import android.os.SystemClock
import app.kaeri.engine.model.BookDownloadsView
import app.kaeri.engine.model.ChapterDownloadState
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.channelFlow
import kotlinx.coroutines.launch

/** How long Downloads on paper holds a change of page counts after its last paint. */
internal const val DOWNLOADS_HOLD_MS = 5_000L

/**
 * The values a panel paints: one whose [shape] differs from the last painted goes through at once;
 * one that differs only in what [shape] leaves out waits until [hold] ms have passed since the
 * last paint, and only the newest of those is painted. Collected on one thread, as Compose does.
 */
internal fun <T> Flow<T>.pacedForPanel(
    hold: Long,
    now: () -> Long = SystemClock::uptimeMillis,
    shape: (T) -> Any?,
): Flow<T> = channelFlow {
    var paintedShape: Any? = Unpainted
    var painted: Any? = Unpainted
    var paintedAt = 0L
    var held: Job? = null
    suspend fun paint(value: T) {
        send(value)
        paintedShape = shape(value)
        painted = value
        paintedAt = now()
    }
    collect { value ->
        held?.cancelAndJoin()
        held = null
        when {
            paintedShape === Unpainted || shape(value) != paintedShape -> paint(value)
            value == painted -> Unit
            else -> {
                val wait = paintedAt + hold - now()
                if (wait <= 0) paint(value) else held = launch { delay(wait); paint(value) }
            }
        }
    }
}

/** Nothing painted yet. */
private object Unpainted

/**
 * What on Downloads is a change of state rather than of counts: the series there, how many of
 * each are here, queued, failed and read, and each chapter's state with its pages left out. A
 * chapter starting, finishing or failing, Stop, or a series arriving or leaving changes it; pages
 * and bytes coming in do not.
 */
internal fun downloadsShape(view: BookDownloadsView?): Any? = view?.let { v ->
    Pair(
        v.series.map { listOf(it.work, it.chapters, it.queued, it.failed, it.read) },
        v.rows.map { row -> Triple(row.work, row.chapter.id, if (row.state is ChapterDownloadState.Downloading) Downloading else row.state) },
    )
}

/** A chapter downloading, whatever its page count. */
private object Downloading
