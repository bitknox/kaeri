// What the app does with the room's chat: the lines it keeps, what the video's column may show,
// the refusals it counts and how long a line is held.

package app.kaeri.store

import app.kaeri.engine.model.Event
import app.kaeri.engine.model.PartyChatBody
import app.kaeri.engine.model.PartyChatLine
import app.kaeri.engine.model.PartyChatRefusal
import app.kaeri.engine.model.PartyChatState
import app.kaeri.engine.model.PartyConversation
import app.kaeri.engine.model.PartyReaction
import app.kaeri.engine.model.Reaction
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class PartyChatTest {
    private val main = UnconfinedTestDispatcher()
    private lateinit var store: AppStore

    @Before
    fun setUp() {
        Dispatchers.setMain(main)
        store = AppStore(scope = TestScope(main), jobs = JobsModel(now = { 0L }), decoder = { _, _ -> null }, clock = { 0L })
    }

    @After
    fun tearDown() = Dispatchers.resetMain()

    private fun said(id: ULong, from: String, text: String, muted: Boolean = false) =
        PartyChatLine(id, from, false, PartyChatBody.Said(text), muted)

    @Test
    fun a_line_is_held_four_seconds_and_a_second_more_per_ten_words_past_twelve() {
        assertEquals(4_000L, PartyModel.holdMillis("Wait for it"))
        assertEquals(4_000L, PartyModel.holdMillis(List(12) { "w" }.joinToString(" ")))
        assertEquals(5_000L, PartyModel.holdMillis(List(13) { "w" }.joinToString(" ")))
        assertEquals(5_000L, PartyModel.holdMillis(List(22) { "w" }.joinToString(" ")))
        assertEquals(6_000L, PartyModel.holdMillis(List(23) { "w" }.joinToString(" ")))
    }

    @Test
    fun the_conversation_the_engine_sends_is_what_the_lists_show() {
        val lines = listOf(said(1u, "Ren", "Here"), said(2u, "Mio", "Hi", muted = true))
        store.apply(listOf(Event.PartyChat(PartyConversation(lines = lines))))
        assertEquals(lines, store.party.conversation.value.lines)
        assertEquals(listOf(lines[0]), PartyModel.said(store.party.conversation.value.lines))
    }

    @Test
    fun the_video_column_carries_on_from_the_newest_line_seen() {
        assertEquals(0uL, store.party.seenId)
        store.party.seenThrough(5uL)
        store.party.seenThrough(3uL)
        assertEquals(5uL, store.party.seenId)
    }

    @Test
    fun a_refusal_counts_so_the_same_one_twice_is_two_events() {
        store.apply(listOf(Event.PartyChatRefused(PartyChatRefusal.Wait(3u))))
        val first = store.party.refusal.value!!
        store.apply(listOf(Event.PartyChatRefused(PartyChatRefusal.Wait(3u))))
        assertTrue(store.party.refusal.value!!.n > first.n)
        assertEquals(first.what, store.party.refusal.value!!.what)
    }

    @Test
    fun a_reaction_is_a_moment_and_the_room_state_is_kept() {
        store.apply(listOf(Event.PartyChatState(PartyChatState(allowed = true, maxLength = 150u))))
        assertEquals(150u, store.party.chat.value!!.maxLength)
        store.apply(listOf(Event.PartyReaction(PartyReaction("Ren", false, Reaction.LAUGH, 61.0))))
        assertEquals(Reaction.LAUGH, store.party.moment.value!!.reaction.reaction)
    }

    @Test
    fun leaving_clears_the_conversation_and_the_moment() {
        store.apply(listOf(Event.PartyChat(PartyConversation(lines = listOf(said(1u, "Ren", "Here"))))))
        store.apply(listOf(Event.PartyChatState(PartyChatState(allowed = true, maxLength = 150u))))
        store.apply(listOf(Event.PartyLeft(null)))
        assertTrue(store.party.conversation.value.lines.isEmpty())
        assertNull(store.party.chat.value)
    }
}
