package app.kaeri.store

import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.UpdateDay
import app.kaeri.engine.model.UpdateRow
import app.kaeri.engine.model.UpdatesView
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.engine.model.WorkTile
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The reader opened from a row knows that row's chapter before the engine answers: from
 * Updates, which lists chapters of series whose page was never opened, and from a series' own
 * chapter list. A chapter of the same id in another series is not taken for it.
 */
class ReaderChapterRowTest {
    private val model = ReadingModel {}

    private fun row(id: String, number: String) =
        ChapterRow(id = id, label = "Chapter $number", state = ChapterReadState.Unread, number = number)

    @Test
    fun an_updates_row_names_its_chapter() {
        val tile = WorkTile(work = "a:ferry", title = "The Lantern Ferry", medium = Medium.MANGA)
        val other = WorkTile(work = "a:kite", title = "Paper Kite", medium = Medium.MANGA)
        model.apply(
            ReadingEvent.Updates(
                UpdatesView(days = listOf(UpdateDay(0L, listOf(UpdateRow(other, row("22", "5"), "Shelf"), UpdateRow(tile, row("22", "22"), "Shelf"))))),
            ),
        )
        assertEquals("22", model.chapterRow("a:ferry", "22")?.number)
        assertNull(model.chapterRow("a:ferry", "23"))
        assertNull(model.chapterRow("a:ferry", null))
    }

    @Test
    fun a_series_list_names_its_chapter() {
        model.apply(ReadingEvent.Chapters("a:ferry", null, listOf(VolumeGroup(null, null, listOf(row("c-7", "7")), false)), null))
        assertEquals("7", model.chapterRow("a:ferry", "c-7")?.number)
    }
}
