package app.kaeri.store

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.SurpriseOutcome
import app.kaeri.engine.model.SurprisePool
import app.kaeri.engine.model.TrendPeriod
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Trending's periods and Surprise me: what the store asks for and keeps. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class BrowsingExtrasTest {
    private fun store() = AppStore(CoroutineScope(Dispatchers.Unconfined))

    @Test
    fun choosing_a_period_asks_for_it_and_keeps_it_over_the_engines_remembered_one() {
        val store = store()
        store.chooseTrending(TrendPeriod.WEEK)
        assertEquals(listOf<Command>(Command.LoadTrending(TrendPeriod.WEEK)), store.waiting)
        assertEquals(TrendPeriod.WEEK, store.search.status.value.period)
        store.apply(Event.Trending(TrendPeriod.MONTH, emptyList(), null, false, null))
        assertEquals("a chosen period is not moved by another's answer", TrendPeriod.WEEK, store.search.status.value.period)
        assertTrue(store.search.status.value.trending.containsKey(TrendPeriod.MONTH))
    }

    @Test
    fun the_remembered_period_shows_until_one_is_chosen() {
        val store = store()
        store.apply(Event.Trending(TrendPeriod.MONTH, emptyList(), null, false, null))
        assertEquals(TrendPeriod.MONTH, store.search.status.value.period)
    }

    @Test
    fun a_pick_is_asked_once_and_opens_its_title_with_another_for_its_pool() {
        val store = store()
        store.surprise(SurprisePool.Planning)
        store.surprise(SurprisePool.Planning)
        assertEquals("a second ask waits for the first", listOf<Command>(Command.Surprise(SurprisePool.Planning)), store.waiting)
        assertEquals(SurprisePool.Planning, store.surprise.asking.value)

        store.apply(Event.Surprise(SurprisePool.Planning, SurpriseOutcome.Picked(900001u, true)))
        assertNull(store.surprise.asking.value)
        assertEquals(900001u, store.surprise.visit.value?.id)
        assertTrue(store.surprise.visit.value?.more == true)

        store.surprise.left(900002u)
        assertEquals("another title closing keeps Another", 900001u, store.surprise.visit.value?.id)
        store.surprise.left(900001u)
        assertNull(store.surprise.visit.value)
    }

    @Test
    fun a_pool_with_nothing_to_pick_opens_nothing() {
        val store = store()
        store.surprise(SurprisePool.Planning)
        store.apply(Event.Surprise(SurprisePool.Planning, SurpriseOutcome.Empty))
        assertNull(store.surprise.asking.value)
        assertFalse(store.surprise.visit.value != null)
    }
}
