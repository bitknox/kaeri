// New chapters of the series on the Reading list, as the background check finds them.

package app.kaeri.store

import android.graphics.Bitmap
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.BookDownloadsView
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.NewChapter

/** A new chapter to notify of: the engine's `NewChapter`, with its cover fetched for the notification. */
data class NewChapterNote(
    val work: WorkId,
    val title: String,
    val chapter: ChapterId,
    /** The source's own words for it: "Chapter 12". */
    val label: String,
    /** When the source dates it, Unix seconds. */
    val published: Long?,
    /** The chapter the person is up to, when that is before this one: the one Read opens. */
    val nextForYou: ChapterId?,
    val art: Bitmap? = null,
    val banner: Bitmap? = null,
) {
    /** The chapter Read opens: the one the person is up to, or this one when they are caught up. */
    val opens: ChapterId get() = nextForYou ?: chapter

    companion object {
        fun of(found: NewChapter, art: Map<String, Bitmap> = emptyMap()) = NewChapterNote(
            work = found.work,
            title = found.title,
            chapter = found.chapter,
            label = found.label,
            published = found.published,
            nextForYou = found.nextForYou,
            art = found.cover?.let(art::get),
            banner = found.banner?.let(art::get),
        )
    }
}

/**
 * The background check for new chapters, `ReadingCommand::CheckUpdates`: what the periodic work
 * asks, behind an interface a test fakes.
 */
fun interface ChapterCheck {
    /**
     * The chapters to notify of; empty when nothing is new, reading is off or chapter
     * notifications are. Throws when the engine could not be asked, so the work tries again.
     */
    suspend fun newChapters(): List<NewChapterNote>
}

/**
 * The check on the engine, on the app's engine while it runs or a quiet one of its own: it runs
 * the paced check of followed series and takes the chapters it says, each with its cover fetched.
 * The engine says them only while the person reads and New chapters is on.
 */
class EngineChapterCheck(
    private val engine: BackgroundEngine,
    private val coverSize: Int = EngineEpisodeCheck.COVER_SIZE,
) : ChapterCheck {
    override suspend fun newChapters(): List<NewChapterNote> = engine.use { store ->
        store.medium.load()
        val medium = withTimeoutOrNull(MEDIUM_MILLIS) { store.medium.view.filterNotNull().first() }
            ?: throw EngineEpisodeCheck.Unanswered("the medium")
        if (!medium.answer.reads || !medium.readingAvailable) return@use emptyList()
        val found = store.reading.checkNow(CHECK_MILLIS) ?: throw EngineEpisodeCheck.Unanswered("the check for new chapters")
        val art = store.covers(found.flatMap { listOfNotNull(it.cover, it.banner) }.distinct(), coverSize, EngineEpisodeCheck.COVERS_MILLIS)
        found.map { NewChapterNote.of(it, art) }
    }

    companion object {
        const val MEDIUM_MILLIS = 10_000L

        /** The check asks every followed series' source, paced: it has longer than the episodes'. */
        const val CHECK_MILLIS = 120_000L
    }
}

/**
 * Run the paced check of followed series now and wait for it: the chapters it found, or `null`
 * when it did not finish within [timeoutMillis]. The engine says Updates with `checking` while
 * it runs and again once it is done, and the chapters found come right after that.
 */
suspend fun ReadingModel.checkNow(timeoutMillis: Long): List<NewChapter>? = coroutineScope {
    val found = mutableListOf<NewChapter>()
    val listening = launch(start = CoroutineStart.UNDISPATCHED) { newChapters.collect { found += it } }
    try {
        checkUpdates()
        withTimeoutOrNull(timeoutMillis) {
            // The running state can be passed over when the check ends at once.
            withTimeoutOrNull(STARTED_MILLIS) { updates.first { it?.checking == true } }
            updates.first { it != null && !it.checking }
            // What it found is sent next, from the same place.
            delay(FOUND_MILLIS)
            found.toList()
        }
    } finally {
        listening.cancel()
    }
}

/**
 * The chapters waiting or downloading in [view], which keep the download job as episodes do: those
 * waiting for Wi-Fi too, which the job's network rule waits on. A queue the engine stopped for a
 * full device fetches nothing until Retry or another download starts it, so what it still holds
 * keeps nothing.
 */
fun chaptersBusy(view: BookDownloadsView?): Int = view?.let { books ->
    books.rows.count {
        when (it.state) {
            is ChapterDownloadState.Queued, is ChapterDownloadState.WaitsForWifi -> !books.stoppedFull
            is ChapterDownloadState.Downloading -> true
            else -> false
        }
    }
} ?: 0

private const val STARTED_MILLIS = 5_000L
private const val FOUND_MILLIS = 1_000L
