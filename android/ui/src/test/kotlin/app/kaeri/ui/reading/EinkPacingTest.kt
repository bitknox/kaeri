// The e-ink shell's pacing and fitting: Downloads paints a change of state at once and page counts
// at most every five seconds, Home tells a 7 inch panel from the whole window and gives up its
// second New chapters row when the page would not fit, and a row's grid fits three rows a page.

package app.kaeri.ui.reading

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.kaeri.eink.EinkSizes
import app.kaeri.engine.model.BookDownloadRow
import app.kaeri.engine.model.BookDownloadSeries
import app.kaeri.engine.model.BookDownloadsView
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.StorageMeter
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class EinkPacingTest {
    /** A value with a state and a count, paced on its state alone. */
    private data class Step(val state: String, val count: Int)

    @Test
    fun a_change_of_state_paints_at_once_and_counts_wait_five_seconds() = runTest {
        val source = MutableStateFlow(Step("one", 0))
        val painted = mutableListOf<Step>()
        val job = launch { source.pacedForPanel(DOWNLOADS_HOLD_MS, now = { testScheduler.currentTime }) { it.state }.collect { painted += it } }
        runCurrent()
        assertEquals(listOf(Step("one", 0)), painted)

        // Counts every 2 s, as the engine sends them: held, and only the newest painted at 5 s.
        advanceTimeBy(2_000); source.value = Step("one", 1); runCurrent()
        advanceTimeBy(2_000); source.value = Step("one", 2); runCurrent()
        assertEquals(1, painted.size)
        advanceTimeBy(999); runCurrent()
        assertEquals(1, painted.size)
        advanceTimeBy(1); runCurrent()
        assertEquals(Step("one", 2), painted.last())
        assertEquals(2, painted.size)

        // A chapter finishing paints at once, and the count held before it is never painted.
        advanceTimeBy(1_000); source.value = Step("one", 3); runCurrent()
        source.value = Step("two", 3); runCurrent()
        assertEquals(Step("two", 3), painted.last())
        advanceTimeBy(10_000); runCurrent()
        assertEquals(3, painted.size)

        // Past the hold, a count paints as it comes.
        source.value = Step("two", 4); runCurrent()
        assertEquals(Step("two", 4), painted.last())
        job.cancel()
    }

    private fun row(chapter: String, state: ChapterDownloadState) = BookDownloadRow(
        work = WORK,
        title = "The Lantern Ferry",
        chapter = ChapterRow(id = chapter, label = "Chapter ${chapter.substringAfterLast('/')}", state = ChapterReadState.Unread, number = chapter.substringAfterLast('/')),
        state = state,
    )

    private fun view(pages: UInt, done: UInt = 1u, queued: UInt = 3u, failed: List<String> = emptyList(), free: ULong = 20_000_000_000uL) = BookDownloadsView(
        rows = listOf(
            row("$WORK/14", ChapterDownloadState.Done),
            row("$WORK/15", ChapterDownloadState.Downloading(pages, 20u)),
            row("$WORK/16", ChapterDownloadState.Queued),
        ),
        meter = StorageMeter(used = 9_400_000uL + pages * 100_000uL, free = free),
        series = listOf(
            BookDownloadSeries(work = WORK, title = "The Lantern Ferry", chapters = done, bytes = 9_400_000uL + pages * 100_000uL, queued = queued, failed = failed),
        ),
    )

    @Test
    fun pages_and_bytes_are_counts_and_chapters_are_state() {
        assertEquals(downloadsShape(view(6u)), downloadsShape(view(9u, free = 19_000_000_000uL)))
        assertNotEquals(downloadsShape(view(6u)), downloadsShape(view(6u, done = 2u, queued = 2u)))
        assertNotEquals(downloadsShape(view(6u)), downloadsShape(view(6u, failed = listOf("$WORK/13"))))
        val stopped = view(6u).let { it.copy(series = emptyList()) }
        assertNotEquals(downloadsShape(view(6u)), downloadsShape(stopped))
        assertNotEquals(downloadsShape(view(6u)), downloadsShape(null))
    }

    @Test
    fun the_downloading_line_leaves_the_totals_to_on_this_device() {
        val series = BookDownloadSeries(
            work = WORK, title = "The Lantern Ferry", chapters = 1u, queued = 3u,
            line = listOf(
                Msg.Reading(ReadingMsg.ReadingChapters(1L)),
                Msg.Reading(ReadingMsg.ReadingDlRead(1L)),
                Msg.Reading(ReadingMsg.ReadingDlChapter("15")),
                Msg.Reading(ReadingMsg.ReadingDlPages(6L, 20L)),
                Msg.Reading(ReadingMsg.ReadingDlMoreQueued(2L)),
            ),
        )
        assertEquals(
            listOf(
                Msg.Reading(ReadingMsg.ReadingDlChapter("15")),
                Msg.Reading(ReadingMsg.ReadingDlPages(6L, 20L)),
                Msg.Reading(ReadingMsg.ReadingDlMoreQueued(2L)),
            ),
            movingLine(series),
        )
    }

    @Test
    fun a_7_inch_panel_is_told_from_the_whole_window_on_every_version() {
        // The boards' panels, whole: 6 inch 572 × 772 dp, 7 inch 674 × 896 dp, at 300 ppi.
        assertFalse(tallPanel(1448, 1.875f))
        assertTrue(tallPanel(1680, 1.875f))
        // A 7 inch panel at 320 dpi is 840 dp: still 7 inch.
        assertTrue(tallPanel(1680, 2f))
        // The 10.3 inch panel.
        assertTrue(tallPanel(2480, 1.875f))
        assertFalse(tallPanel(0, 0f))
    }

    @Test
    fun the_second_new_chapters_row_gives_way_only_when_the_page_would_not_fit() {
        val heights = listOf(300, 200, 50, 66, 66, 70)
        val sheddable = listOf(false, false, false, false, true, false)
        assertEquals(emptySet<Int>(), linesToShed(heights, sheddable, 752))
        assertEquals(setOf(4), linesToShed(heights, sheddable, 751))
        // Too tall even without it: it leaves, and the rest scrolls.
        assertEquals(setOf(4), linesToShed(heights, sheddable, 400))
        assertEquals(emptySet<Int>(), linesToShed(heights, List(heights.size) { false }, 100))
    }

    /** A grid's rows with their covers and words fit the page, the pager under them. */
    private fun fits(grid: PaperGrid, height: Dp, words: Dp) =
        (grid.cover * 1.5f + 24.dp + words) * grid.rows + EinkSizes.PagerRow <= height

    @Test
    fun a_grid_takes_its_columns_from_the_width_and_its_rows_from_the_height() {
        // The page under the title on each panel (the window less 232 dp of bars and title), at
        // the Air's 0.85 text: 36 and 40 dp of words.
        val small = 36.dp
        val large = 40.dp
        // 6 inch (572 × 772 dp): three by three, as the boards draw it.
        val six = paperGrid(516.dp, 540.dp, small, large)
        assertEquals(3 to 3, six.columns to six.rows)
        assertTrue(fits(six, 540.dp, small) && !six.large)
        // 7 inch (674 × 896 dp): still three across, the covers larger.
        val seven = paperGrid(618.dp, 664.dp, small, large)
        assertEquals(3 to 3, seven.columns to seven.rows)
        assertTrue(seven.cover > six.cover && fits(seven, 664.dp, small))
        // 10.3 inch (992 × 1323 dp, the Note Air 4C): four across, each cover filling most of
        // its 210 dp column, at the body size; three rows fill the page.
        val ten = paperGrid(912.dp, 1090.dp, small, large)
        assertEquals(4 to 3, ten.columns to ten.rows)
        assertTrue(ten.large && ten.cover >= 180.dp && fits(ten, 1090.dp, large))
        // On its side: five by two, as the e-ink board says.
        val side = paperGrid(1243.dp, 760.dp, small, large, upright = false)
        assertEquals(5 to 2, side.columns to side.rows)
        assertTrue(fits(side, 760.dp, large))
        // The e-ink phone (439 × 879 dp): three across.
        val phone = paperGrid(383.dp, 647.dp, small, large)
        assertEquals(3, phone.columns)
        assertTrue(fits(phone, 647.dp, small))
        // No cover is wider than its column.
        for (grid in listOf(six to 516.dp, seven to 618.dp, ten to 912.dp, side to 1243.dp, phone to 383.dp)) {
            assertTrue(grid.first.cover * grid.first.columns + 24.dp * (grid.first.columns - 1) <= grid.second)
        }
    }

    @Test
    fun large_text_takes_fewer_rows_rather_than_covers_under_56_dp() {
        val grid = paperGrid(516.dp, 540.dp, 120.dp, 130.dp)
        assertTrue(grid.cover >= 56.dp && grid.rows < 3)
        assertTrue(fits(grid, 540.dp, 120.dp))
    }

    private companion object {
        const val WORK = "local:lantern-ferry"
    }
}
