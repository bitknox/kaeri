package app.kaeri.store

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * A browse's followed services: below Android 14, a service found again and a connection that
 * failed both resolve again, as nothing else says that a service moved.
 */
class NsdFollowsTest {
    private val follows = NsdFollows()
    private val followed = mutableListOf<String>()
    private val relooked = mutableListOf<String>()
    private val stopped = mutableListOf<String>()

    private fun found(name: String) = follows.found(name) {
        followed += name
        NsdFollowing(stop = { stopped += name }, relook = { relooked += name })
    }

    @Test
    fun a_service_found_again_is_resolved_again_not_followed_twice() {
        found("Living room")
        found("Living room")
        assertEquals(listOf("Living room"), followed)
        assertEquals(listOf("Living room"), relooked)
    }

    @Test
    fun a_failed_connection_resolves_every_service_again() {
        found("Living room")
        found("Bedroom")
        follows.relook()
        assertEquals(listOf("Living room", "Bedroom"), relooked)
    }

    @Test
    fun a_service_lost_is_followed_afresh_when_it_comes_back() {
        found("Living room")
        follows.lost("Living room")
        follows.relook()
        found("Living room")
        assertEquals(listOf("Living room"), stopped)
        assertEquals(emptyList<String>(), relooked)
        assertEquals(listOf("Living room", "Living room"), followed)
    }

    @Test
    fun a_stopped_browse_follows_nothing() {
        found("Living room")
        found("Bedroom")
        follows.clear()
        follows.relook()
        assertEquals(listOf("Living room", "Bedroom"), stopped)
        assertEquals(emptyList<String>(), relooked)
    }

    @Test
    fun a_follow_that_throws_is_not_kept() {
        follows.found("Living room") { error("the system refused it") }
        found("Living room")
        assertEquals(listOf("Living room"), followed)
        assertEquals(emptyList<String>(), relooked)
    }
}
