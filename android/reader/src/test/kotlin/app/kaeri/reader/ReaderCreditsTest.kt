// Credit pages and chapter parts in the session: what page 1 and the end page name, where a turn
// goes across pages brought back, how a kept page counts while its Undo waits, and when the
// engine hears of it.

package app.kaeri.reader

import app.kaeri.engine.model.ChapterPart
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.CreditPage
import app.kaeri.engine.model.PageEncoding
import app.kaeri.engine.model.PageFile
import app.kaeri.engine.model.PixelFormat
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.TurnHow
import app.kaeri.reader.ReaderFixtures.CHAPTER
import app.kaeri.reader.ReaderFixtures.SESSION
import app.kaeri.reader.ReaderFixtures.WORK
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ReaderCreditsTest {
    private val sent = mutableListOf<ReaderCommand>()
    private val session = ReaderSession { command -> sent += (command as Command.Reader).v1 }
    private val state get() = session.state.value

    private fun file(name: String) = PageFile(
        path = "/nowhere/$name",
        encoding = PageEncoding.Raw(PixelFormat.GRAY8, 4u),
        width = 4u,
        height = 6u,
        crop = null,
        colour = false,
        bytes = 24uL,
    )

    /** The source's page 0 before the story and page 7 after it; the story is 6 pages. */
    private val start = CreditPage(page = 0u, atEnd = false, file = file("start"), fingerprint = 11uL)
    private val end = CreditPage(page = 7u, atEnd = true, file = file("end"), fingerprint = 22uL)

    private fun opened() {
        session.open(WORK, ReaderFixtures.surface)
        session.apply(ReaderEvent.Opened(ReaderFixtures.view(start = 0u)))
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(6)))
        session.apply(ReaderEvent.CreditPages(SESSION, CHAPTER, listOf(start, end), Msg.Reading(ReadingMsg.ReaderCreditPagesSkipped(2))))
        sent.clear()
    }

    @Test
    fun page_one_and_the_end_page_name_the_pages_skipped_at_their_end() {
        opened()
        assertEquals(listOf(start), state.startSkipped)
        assertEquals(listOf(end), state.endSkipped(CHAPTER))
        assertEquals(0, state.anchor(start, CHAPTER))
        assertEquals(6, state.anchor(end, CHAPTER))
        // The page after the story sits before the end page.
        assertEquals(state.leaves.indexOfFirst { it.endPage }, state.creditLeaf(end))
    }

    @Test
    fun show_brings_the_start_pages_back_and_turns_step_across_them() {
        opened()
        val step = session.showStartCredits()
        assertEquals(CreditStep.Credit(0u, 0), step)
        assertTrue(state.startSkipped.isEmpty())
        // Back from page 1 shows the credit page over it; back from there goes nowhere.
        assertEquals(CreditStep.Credit(0u, 0), state.creditStep(forward = false, leaf = 0))
        session.showCredit(0u)
        assertNull(state.creditStep(forward = false, leaf = 0))
        assertEquals(CreditStep.Leaf(0), state.creditStep(forward = true, leaf = 0))
        assertEquals(CreditNotice.SHOWN, state.creditNotice)
        assertEquals(1 to 1, state.creditPlace)
    }

    @Test
    fun a_shown_page_has_no_number_and_the_total_stays() {
        opened()
        session.showStartCredits()
        session.showCredit(0u)
        assertTrue(state.onUnkeptCredit)
        assertEquals(6, state.shownPages)
    }

    @Test
    fun a_kept_page_counts_at_once_and_its_undo_outlasts_a_turn() {
        opened()
        session.showStartCredits()
        session.showCredit(0u)
        session.shown(0, TurnHow.TURN)
        session.keepCredit()
        assertEquals(CreditNotice.KEPT, state.creditNotice)
        assertEquals(7, state.shownPages)
        assertEquals(0, state.shownPage(0))
        // On to the story's first page, which is page 2 of 7 now.
        session.showCredit(null)
        session.shown(0, TurnHow.TURN)
        session.shown(1, TurnHow.TURN)
        assertEquals(1, state.shownPage(0))
        assertEquals(CreditNotice.KEPT, state.creditNotice)
        assertTrue("the engine has not heard: $sent", sent.none { it is ReaderCommand.NotACreditPage })
        session.undoKeep()
        assertNull(state.keeping)
        assertEquals(6, state.shownPages)
        assertTrue(sent.none { it is ReaderCommand.NotACreditPage })
    }

    @Test
    fun a_kept_page_keeps_its_number_and_no_notice_while_the_engine_lays_it_out() {
        opened()
        session.showStartCredits()
        session.showCredit(0u)
        session.shown(0, TurnHow.TURN)
        session.keepCredit()
        session.commitKeep()
        assertEquals(ReaderCommand.NotACreditPage(SESSION, 11uL), sent.single { it is ReaderCommand.NotACreditPage })
        // Not Skipped as a credit page again between the engine hearing and its answer.
        assertNull(state.creditNotice)
        assertFalse(state.onUnkeptCredit)
        assertEquals(7, state.shownPages)
        assertEquals(0, state.shownPage(0))
        // The answer: the chapter laid out with the page in it.
        session.apply(ReaderEvent.Leaves(SESSION, CHAPTER, ReaderFixtures.leaves(7)))
        assertNull(state.committed)
        assertEquals(7, state.shownPages)
    }

    @Test
    fun the_engine_hears_a_kept_page_when_the_chapter_closes() {
        opened()
        session.showStartCredits()
        session.showCredit(0u)
        session.keepCredit()
        session.close()
        assertEquals(ReaderCommand.NotACreditPage(SESSION, 11uL), sent.single { it is ReaderCommand.NotACreditPage })
    }

    @Test
    fun a_turn_off_the_credit_page_leaves_it_but_not_the_pages_brought_back() {
        opened()
        session.showStartCredits()
        session.showCredit(0u)
        session.shown(0, TurnHow.TURN)
        session.shown(2, TurnHow.JUMP)
        assertNull(state.onCredit)
        assertFalse(state.startSkipped.isNotEmpty())
    }

    @Test
    fun parts_tick_where_each_later_part_starts_and_where_a_missing_one_would() {
        opened()
        session.apply(
            ReaderEvent.ChapterParts(
                SESSION,
                CHAPTER,
                listOf(ChapterPart(number = 1u, firstPage = 1u, group = null, otherGroup = false), ChapterPart(number = 2u, firstPage = 4u, group = null, otherGroup = false)),
                3u,
                listOf(3u),
            ),
        )
        assertEquals(listOf(PartTick(2u, 3, missing = false), PartTick(3u, 6, missing = true)), state.partTicks)
        assertEquals(2u, state.partStarting(3))
        assertNull(state.partStarting(2))
    }

    @Test
    fun a_missing_part_is_named_where_the_part_after_it_starts() {
        opened()
        session.apply(
            ReaderEvent.ChapterParts(
                SESSION,
                CHAPTER,
                listOf(ChapterPart(1u, 1u, null, false), ChapterPart(2u, 3u, null, false), ChapterPart(4u, 5u, null, false)),
                4u,
                listOf(3u),
            ),
        )
        assertEquals(4u, state.partStarting(4))
        assertEquals(3u, state.partMissingAt(4))
        assertNull(state.partMissingAt(2))
    }
}
