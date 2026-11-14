// A flat chapter list, drawn with no volume headings, still offers Download by volume from the
// volume each chapter says it is in.

package app.kaeri.ui.reading

import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.VolumeGroup
import org.junit.Assert.assertEquals
import org.junit.Test

class DownloadVolumesTest {
    private fun row(n: Int, volume: String?) =
        ChapterRow(id = "c$n", label = "Chapter $n", state = ChapterReadState.Unread, number = "$n", volume = volume)

    @Test
    fun a_flat_list_offers_the_volumes_its_rows_say() {
        val flat = VolumeGroup(volume = null, title = null, chapters = listOf(row(1, "1"), row(2, "1"), row(3, "2"), row(4, null)), allRead = false, flat = true)
        val volumes = listOf(flat).downloadVolumes()
        assertEquals(listOf("1", "2"), volumes.map { it.volume })
        assertEquals(listOf(2, 1), volumes.map { it.chapters.size })
        assertEquals(false, volumes.any { it.flat })
    }

    @Test
    fun grouped_volumes_stay_as_they_are() {
        val groups = listOf(
            VolumeGroup(volume = "1", title = null, chapters = listOf(row(1, "1")), allRead = false),
            VolumeGroup(volume = null, title = null, chapters = listOf(row(2, null)), allRead = false),
        )
        assertEquals(groups, groups.downloadVolumes())
    }
}
