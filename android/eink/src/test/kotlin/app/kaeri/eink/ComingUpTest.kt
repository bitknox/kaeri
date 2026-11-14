// Coming up on the large end page: the next chapter and those after it, and only the downloaded
// ones when the engine led past chapters that are not on the device.

package app.kaeri.eink

import app.kaeri.engine.model.ChapterDownloadState
import org.junit.Assert.assertEquals
import org.junit.Test

class ComingUpTest {
    private val here = ChapterDownloadState.Done

    private fun row(number: String, download: ChapterDownloadState? = null) =
        EinkFixtures.chapter(id = "c$number", number = number, title = null).copy(download = download)

    private val chapters = listOf(row("4", here), row("4.5"), row("5", here), row("6"), row("7", here))

    @Test
    fun the_next_chapter_and_all_after_it_follow_when_it_is_the_one_after() {
        val coming = comingUp(chapters, "c4", chapters[1])
        assertEquals(listOf("c4.5", "c5", "c6", "c7"), coming.map { it.id })
    }

    @Test
    fun led_past_to_a_download_only_downloads_follow() {
        val coming = comingUp(chapters, "c4", chapters[2])
        assertEquals(listOf("c5", "c7"), coming.map { it.id })
    }

    @Test
    fun a_next_chapter_missing_from_the_list_stands_alone() {
        assertEquals(listOf("c9"), comingUp(chapters, "c4", row("9")).map { it.id })
    }

    @Test
    fun the_next_rows_download_comes_from_the_end_page() {
        val listed = listOf(row("4"), row("5"))
        val coming = comingUp(listed, "c4", row("5", here))
        assertEquals(here, coming.single().download)
    }
}
