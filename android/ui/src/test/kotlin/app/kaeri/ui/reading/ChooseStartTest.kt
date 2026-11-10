// Choose chapters starts with the next ten unread chapters not downloaded yet, from the reader's
// place on, rather than every unread chapter of the series.

package app.kaeri.ui.reading

import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.VolumeGroup
import org.junit.Assert.assertEquals
import org.junit.Test

class ChooseStartTest {
    private fun row(n: Int, state: ChapterReadState = ChapterReadState.Unread, download: ChapterDownloadState? = null) =
        ChapterRow(id = "c$n", label = "Chapter $n", state = state, number = "$n", download = download)

    private fun ids(vararg n: Int) = n.map { "c$it" }.toSet()

    private fun groups(rows: List<ChapterRow>) = listOf(VolumeGroup(null, null, rows, false))

    @Test
    fun ten_chapters_from_the_one_in_progress_are_ticked() {
        val rows = (1..30).map { n ->
            when {
                n < 5 -> row(n, ChapterReadState.Read)
                n == 5 -> row(n, ChapterReadState.InProgress(3u, 20u))
                else -> row(n)
            }
        }
        assertEquals(ids(5, 6, 7, 8, 9, 10, 11, 12, 13, 14), groups(rows).chooseStart())
    }

    @Test
    fun the_place_is_after_the_last_read_chapter_and_downloaded_ones_are_skipped() {
        val rows = (1..20).map { n ->
            when {
                n == 2 -> row(n) // skipped before the place
                n <= 6 -> row(n, ChapterReadState.Read)
                n == 8 -> row(n, download = ChapterDownloadState.Done)
                else -> row(n)
            }
        }
        assertEquals(ids(7, 9, 10, 11, 12, 13, 14, 15, 16, 17), groups(rows).chooseStart())
    }

    @Test
    fun a_new_series_starts_from_its_first_chapter() {
        assertEquals(ids(1, 2, 3), groups((1..3).map { row(it) }).chooseStart())
    }

    @Test
    fun with_nothing_left_after_the_place_the_first_unread_ones_are_ticked() {
        val rows = listOf(row(1), row(2, ChapterReadState.Read), row(3, ChapterReadState.Read))
        assertEquals(ids(1), groups(rows).chooseStart())
    }
}
