// The e-ink Library's On this device keeps the series in the book folders beside the downloaded
// ones: both can be read with no connection.

package app.kaeri.ui.reading

import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.BookDownloadSeries
import app.kaeri.engine.model.BookDownloadsView
import app.kaeri.engine.model.BookGroupRow
import app.kaeri.engine.model.BookSourcesView
import app.kaeri.engine.model.StorageMeter
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class OnThisDeviceTest {
    private fun series(work: String, chapters: UInt) = BookDownloadSeries(
        work = work, title = work, cover = null, chapters = chapters, bytes = 0u, read = 0u, queued = 0u,
        failed = emptyList(), line = emptyList(), progress = null,
    )

    private fun downloads(vararg series: BookDownloadSeries) = BookDownloadsView(
        rows = emptyList(),
        meter = StorageMeter(used = 0u, free = 0u, chapters = 0u, readBytes = 0u),
        series = series.toList(),
    )

    private fun group(id: String, work: String?, kept: Boolean = false) =
        BookGroupRow(group = id, name = id, medium = Medium.MANGA, work = work, matchedTitle = null, files = 3u, kept = kept)

    @Test
    fun folder_series_join_the_downloaded_ones() {
        val sources = BookSourcesView(
            groups = listOf(
                group("g1", "a:900101"),
                group("g2", "l:0123456789abcdef0123456789abcdef", kept = true),
                // Waiting for a look: in no list yet.
                group("g3", null),
            ),
        )
        assertEquals(
            setOf("a:900201", "a:900101", "l:0123456789abcdef0123456789abcdef"),
            onThisDevice(downloads(series("a:900201", 4u), series("a:900202", 0u)), sources),
        )
    }

    @Test
    fun the_downloads_alone_until_the_folders_are_known() {
        assertEquals(setOf("a:900201"), onThisDevice(downloads(series("a:900201", 1u)), null))
    }

    @Test
    fun nothing_until_the_downloads_are_known() {
        assertNull(onThisDevice(null, BookSourcesView(groups = listOf(group("g1", "a:900101")))))
    }
}
