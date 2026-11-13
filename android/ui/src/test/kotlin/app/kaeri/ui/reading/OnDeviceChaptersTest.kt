// A folder's chapters are files on this device: nothing offers to download them, Choose chapters
// and Next unread pass over them, and the series' Download button rests when it has nothing to do.

package app.kaeri.ui.reading

import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.VolumeGroup
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class OnDeviceChaptersTest {
    private fun row(n: Int, onDevice: Boolean = false, download: ChapterDownloadState? = null) =
        ChapterRow(id = "c$n", label = "Chapter $n", state = ChapterReadState.Unread, number = "$n", download = download, onDevice = onDevice)

    private fun groups(rows: List<ChapterRow>) = listOf(VolumeGroup(null, null, rows, false))

    @Test
    fun a_folders_chapters_offer_no_download() {
        val folder = groups((1..3).map { row(it, onDevice = true) })
        assertFalse(folder.chapterRows().any { it.downloadable })
        assertEquals(emptySet<String>(), folder.chooseStart())
        assertEquals(emptyList<ChapterRow>(), folder.unreadAhead())
        assertEquals(emptyList<ChapterRow>(), folder.nextToFetch(5))
        assertFalse("the Download button rests", folder.hasDownloads())
    }

    @Test
    fun a_sources_chapters_still_do() {
        val source = groups((1..3).map { row(it) })
        assertEquals(setOf("c1", "c2", "c3"), source.chooseStart())
        assertEquals(3, source.nextToFetch(5).size)
        assertTrue(source.hasDownloads())
        // With every chapter downloaded, the button stays for Send and Delete.
        assertTrue(groups(listOf(row(1, download = ChapterDownloadState.Done))).hasDownloads())
    }
}
