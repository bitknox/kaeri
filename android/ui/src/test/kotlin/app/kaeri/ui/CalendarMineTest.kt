package app.kaeri.ui

import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.LibrarySegment
import app.kaeri.engine.model.TitleProgress
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Which titles the calendar's My list filter keeps. */
class CalendarMineTest {
    private val started = TitleProgress(done = 3u, next = 4u)

    private fun title(status: LibrarySegment?, progress: TitleProgress?) = Entry(
        id = 1u, title = "Lantern Harbour", secondary = null, format = "TV", episodes = 12u, year = 2026u,
        score = null, studio = null, synopsis = "", coverUrl = null, bannerUrl = null,
        genres = emptyList(), availableOn = emptyList(), progress = progress, airingIn = null,
        airingEpisode = null, nextEpisode = null, nextAiringIn = null, lastAired = null, resume = null,
        related = emptyList(), recommended = emptyList(), trailerUrl = null, listStatus = status,
        titleRomaji = "Lantern Harbour", titleEnglish = null, titleNative = null, season = null, startDate = null,
        endDate = null, source = null, adult = false, detail = null,
        status = null, synopsisSpans = emptyList(),
    )

    @Test
    fun a_dropped_title_is_not_mine_whatever_its_progress() {
        assertFalse(title(LibrarySegment.DROPPED, started).isMine())
        assertFalse(title(LibrarySegment.DROPPED, null).isMine())
    }

    @Test
    fun watching_and_planning_are_mine() {
        assertTrue(title(LibrarySegment.WATCHING, null).isMine())
        assertTrue(title(LibrarySegment.PLANNING, null).isMine())
    }

    @Test
    fun progress_with_no_status_is_mine() {
        assertTrue(title(null, started).isMine())
        assertFalse(title(null, null).isMine())
    }
}
