// Next 5 unread fetches as the engine does: from the reader's place on, past chapters already here
// or on their way, so a second tap fetches the five after the first; with all of them here the menu
// says so instead of queuing nothing.

package app.kaeri.ui.reading

import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.VolumeGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class NextUnreadTest {
    private fun row(n: Int, state: ChapterReadState = ChapterReadState.Unread, download: ChapterDownloadState? = null) =
        ChapterRow(id = "c$n", label = "Chapter $n", state = state, number = "$n", download = download)

    private fun groups(rows: List<ChapterRow>) = listOf(VolumeGroup(null, null, rows, false))

    private fun List<ChapterRow>.ids() = map { it.id }

    @Test
    fun the_second_tap_fetches_the_five_after_the_first() {
        val first = (1..12).map { n -> if (n <= 2) row(n, ChapterReadState.Read) else row(n) }
        assertEquals((3..7).map { "c$it" }, groups(first).nextToFetch(5).ids())
        val queued = first.map { if (it.id in setOf("c3", "c4", "c5", "c6", "c7")) it.copy(download = ChapterDownloadState.Queued) else it }
        assertEquals((8..12).map { "c$it" }, groups(queued).nextToFetch(5).ids())
    }

    @Test
    fun unread_behind_the_place_are_passed_over_and_a_failed_one_is_asked_again() {
        val rows = (1..9).map { n ->
            when {
                n == 1 -> row(n)
                n <= 4 -> row(n, ChapterReadState.Read)
                n == 5 -> row(n, download = ChapterDownloadState.Failed(ReadingFailure.Offline))
                n == 6 -> row(n, download = ChapterDownloadState.Done)
                else -> row(n)
            }
        }
        assertEquals(listOf("c5", "c7", "c8"), groups(rows).nextToFetch(3).ids())
    }

    @Test
    fun with_every_chapter_ahead_here_nothing_is_left_to_fetch() {
        val rows = (1..6).map { n -> if (n <= 3) row(n, ChapterReadState.Read) else row(n, download = ChapterDownloadState.Done) }
        assertTrue(groups(rows).nextToFetch(5).isEmpty())
        assertEquals(3, groups(rows).unreadAhead().size)
    }
}
