package app.kaeri.store

import app.kaeri.engine.model.TrailerControl
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * One visit's banner trailer, as the trailers board has it: the art goes only at the first frame,
 * comes back before any pause or end, and the view goes for good after the end, an error, eight
 * seconds with nothing, or thirty seconds scrolled away.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class TrailerVisitTest {
    private fun TestScope.visit(sent: MutableList<TrailerControl> = mutableListOf()) = TrailerVisit(backgroundScope, { sent += it })

    @Test
    fun `nothing happens until begin`() = runTest {
        val v = visit()
        v.leave()
        v.scrolledAway(true)
        assertFalse(v.live.value)
        // Leaving before it began does not spend the visit.
        v.scrolledAway(false)
        v.begin()
        assertTrue(v.live.value)
    }

    @Test
    fun `the art goes only at the first frame`() = runTest {
        val v = visit()
        v.begin()
        assertTrue(v.live.value)
        assertFalse(v.shown.value)
        v.signal(TrailerSignal.Quiet)
        assertFalse(v.shown.value)
        v.signal(TrailerSignal.Playing)
        assertTrue(v.shown.value)
    }

    @Test
    fun `nothing playing in eight seconds releases silently`() = runTest {
        val v = visit()
        v.begin()
        advanceTimeBy(7_999)
        assertTrue(v.live.value)
        advanceTimeBy(2)
        assertFalse(v.live.value)
        assertFalse(v.shown.value)
    }

    @Test
    fun `an error releases`() = runTest {
        val v = visit()
        v.begin()
        v.signal(TrailerSignal.Failed(150))
        assertFalse(v.live.value)
    }

    @Test
    fun `the art returns half a second before the end, then the view goes and it does not loop`() = runTest {
        val sent = mutableListOf<TrailerControl>()
        val v = visit(sent)
        v.begin()
        v.signal(TrailerSignal.Playing)
        // The page says ending a second before the end.
        v.signal(TrailerSignal.Ending)
        advanceTimeBy(499)
        assertTrue(v.shown.value)
        advanceTimeBy(2)
        assertFalse(v.shown.value)
        assertTrue("the view stays while the art comes back", v.live.value)
        advanceTimeBy(300)
        assertFalse(v.live.value)
        v.signal(TrailerSignal.Ended)
        v.scrolledAway(true)
        v.scrolledAway(false)
        v.begin()
        assertFalse(v.live.value)
        assertTrue(sent.isEmpty())
    }

    @Test
    fun `an end with no warning brings the art back at once`() = runTest {
        val v = visit()
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.signal(TrailerSignal.Ended)
        assertFalse(v.shown.value)
        assertTrue(v.live.value)
        advanceTimeBy(301)
        assertFalse(v.live.value)
    }

    @Test
    fun `scrolling away brings the art back first and pauses once it covers the video`() = runTest {
        val sent = mutableListOf<TrailerControl>()
        val v = visit(sent)
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.scrolledAway(true)
        assertFalse(v.shown.value)
        assertTrue(sent.isEmpty())
        advanceTimeBy(299)
        assertTrue(sent.isEmpty())
        advanceTimeBy(2)
        assertEquals(listOf(TrailerControl.PAUSE), sent)
        assertTrue(v.live.value)
    }

    @Test
    fun `back at the top it plays, then the art goes at the next frame`() = runTest {
        val sent = mutableListOf<TrailerControl>()
        val v = visit(sent)
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.scrolledAway(true)
        advanceTimeBy(5_000)
        v.scrolledAway(false)
        assertEquals(listOf(TrailerControl.PAUSE, TrailerControl.PLAY), sent)
        assertFalse(v.shown.value)
        v.signal(TrailerSignal.Playing)
        assertTrue(v.shown.value)
        // Back in time: the away clock stopped, so it plays on.
        advanceTimeBy(40_000)
        assertTrue(v.live.value)
    }

    @Test
    fun `back before the pause lands it simply comes forward`() = runTest {
        val sent = mutableListOf<TrailerControl>()
        val v = visit(sent)
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.scrolledAway(true)
        advanceTimeBy(100)
        v.scrolledAway(false)
        advanceTimeBy(1_000)
        assertTrue(sent.isEmpty())
        assertTrue(v.shown.value)
    }

    @Test
    fun `thirty seconds away releases for the rest of the visit`() = runTest {
        val sent = mutableListOf<TrailerControl>()
        val v = visit(sent)
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.scrolledAway(true)
        advanceTimeBy(29_999)
        assertTrue(v.live.value)
        advanceTimeBy(2)
        assertFalse(v.live.value)
        v.scrolledAway(false)
        v.begin()
        assertFalse(v.live.value)
        assertEquals(listOf(TrailerControl.PAUSE), sent)
    }

    @Test
    fun `scrolled away while it loads it stays under the art and pauses`() = runTest {
        val sent = mutableListOf<TrailerControl>()
        val v = visit(sent)
        v.begin()
        v.scrolledAway(true)
        v.signal(TrailerSignal.Playing)
        assertFalse(v.shown.value)
        advanceTimeBy(301)
        assertEquals(listOf(TrailerControl.PAUSE), sent)
        v.scrolledAway(false)
        v.signal(TrailerSignal.Playing)
        assertTrue(v.shown.value)
    }

    @Test
    fun `leaving the page releases at once`() = runTest {
        val v = visit()
        v.begin()
        v.signal(TrailerSignal.Playing)
        v.leave()
        runCurrent()
        assertFalse(v.live.value)
        assertFalse(v.shown.value)
        v.begin()
        assertFalse(v.live.value)
    }
}
