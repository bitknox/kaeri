// The session's half of the reader protocol: what goes out for each report, and the state each
// engine event leaves, without an engine.

package app.kaeri.reader

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.NoNext
import app.kaeri.engine.model.OpenAt
import app.kaeri.engine.model.PageEncoding
import app.kaeri.engine.model.PageFile
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReadPlace
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.model.ReaderAction as NoticeAction
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderNotice
import app.kaeri.engine.model.ReaderWait
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.StripPos
import app.kaeri.engine.model.TurnHow
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import app.kaeri.reader.ReaderFixtures.WORK
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ReaderSessionTest {
    private val sent = mutableListOf<ReaderCommand>()
    private val session = ReaderSession { command -> sent += (command as Command.Reader).v1 }

    private fun opened(start: UInt? = 4u, pages: Int = 6) {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = start)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(pages)))
    }

    @Test
    fun open_sends_the_work_and_surface_and_waits() {
        session.open(WORK, ReaderFixtures.surface, at = OpenAt.Start)
        assertEquals(ReaderCommand.Open(WORK, null, OpenAt.Start, ReaderFixtures.surface), sent.single())
        assertTrue(session.state.value.opening)
    }

    @Test
    fun the_start_place_is_a_jump_once_the_leaves_come() {
        opened(start = 4u)
        val state = session.state.value
        assertEquals(7, state.leaves.size)
        assertEquals(6, state.pages)
        assertEquals(4, state.jump?.leaf)
        session.jumped(state.jump!!.id)
        assertNull(session.state.value.jump)
    }

    @Test
    fun a_new_layout_keeps_the_page_that_was_shown() {
        opened(start = 0u)
        session.jumped(session.state.value.jump!!.id)
        session.shown(3, TurnHow.TURN)
        // Two pages a leaf now: page 3 is on the second leaf.
        val two = ReaderFixtures.leaves(6).filter { !it.endPage }.chunked(2).mapIndexed { i, pair ->
            app.kaeri.engine.model.Leaf(i.toUInt(), pair.flatMap { it.parts }, false)
        }
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, two))
        assertEquals(1, session.state.value.jump?.leaf)
    }

    @Test
    fun a_mode_changed_in_the_reader_turns_the_view_with_it() {
        opened()
        fun row(key: ReaderSettingKey, word: String) =
            ReaderSettingRow(key, ReaderValue.Choice(word), SettingScope.SERIES, false, true, emptyList())
        // The first settings leave the engine's layout as it is, guesses and all.
        val opened = session.state.value.view
        session.apply(ReaderEvent.Settings(ReaderSettingsView(Medium.MANGA, WORK, listOf(row(ReaderSettingKey.MODE, "strip"), row(ReaderSettingKey.DIRECTION, "rtl")))))
        assertEquals(opened, session.state.value.view)
        session.apply(ReaderEvent.Settings(ReaderSettingsView(Medium.MANGA, WORK, listOf(row(ReaderSettingKey.MODE, "paged"), row(ReaderSettingKey.DIRECTION, "ltr")))))
        assertEquals(ReadDirection.LEFT_TO_RIGHT, session.state.value.view?.direction)
        assertEquals(ReaderMode.PAGED, session.state.value.view?.mode)
        session.apply(ReaderEvent.Settings(ReaderSettingsView(Medium.MANGA, WORK, listOf(row(ReaderSettingKey.MODE, "strip"), row(ReaderSettingKey.DIRECTION, "ltr")))))
        assertEquals(ReaderMode.STRIP, session.state.value.view?.mode)
        // Another medium's defaults leave the open chapter alone.
        session.apply(ReaderEvent.Settings(ReaderSettingsView(Medium.NOVEL, null, listOf(row(ReaderSettingKey.MODE, "paged")))))
        assertEquals(ReaderMode.STRIP, session.state.value.view?.mode)
    }

    @Test
    fun spreads_count_pages_not_leaves() {
        opened(start = 0u, pages = 9)
        // The cover alone, then pairs: five leaves hold nine pages.
        val parts = ReaderFixtures.leaves(9).filter { !it.endPage }.flatMap { it.parts }
        val spreads = (listOf(listOf(parts[0])) + parts.drop(1).chunked(2)).mapIndexed { i, pair ->
            app.kaeri.engine.model.Leaf(i.toUInt(), pair, false)
        } + app.kaeri.engine.model.Leaf(5u, emptyList(), true)
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, spreads))
        assertEquals(9, session.state.value.pages)
    }

    @Test
    fun shown_reports_the_leaf_and_how() {
        opened()
        sent.clear()
        session.shown(5, TurnHow.SWIPE)
        assertEquals(ReaderCommand.Shown(SESSION, CHAPTER, 5u, TurnHow.SWIPE), sent.single())
    }

    @Test
    fun a_surface_goes_out_only_when_it_changed() {
        opened()
        sent.clear()
        session.surface(ReaderFixtures.surface)
        assertTrue(sent.isEmpty())
        val wide = ReaderFixtures.surface.copy(widthPx = 1920u, heightPx = 1080u, twoPages = true)
        session.surface(wide)
        assertEquals(ReaderCommand.Surface(SESSION, wide), sent.single())
    }

    @Test
    fun pages_land_and_fail_by_page_and_half() {
        opened()
        val file = ReaderFixtures.part(2u)
        val page = PageFile("/cache/2.jpg", app.kaeri.engine.model.PageEncoding.Original("image/jpeg"), 800u, 1200u, null, false, 1u)
        session.apply(ReaderEvent.PageReady(SESSION, CHAPTER, 2u, null, page))
        assertEquals(page, session.state.value.file(file))
        session.apply(ReaderEvent.PageFailed(SESSION, CHAPTER, 3u, ReadingFailure.PageFetch(404u)))
        assertEquals(ReadingFailure.PageFetch(404u), session.state.value.failures[3u])
        sent.clear()
        session.retry(3u)
        assertEquals(ReaderCommand.Retry(SESSION, CHAPTER, 3u), sent.single())
        assertTrue(session.state.value.failures.isEmpty())
    }

    @Test
    fun a_missing_file_is_asked_for_once_a_layout() {
        opened()
        sent.clear()
        val part = ReaderFixtures.part(2u)
        session.needPage(part)
        session.needPage(part)
        assertEquals(listOf<ReaderCommand>(ReaderCommand.NeedPage(SESSION, CHAPTER, 2u)), sent)
    }

    @Test
    fun another_sessions_events_are_left() {
        opened()
        val before = session.state.value
        session.apply(ReaderEvent.Leaves(SESSION + 1u, CHAPTER, ReaderFixtures.leaves(2)))
        session.apply(ReaderEvent.EndPage(SESSION + 1u, null, NoNext.FINISHED))
        session.apply(ReaderEvent.Closed(SESSION + 1u))
        assertEquals(before, session.state.value)
    }

    @Test
    fun an_eink_notice_clears_at_the_next_turn_but_not_at_a_jump() {
        opened()
        val notice = ReaderNotice(Msg.Reading(ReadingMsg.ReaderBackToPage(5)), NoticeAction.BACK_TO, 0u)
        session.apply(ReaderEvent.Notice(notice))
        session.shown(2, TurnHow.JUMP)
        assertEquals(notice, session.state.value.notice?.notice)
        session.shown(3, TurnHow.TURN)
        assertNull(session.state.value.notice)
    }

    @Test
    fun a_phone_notice_waits_for_its_time() {
        opened()
        session.apply(ReaderEvent.Notice(ReaderNotice(Msg.Reading(ReadingMsg.ReaderResumed(5)), NoticeAction.START_OVER, 6000u)))
        session.shown(3, TurnHow.TURN)
        val shown = session.state.value.notice!!
        session.dismissNotice(shown.id)
        assertNull(session.state.value.notice)
    }

    @Test
    fun go_to_jumps_within_the_chapter_and_end_page_names_what_is_next() {
        opened()
        session.apply(ReaderEvent.GoTo(SESSION, ReadPlace(WORK, CHAPTER, 1u, null, null)))
        assertEquals(1, session.state.value.jump?.leaf)
        val next = ReaderFixtures.chapter("local:lantern-stair/13", "13")
        session.apply(ReaderEvent.EndPage(SESSION, next, null))
        assertEquals(EndPageView(next, null), session.state.value.end)
        session.apply(ReaderEvent.ChapterRead(WORK, CHAPTER))
        assertTrue(CHAPTER in session.state.value.read)
    }

    @Test
    fun a_strip_place_goes_out_with_the_foot() {
        opened()
        sent.clear()
        session.stripAt(StripPos(4u, 0.5f), footVisible = true)
        assertEquals(ReaderCommand.StripAt(SESSION, CHAPTER, StripPos(4u, 0.5f), true), sent.single())
    }

    @Test
    fun a_failed_open_is_kept_for_the_screen() {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(Event.Reading(ReadingEvent.Failed(ReadingAbout.READER, ReadingFailure.Offline)))
        assertEquals(ReadingFailure.Offline, session.state.value.failure)
    }

    @Test
    fun close_and_closed() {
        opened()
        sent.clear()
        session.close()
        assertEquals(ReaderCommand.Close(SESSION), sent.single())
        session.apply(ReaderEvent.Closed(SESSION))
        assertTrue(session.state.value.closed)
    }
    @Test
    fun a_strip_that_flows_on_keeps_the_chapter_it_left_above_the_next() {
        opened(start = 0u)
        session.jumped(session.state.value.jump!!.id)
        val next = "local:lantern-stair/13"
        sent.clear()
        session.flowInto(next)
        session.flowInto(next)
        assertEquals(listOf<ReaderCommand>(ReaderCommand.OpenChapter(SESSION, next)), sent)
        val view = ReaderFixtures.view(start = 0u).copy(chapter = ReaderFixtures.chapter(next, "13"))
        session.apply(ReaderEvent.Opened(view))
        assertEquals(CHAPTER, session.state.value.previous?.chapter)
        assertEquals(6, session.state.value.previous?.pages)
        // Flowed into from above, the scroll is already where the chapter starts: no jump.
        session.apply(ReaderEvent.Leaves(SESSION, next, ReaderFixtures.leaves(4)))
        assertNull(session.state.value.jump)
        assertEquals(4, session.state.value.pages)
    }

    @Test
    fun a_chapter_opened_by_hand_does_not_keep_the_one_before() {
        opened()
        session.openChapter("local:lantern-stair/13")
        val view = ReaderFixtures.view().copy(chapter = ReaderFixtures.chapter("local:lantern-stair/13", "13"))
        session.apply(ReaderEvent.Opened(view))
        assertNull(session.state.value.previous)
    }

    @Test
    fun a_chapter_that_fails_to_open_from_another_is_named_and_its_sources_asked() {
        opened()
        session.openChapter("local:lantern-stair/13")
        session.apply(Event.Reading(ReadingEvent.Failed(ReadingAbout.READER, ReadingFailure.Offline)))
        val state = session.state.value
        assertEquals(ReadingFailure.Offline, state.failure)
        assertEquals("13", state.openingChapter?.number)
        val asked = mutableListOf<Command>()
        val other = ReaderSession { asked += it }
        other.open(WORK, ReaderFixtures.surface, "local:lantern-stair/13")
        other.loadSources()
        assertEquals(Command.Reading(ReadingCommand.LoadVersions(WORK, "local:lantern-stair/13")), asked.last())
    }

    @Test
    fun the_next_chapter_s_first_page_and_source_reach_the_end_page() {
        opened()
        val next = ReaderFixtures.chapter("local:lantern-stair/13", "13")
        session.apply(ReaderEvent.EndPage(SESSION, next, null, "Lantern Archive"))
        assertEquals("Lantern Archive", session.state.value.end?.source)
        val page = PageFile("/cache/n0.jpg", PageEncoding.Original("image/jpeg"), 800u, 1200u, null, false, 1uL)
        session.apply(ReaderEvent.NextPreview(SESSION, next.id, page))
        assertEquals(NextPreview(next.id, page), session.state.value.preview)
        session.apply(ReaderEvent.NextPreview(SESSION + 1u, next.id, page.copy(path = "/other")))
        assertEquals("/cache/n0.jpg", session.state.value.preview?.file?.path)
    }

    @Test
    fun what_an_open_waits_on_shows_only_while_opening() {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opening(ReaderFixtures.chapter(), ReaderWait.FINDING_SOURCE))
        assertEquals(ReaderWait.FINDING_SOURCE, session.state.value.openingWait)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view()))
        session.apply(ReaderEvent.Opening(null, null))
        assertNull(session.state.value.openingWait)
    }
}
