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
import org.junit.Assert.assertFalse
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
    fun a_new_layout_before_the_start_was_shown_still_opens_at_the_start() {
        // The phone turned while the reader opened: a second layout comes before the jump to
        // the start place was shown.
        opened(start = 4u)
        assertEquals(4, session.state.value.jump?.leaf)
        val two = ReaderFixtures.leaves(6).filter { !it.endPage }.chunked(2).mapIndexed { i, pair ->
            app.kaeri.engine.model.Leaf(i.toUInt(), pair.flatMap { it.parts }, false)
        }
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, two))
        val jump = session.state.value.jump!!
        assertEquals(2, jump.leaf)
        // Once that jump was shown, a layout keeps the page shown, not the start.
        session.jumped(jump.id)
        session.shown(0, TurnHow.TURN)
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(6)))
        assertEquals(0, session.state.value.jump?.leaf)
    }

    @Test
    fun a_layout_that_comes_before_any_page_is_painted_keeps_the_place_it_opened_at() {
        // A long strip opens at image 4 on whole images, then is cut into two leaves an image
        // once the sizes are known, before image 4 was painted: image 4 starts at leaf 8 now.
        opened(start = 4u)
        val first = session.state.value.jump!!
        session.jumped(first.id)
        val cuts = ReaderFixtures.leaves(6).filter { !it.endPage }.flatMap { listOf(it, it) }.mapIndexed { i, leaf ->
            app.kaeri.engine.model.Leaf(i.toUInt(), leaf.parts, false)
        }
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, cuts))
        val jump = session.state.value.jump!!
        assertEquals(8, jump.leaf)
        assertTrue(jump.id != first.id)
    }

    /** Open a long strip on e-ink at [place], cut four screens an image. */
    private fun openedCut(place: StripPos) {
        session.open(WORK, ReaderFixtures.surface)
        val start = ReadPlace(WORK, CHAPTER, place.image, place, null)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(mode = ReaderMode.STRIP).copy(start = start)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.cuts(images = 4, each = 4)))
    }

    @Test
    fun an_e_ink_strip_opens_on_the_cut_its_place_is_in() {
        // Half way down image 2: the third cut of that image, not cut 2.
        openedCut(StripPos(2u, 0.5f))
        assertEquals(10, session.state.value.jump?.leaf)
        // Between two cuts, the one above.
        openedCut(StripPos(2u, 0.6f))
        assertEquals(10, session.state.value.jump?.leaf)
    }

    @Test
    fun resuming_an_e_ink_strip_three_times_keeps_the_place() {
        var place = StripPos(2u, 0.5f)
        repeat(3) { round ->
            openedCut(place)
            val jump = session.state.value.jump!!
            session.jumped(jump.id)
            sent.clear()
            session.shown(jump.leaf, TurnHow.PLACED)
            // The engine keeps where the cut shown starts, and the next open starts there.
            val shown = sent.filterIsInstance<ReaderCommand.Shown>().single()
            place = session.state.value.leaves[shown.leaf.toInt()].parts.single().strip!!
            assertEquals("round ${round + 1}", StripPos(2u, 0.5f), place)
        }
    }

    @Test
    fun a_strip_whose_pages_were_coming_is_cut_at_the_place_it_showed() {
        // Resumed half way down image 2 while the source's pages still stream: the engine lays
        // the chapter out as whole images first, and the leaf of image 2 is reported shown before
        // its picture came. Once every page is there, the strip is cut: image 2's half way mark
        // starts cut 10, not cut 2.
        session.open(WORK, ReaderFixtures.surface)
        val start = ReadPlace(WORK, CHAPTER, 2u, StripPos(2u, 0.5f), null)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(mode = ReaderMode.STRIP).copy(start = start)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(4)))
        val whole = session.state.value.jump!!
        assertEquals(2, whole.leaf)
        session.jumped(whole.id)
        session.shown(2, TurnHow.PLACED)
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.cuts(images = 4, each = 4)))
        assertEquals(10, session.state.value.jump?.leaf)
    }

    @Test
    fun a_whole_image_turned_to_before_the_cuts_come_is_found_by_its_image() {
        // Turned to image 3 on the whole images: no spot in it is known, so its first cut.
        session.open(WORK, ReaderFixtures.surface)
        val start = ReadPlace(WORK, CHAPTER, 2u, StripPos(2u, 0.5f), null)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(mode = ReaderMode.STRIP).copy(start = start)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(4)))
        session.jumped(session.state.value.jump!!.id)
        session.shown(2, TurnHow.PLACED)
        session.shown(3, TurnHow.TURN)
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.cuts(images = 4, each = 4)))
        assertEquals(12, session.state.value.jump?.leaf)
    }

    @Test
    fun a_new_e_ink_layout_keeps_the_cut_shown_by_where_it_starts() {
        openedCut(StripPos(0u, 0f))
        session.jumped(session.state.value.jump!!.id)
        session.shown(10, TurnHow.TURN)
        // A turn of the device cuts the strip anew, two screens an image: image 2's half way
        // mark starts cut 5.
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.cuts(images = 4, each = 2)))
        assertEquals(5, session.state.value.jump?.leaf)
    }

    @Test
    fun a_second_layout_of_the_open_chapter_keeps_its_place_and_its_pictures() {
        // The page it opens at lays out first; the rest stream in and lay the chapter out again.
        opened(start = 0u)
        session.jumped(session.state.value.jump!!.id)
        val first = PageFile("/cache/0.jpg", app.kaeri.engine.model.PageEncoding.Original("image/jpeg"), 800u, 1200u, null, false, 1u)
        val second = first.copy(path = "/cache/1.jpg")
        session.apply(ReaderEvent.PageReady(SESSION, CHAPTER, 0u, null, first))
        session.apply(ReaderEvent.PageReady(SESSION, CHAPTER, 1u, null, second))
        session.shown(1, TurnHow.TURN)
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(8)))
        val state = session.state.value
        assertEquals(8, state.pages)
        assertEquals(1, state.jump?.leaf)
        // Drawn from before until the page lands for this layout, which a strip still asks for.
        val part = ReaderFixtures.part(1u)
        assertEquals(second, state.file(part))
        assertNull(state.landed(part))
        val fresh = second.copy(path = "/cache/1b.jpg")
        session.apply(ReaderEvent.PageReady(SESSION, CHAPTER, 1u, null, fresh))
        assertEquals(fresh, session.state.value.file(part))
        // A new chapter starts with nothing held.
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u)))
        assertNull(session.state.value.file(part))
    }

    @Test
    fun the_opening_wait_counts_from_the_ask_through_the_chapter_opening() {
        // A novel's text lands after Opened: its plate goes on counting from the ask.
        val before = openingClock()
        session.open(WORK, ReaderFixtures.surface)
        val since = session.state.value.openingSince
        assertTrue(since >= before)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u)))
        assertEquals(since, session.state.value.openingSince)
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
    fun a_surface_that_changed_while_opening_goes_out_once_the_session_opened() {
        // A phone held sideways turns while the reader opens: the engine opened for the old shape.
        session.open(WORK, ReaderFixtures.surface)
        val wide = ReaderFixtures.surface.copy(widthPx = 2400u, heightPx = 1080u, twoPages = true)
        session.surface(wide)
        sent.clear()
        session.apply(ReaderEvent.Opened(ReaderFixtures.view()))
        assertEquals(ReaderCommand.Surface(SESSION, wide), sent.single())
        // The engine has it now: the same size again sends nothing, nor does the next chapter's open.
        sent.clear()
        session.surface(wide)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view()))
        assertTrue(sent.isEmpty())
    }

    @Test
    fun a_surface_back_to_the_opened_one_sends_nothing_once_the_session_opened() {
        session.open(WORK, ReaderFixtures.surface)
        session.surface(ReaderFixtures.surface.copy(widthPx = 2400u, heightPx = 1080u, twoPages = true))
        session.surface(ReaderFixtures.surface)
        sent.clear()
        session.apply(ReaderEvent.Opened(ReaderFixtures.view()))
        assertTrue(sent.isEmpty())
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
    fun an_eink_notice_clears_at_the_next_turn_but_not_at_a_jump_or_the_apps_own_move() {
        opened()
        val notice = ReaderNotice(Msg.Reading(ReadingMsg.ReaderResumed(5)), NoticeAction.START_OVER, 0u)
        session.apply(ReaderEvent.Notice(notice))
        session.shown(4, TurnHow.PLACED)
        session.shown(2, TurnHow.JUMP)
        assertEquals(notice, session.state.value.notice?.notice)
        session.shown(3, TurnHow.TURN)
        assertNull(session.state.value.notice)
    }

    @Test
    fun back_to_page_is_state_the_engine_offers_and_takes_away() {
        opened()
        session.apply(ReaderEvent.BackToPage(SESSION, 2u))
        assertEquals(2u, session.state.value.backTo)
        sent.clear()
        session.backTo()
        assertEquals(ReaderCommand.BackTo(SESSION), sent.single())
        assertNull(session.state.value.backTo)
        session.apply(ReaderEvent.BackToPage(SESSION, 3u))
        session.apply(ReaderEvent.BackToPage(SESSION, null))
        assertNull(session.state.value.backTo)
        // Another session's offer is left, and a new chapter starts without one.
        session.apply(ReaderEvent.BackToPage(SESSION + 1u, 3u))
        assertNull(session.state.value.backTo)
        session.apply(ReaderEvent.BackToPage(SESSION, 3u))
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u)))
        assertNull(session.state.value.backTo)
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
    fun the_end_page_names_the_chapter_past_the_extras_and_the_skip_opens_it() {
        opened()
        val extra = ReaderFixtures.chapter("local:lantern-stair/12.5", "12.5", title = "Volume 1 extras")
        val whole = ReaderFixtures.chapter("local:lantern-stair/13", "13")
        session.apply(ReaderEvent.EndPage(SESSION, extra, null, null, whole, 1u))
        assertEquals(EndPageView(extra, null, null, whole, 1u), session.state.value.end)
        sent.clear()
        session.openChapter(whole.id)
        assertEquals(listOf<ReaderCommand>(ReaderCommand.OpenChapter(SESSION, whole.id)), sent)
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
    fun a_chapter_asked_for_is_on_its_way_until_it_opens_and_taps_meanwhile_ask_nothing() {
        opened()
        sent.clear()
        val next = "local:lantern-stair/13"
        assertTrue(session.openChapter(next))
        assertEquals(next, session.state.value.asking?.chapter)
        assertTrue(session.state.value.openingSince > 0)
        // A second tap, a forward turn on the end page or another chapter: nothing more is asked.
        assertFalse(session.openChapter(next))
        assertTrue(session.crossOn())
        assertFalse(session.openChapter("local:lantern-stair/14"))
        assertEquals(listOf<ReaderCommand>(ReaderCommand.OpenChapter(SESSION, next)), sent)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view().copy(chapter = ReaderFixtures.chapter(next, "13"))))
        assertNull(session.state.value.asking)
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
        session.apply(ReaderEvent.Opening(ReaderFixtures.chapter(), ReaderWait.ChapterPages("1", "Paper Lantern")))
        assertEquals(ReaderWait.ChapterPages("1", "Paper Lantern"), session.state.value.openingWait)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view()))
        session.apply(ReaderEvent.Opening(null, null))
        assertNull(session.state.value.openingWait)
    }

    @Test
    fun the_place_shown_is_kept_to_open_there_again() {
        opened(start = 4u)
        assertEquals(ReadPlace(WORK, CHAPTER, 4u, null, null), session.place())
        session.jumped(session.state.value.jump!!.id)
        session.shown(5, TurnHow.TURN)
        assertEquals(ReadPlace(WORK, CHAPTER, 5u, null, null), session.place())
        session.stripAt(StripPos(2u, 0.5f), false)
        assertEquals(ReadPlace(WORK, CHAPTER, 2u, StripPos(2u, 0.5f), null), session.place())
    }

    @Test
    fun the_kept_place_follows_the_chapter_crossed_into() {
        opened(start = 4u)
        session.openChapter("local:lantern-stair/13")
        val next = ReaderFixtures.view(start = 0u).let {
            it.copy(chapter = ReaderFixtures.chapter("local:lantern-stair/13", "13"), start = ReadPlace(WORK, "local:lantern-stair/13", 0u, null, null))
        }
        session.apply(ReaderEvent.Opened(next))
        assertEquals("local:lantern-stair/13", session.place()?.chapter)
        assertEquals(0u, session.place()?.page)
    }

    @Test
    fun a_forward_turn_on_the_end_page_opens_the_next_chapter_once() {
        opened()
        sent.clear()
        assertTrue(session.crossOn())
        assertTrue(session.crossOn())
        assertEquals(listOf<ReaderCommand>(ReaderCommand.OpenChapter(SESSION, "local:lantern-stair/13")), sent)
    }

    @Test
    fun with_no_next_chapter_a_forward_turn_on_the_end_page_stays() {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(next = null)))
        sent.clear()
        assertEquals(false, session.crossOn())
        assertTrue(sent.isEmpty())
    }

    /** A screen made anew around the session starts its pager at the first leaf: it jumps back. */
    @Test
    fun a_screen_made_anew_jumps_back_to_the_place_shown() {
        opened(start = 0u)
        session.jumped(session.state.value.jump!!.id)
        session.shown(3, TurnHow.TURN)
        val before = session.place()
        session.shown(0, TurnHow.PLACED)
        session.reshow(before)
        assertEquals(3, session.state.value.jump?.leaf)

        session.reshow(null)
        assertEquals(3, session.state.value.jump?.leaf)
    }
}
