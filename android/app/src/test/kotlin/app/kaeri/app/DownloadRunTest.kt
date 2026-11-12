package app.kaeri.app

import org.junit.Assert.assertEquals
import org.junit.Test
import app.kaeri.engine.model.BookDownloadRow
import app.kaeri.engine.model.BookDownloadsView
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.StorageMeter
import app.kaeri.store.LiveDownloads
import app.kaeri.store.chaptersBusy

/** What the download run does with episodes and chapters as they stand. */
class DownloadRunTest {
    private val running = LiveDownloads(
        title = "Stardust Wayfarer", text = "Episode 8 · 5 min left", segments = listOf(LiveDownloads.BAR), progress = 374,
        indeterminate = false, chip = "5 min", several = false, paused = false,
    )
    private val paused = running.copy(paused = true, chip = null)

    private fun books(vararg states: ChapterDownloadState) = BookDownloadsView(
        rows = states.mapIndexed { i, state ->
            BookDownloadRow(
                work = "lantern",
                title = "The Lantern Ferry",
                chapter = ChapterRow(id = "lantern/$i", label = "Chapter $i", state = ChapterReadState.Unread, number = "$i"),
                state = state,
            )
        },
        meter = StorageMeter(used = 0uL, free = 20_000_000_000uL),
    )

    @Test
    fun started_again_with_only_chapters_it_waits_for_them_rather_than_ending() {
        // A cold start: the episodes load empty before the chapters answer.
        assertEquals(RunStep.Wait, RunStep.of(null, chaptersBusy(null), chaptersLoaded = false))
        val waiting = books(ChapterDownloadState.Queued, ChapterDownloadState.Downloading(3u, 20u))
        assertEquals(RunStep.Chapters(2), RunStep.of(null, chaptersBusy(waiting), chaptersLoaded = true))
    }

    @Test
    fun it_ends_once_nothing_is_left_or_reading_has_nothing_to_say() {
        assertEquals(RunStep.End, RunStep.of(null, chaptersBusy(books(ChapterDownloadState.Done)), chaptersLoaded = true))
        // Reading off answers the ask with a failure and no view.
        assertEquals(RunStep.End, RunStep.of(null, chaptersBusy(null), chaptersLoaded = true))
    }

    @Test
    fun paused_episodes_keep_it_without_a_grace_while_chapters_download() {
        val downloading = books(ChapterDownloadState.Downloading(3u, 20u))
        assertEquals(RunStep.Episodes(paused, grace = false), RunStep.of(paused, chaptersBusy(downloading), chaptersLoaded = true))
        // Not before the chapters answered either.
        assertEquals(RunStep.Episodes(paused, grace = false), RunStep.of(paused, chaptersBusy(null), chaptersLoaded = false))
        // Once the chapters are done, the paused episodes keep it only a while.
        assertEquals(RunStep.Episodes(paused, grace = true), RunStep.of(paused, chaptersBusy(books(ChapterDownloadState.Done)), chaptersLoaded = true))
        assertEquals(RunStep.Episodes(running, grace = false), RunStep.of(running, 0, chaptersLoaded = true))
    }
}
