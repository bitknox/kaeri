// A series page whose source fails keeps the chapter list the source gave before under the
// failure's note, and its main button still reads from it. With no list kept, the failure stands
// alone and the button leads to another source.

package app.kaeri.ui.reading

import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.SeriesAction
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.store.ReadChapters
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class KeptChaptersTest {
    private fun row(n: Int, download: ChapterDownloadState? = null) =
        ChapterRow(id = "c$n", label = "$n", state = ChapterReadState.Unread, number = "$n", download = download)

    /** Three chapters the source listed before, the first downloaded. */
    private val kept = listOf(VolumeGroup(volume = null, title = null, chapters = listOf(row(1, ChapterDownloadState.Done), row(2), row(3)), allRead = false))

    /** Continue at the second chapter, which is not downloaded. */
    private val action = SeriesAction.Continue(chapter = "c2", label = "2", page = 1u)

    @Test
    fun a_failed_refresh_keeps_the_list_the_source_gave_before() {
        val chapters = ReadChapters(edition = null, volumes = kept, failure = ReadingFailure.Offline)
        assertEquals(listOf("c1", "c2", "c3"), chapters.listedGroups().flatMap { it.chapters }.map { it.id })
        assertFalse(chapters.leadsElsewhere(action))
    }

    @Test
    fun a_failure_with_no_list_kept_leads_to_another_source() {
        val chapters = ReadChapters(edition = null, volumes = emptyList(), failure = ReadingFailure.Offline)
        assertTrue(chapters.listedGroups().isEmpty())
        assertTrue(chapters.leadsElsewhere(action))
    }

    @Test
    fun a_list_with_no_failure_is_listed_as_it_came() {
        val chapters = ReadChapters(edition = null, volumes = kept, failure = null)
        assertEquals(kept, chapters.listedGroups())
        assertFalse(chapters.leadsElsewhere(action))
    }
}
