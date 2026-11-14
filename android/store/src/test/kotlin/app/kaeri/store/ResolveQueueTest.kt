package app.kaeri.store

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Below Android 14 a second resolve while one runs fails: the queue runs them one at a time. */
class ResolveQueueTest {
    private val answered = mutableListOf<Pair<String, String?>>()
    private val queue = ResolveQueue<String, String> { item, result, _ -> answered += item to result }
    private val started = mutableListOf<String>()
    private val pending = mutableMapOf<String, (String?, Int) -> Unit>()

    private fun add(item: String) = queue.add(item) { it, done ->
        started += it
        pending[it] = done
    }

    @Test
    fun the_next_starts_only_once_the_last_answered() {
        add("a")
        add("b")
        assertEquals(listOf("a"), started)
        pending.getValue("a")("a.local", 0)
        assertEquals(listOf("a", "b"), started)
        pending.getValue("b")(null, 3)
        assertEquals(listOf("a" to "a.local", "b" to null), answered)
        assertTrue(queue.idle)
    }

    @Test
    fun one_taken_off_while_it_waits_never_starts() {
        add("a")
        add("b")
        add("c")
        queue.remove("b")
        pending.getValue("a")("a.local", 0)
        assertEquals(listOf("a", "c"), started)
    }

    @Test
    fun one_taken_off_while_it_runs_answers_no_one_and_the_next_goes_on() {
        add("a")
        add("b")
        queue.remove("a")
        pending.getValue("a")("a.local", 0)
        assertEquals(emptyList<Pair<String, String?>>(), answered)
        assertEquals(listOf("a", "b"), started)
    }

    @Test
    fun a_second_answer_for_the_same_resolve_is_ignored() {
        add("a")
        add("b")
        pending.getValue("a")("a.local", 0)
        pending.getValue("a")("again", 0)
        assertEquals(listOf("a" to "a.local"), answered)
        assertEquals(listOf("a", "b"), started)
    }

    // One found service below Android 14, resolved again when it is found again or a connection
    // to it failed.

    private fun resolving(item: String) = Resolving(queue, item) { it, done ->
        started += it
        pending[it] = done
    }

    @Test
    fun a_service_is_resolved_again_once_the_last_resolve_answered() {
        val tv = resolving("tv")
        tv.resolve()
        pending.getValue("tv")("10.0.0.5", 0)
        tv.resolve()
        pending.getValue("tv")("10.0.0.9", 0)
        assertEquals(listOf("tv", "tv"), started)
        assertEquals(listOf("tv" to "10.0.0.5", "tv" to "10.0.0.9"), answered)
    }

    @Test
    fun asking_again_while_a_resolve_waits_or_runs_adds_none() {
        add("other")
        val tv = resolving("tv")
        tv.resolve()
        tv.resolve()
        pending.getValue("other")("other.local", 0)
        tv.resolve()
        assertEquals(listOf("other", "tv"), started)
        pending.getValue("tv")("10.0.0.5", 0)
        assertTrue(queue.idle)
    }

    @Test
    fun a_service_no_longer_followed_is_not_resolved_again() {
        val tv = resolving("tv")
        tv.resolve()
        tv.stop()
        pending.getValue("tv")("10.0.0.5", 0)
        tv.resolve()
        assertEquals(listOf("tv"), started)
        assertEquals(emptyList<Pair<String, String?>>(), answered)
        assertTrue(queue.idle)
    }
}
