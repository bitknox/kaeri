package app.kaeri.ui

import androidx.compose.ui.text.intl.LocaleList
import androidx.compose.ui.unit.dp
import app.kaeri.design.Reveals
import app.kaeri.design.tagged
import app.kaeri.engine.ffi.episodeFold
import app.kaeri.engine.ffi.episodeRangeToOpen
import app.kaeri.engine.ffi.episodeText
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.EpisodeText
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The rules of the Netflix-style episode row: which rows are tall, how tall, what spoiler
 * protection hides, and how a long series splits. The ranges and the text outlook are the
 * engine's, called as the app calls them.
 */
class EpisodeSynopsisTest {
    private fun row(number: Int, description: String? = null, completed: Boolean = false, pending: Boolean = false, airsAt: Long? = null, ahead: Boolean = false) = EpisodeRow(
        number = number.toString(), title = "Episode title $number", thumbnail = null, description = description,
        durationSecs = 1440u, watched = 0.0, completed = completed, kind = null, skippable = false,
        airsAt = airsAt, synopsisPending = pending, spoilerAhead = ahead,
    )

    @Test
    fun a_row_is_tall_only_where_text_can_land() {
        assertEquals(RowForm.TALL, rowForm(EpisodeText.PRESENT, protectedRow = false))
        assertEquals(RowForm.TALL, rowForm(EpisodeText.EXPECTED, protectedRow = false))
        assertEquals(RowForm.SHORT, rowForm(EpisodeText.NONE, protectedRow = false))
        assertEquals(RowForm.SHORT, rowForm(EpisodeText.PRESENT, protectedRow = true))
        assertEquals(RowForm.SHORT, rowForm(EpisodeText.PRESENT, protectedRow = false, unaired = true))
    }

    @Test
    fun rows_are_172_dp_tall_and_100_short_at_the_default_text_size() {
        val header = RowDims.StillHeight
        val band = 60.dp
        assertEquals(172.dp, rowHeight(RowForm.TALL, header, band))
        assertEquals(100.dp, rowHeight(RowForm.SHORT, header, band))
    }

    @Test
    fun the_outlook_comes_from_the_table() {
        assertEquals(EpisodeText.NONE, episodeText(listOf(row(1), row(2))))
        assertEquals(EpisodeText.EXPECTED, episodeText(listOf(row(1, pending = true), row(2))))
        assertEquals(EpisodeText.PRESENT, episodeText(listOf(row(1), row(2, description = "A storm closes the pass."))))
    }

    @Test
    fun protection_hides_the_rows_the_engine_says_are_ahead_once_aired() {
        assertFalse(hiddenBySpoilers(row(7), protect = true, unaired = false))
        assertTrue(hiddenBySpoilers(row(8, ahead = true), protect = true, unaired = false))
        assertFalse(hiddenBySpoilers(row(8, ahead = true), protect = false, unaired = false))
        assertFalse(hiddenBySpoilers(row(8, ahead = true), protect = true, unaired = true))
    }

    @Test
    fun a_reveal_covers_one_episode_or_all_for_the_visit() {
        val reveals = Reveals()
        assertFalse(reveals.isShown("8"))
        reveals.show("8")
        assertTrue(reveals.isShown("8"))
        assertFalse(reveals.isShown("9"))
        assertFalse(reveals.everything)
        reveals.showAll()
        assertTrue(reveals.isShown("9"))
        assertTrue(reveals.everything)
        // The line's Hide covers them again, the one revealed alone too.
        reveals.hideAll()
        assertFalse(reveals.everything)
        assertFalse(reveals.isShown("8"))
    }

    @Test
    fun the_protection_line_names_the_row_before_the_first_one_hidden() {
        // A long runner whose progress points elsewhere: the rows decide, not the progress.
        val rows = (1..230).map { row(it, completed = it <= 150, ahead = it > 151) }
        val first = rows.first { it.spoilerAhead }
        assertEquals(151, protectionStart(rows, first))
        // A range that starts hidden names the last row of the range before it.
        val later = (1..230).map { row(it, completed = it <= 99, ahead = it > 200) }
        assertEquals(200, protectionStart(later, later.first { it.spoilerAhead }))
        assertNull(protectionStart(rows, rows.first()))
    }

    @Test
    fun a_synopsis_is_tagged_with_its_language() {
        val text = tagged("A storm closes the pass.", "en")
        assertEquals(1, text.spanStyles.size)
        assertEquals(LocaleList("en"), text.spanStyles.single().item.localeList)
        assertEquals(0, tagged("A storm closes the pass.", null).spanStyles.size)
    }

    @Test
    fun a_long_series_splits_into_ranges_of_100_opening_on_the_next_episode() {
        val rows = (1..230).map { row(it, completed = it <= 150) }
        val ranges = rangesOf(rows, now = 1_700_000_000L)
        assertEquals(3, ranges.size)
        assertEquals(1 to 100, rangeBounds(rows, ranges[0]))
        assertEquals(201 to 230, rangeBounds(rows, ranges[2]))
        assertEquals(30, rangeRows(rows, ranges[2]).size)
        assertTrue(ranges[0].watched)
        assertFalse(ranges[1].watched)
        // The next episode is 151, in the second range, where the first 50 rows are watched.
        assertEquals(1, episodeRangeToOpen(rows).toInt())
        assertEquals(50, episodeFold(rows, ranges[1]).toInt())
    }

    @Test
    fun a_short_series_has_no_ranges() {
        assertTrue(rangesOf((1..100).map { row(it) }, now = 1_700_000_000L).isEmpty())
    }

    @Test
    fun a_range_marks_episodes_not_out_yet() {
        val future = 4_000_000_000L
        val rows = (1..150).map { row(it, completed = it <= 100, airsAt = if (it > 100) future else null) }
        val ranges = rangesOf(rows, now = 1_700_000_000L)
        assertTrue(ranges[0].watched)
        assertTrue(ranges[1].notOut)
        assertNull(rows[0].description)
    }
}
