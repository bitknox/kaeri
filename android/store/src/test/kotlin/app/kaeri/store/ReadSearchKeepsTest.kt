package app.kaeri.store

import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.ReadAnilistState
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.engine.model.ReadSourceGroup
import app.kaeri.engine.model.ReadSourceSearch
import app.kaeri.engine.model.ReadSourceState
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.WorkTile
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Reading's Search with new words: what was found stays on screen until the new answer replaces it. */
class ReadSearchKeepsTest {
    private val model = ReadingModel { _: Command -> }

    private fun query(words: String, sourcesOnly: Boolean = false): ReadSearchQuery =
        ReadingModel.EMPTY_QUERY.copy(text = words, sourcesOnly = sourcesOnly)

    private fun tile(n: Int) = WorkTile(work = "a:$n", title = "Paper Kite $n", medium = Medium.MANGA)

    private fun group(id: String, state: ReadSourceState, tiles: List<WorkTile> = emptyList()) =
        ReadSourceGroup(id, id, null, state, tiles, 0u)

    private fun sources(q: ReadSearchQuery, vararg groups: ReadSourceGroup, done: Boolean = false) =
        ReadingEvent.SourceSearch(ReadSourceSearch(q, ReadAnilistState.ANSWERED, groups.toList(), emptyList(), done))

    /** "kite" answered by AniList and two sources, one with hits. */
    private fun answered(): ReadSearchQuery {
        val first = query("kite")
        model.runSearch(first)
        model.apply(sources(first, group("shelf", ReadSourceState.Hits, listOf(tile(9))), group("folder", ReadSourceState.Nothing), done = true))
        model.apply(ReadingEvent.Search(first, 0u, listOf(tile(1), tile(2)), false, null, 2u))
        return first
    }

    @Test
    fun new_words_keep_the_last_answer_until_their_own_lands() {
        answered()
        val next = query("kite rider")
        model.runSearch(next)

        val asking = model.search.value
        assertEquals(next, asking.query)
        assertTrue(asking.loading)
        assertTrue(asking.stale)
        assertEquals(listOf(tile(1), tile(2)), asking.tiles)
        assertEquals(setOf("shelf", "folder"), asking.staleSources)
        // Kept, but not said as done: the announcement waits for the new answer.
        assertFalse(asking.sources!!.done)
        assertEquals(ReadSourceState.Hits, asking.sources!!.groups.first { it.sourceId == "shelf" }.state)

        // The sources' first word for the new query is that they are asking: the old rows stay.
        model.apply(sources(next, group("shelf", ReadSourceState.Asking), group("folder", ReadSourceState.Asking)))
        assertEquals(listOf(tile(9)), model.search.value.sources!!.groups.first { it.sourceId == "shelf" }.tiles)

        // One source answers: its row takes the new answer, the other still shows the old one.
        model.apply(sources(next, group("shelf", ReadSourceState.Asking), group("folder", ReadSourceState.Hits, listOf(tile(7)))))
        val one = model.search.value
        assertEquals(setOf("shelf"), one.staleSources)
        assertEquals(listOf(tile(7)), one.sources!!.groups.first { it.sourceId == "folder" }.tiles)
        assertEquals(listOf(tile(9)), one.sources!!.groups.first { it.sourceId == "shelf" }.tiles)

        // AniList's page replaces its grid in one go.
        model.apply(ReadingEvent.Search(next, 0u, listOf(tile(3)), false, null, 1u))
        val landed = model.search.value
        assertFalse(landed.stale)
        assertFalse(landed.loading)
        assertEquals(listOf(tile(3)), landed.tiles)
    }

    @Test
    fun a_source_asked_again_by_its_own_try_again_shows_as_asking() {
        val first = answered()
        model.apply(sources(first, group("shelf", ReadSourceState.Asking), group("folder", ReadSourceState.Nothing)))
        assertEquals(ReadSourceState.Asking, model.search.value.sources!!.groups.first { it.sourceId == "shelf" }.state)
    }

    @Test
    fun a_failed_first_page_takes_the_place_of_the_old_grid() {
        answered()
        model.runSearch(query("kite rider"))
        model.apply(ReadingEvent.Failed(ReadingAbout.SEARCH, ReadingFailure.Offline))
        val failed = model.search.value
        assertTrue(failed.tiles.isEmpty())
        assertFalse(failed.stale)
        assertEquals(ReadingFailure.Offline, failed.failure)
    }

    @Test
    fun clearing_the_words_drops_the_sources() {
        answered()
        model.runSearch(query(""))
        assertEquals(null, model.search.value.sources)
        assertTrue(model.search.value.stale)
    }

    @Test
    fun view_all_from_a_row_starts_afresh_and_a_chip_keeps_the_grid() {
        val row = ReadingModel.EMPTY_QUERY.copy(formats = listOf("manga"))
        model.runViewAll(row)
        model.apply(ReadingEvent.Search(row, 0u, listOf(tile(1)), false, null, 1u))
        val chip = row.copy(status = "releasing")
        model.runViewAll(chip, keep = true)
        assertEquals(listOf(tile(1)), model.viewAll.value.tiles)
        assertTrue(model.viewAll.value.stale)
        model.runViewAll(row)
        assertTrue(model.viewAll.value.tiles.isEmpty())
    }

    @Test
    fun view_all_opens_on_the_row_covers_until_its_first_page_lands() {
        val row = ReadingModel.EMPTY_QUERY.copy(formats = listOf("manga"))
        model.runViewAll(row, first = listOf(tile(1), tile(2)))
        assertEquals(listOf(tile(1), tile(2)), model.viewAll.value.tiles)
        assertTrue(model.viewAll.value.stale)
        model.apply(ReadingEvent.Search(row, 0u, listOf(tile(1), tile(2), tile(3)), false, null, 3u))
        assertEquals(listOf(tile(1), tile(2), tile(3)), model.viewAll.value.tiles)
        assertFalse(model.viewAll.value.stale)
    }

    /** A source picked under Search in: AniList is not asked, so the groups end the wait. */
    @Test
    fun a_scoped_search_waits_on_its_sources_alone() {
        answered()
        val scoped = query("kite rider").copy(sourceScope = listOf("shelf"))
        model.runSearch(scoped)
        assertEquals(ReadAnilistState.SKIPPED, model.search.value.sources!!.anilist)
        assertTrue(model.search.value.loading)
        model.apply(sources(scoped, group("shelf", ReadSourceState.Hits, listOf(tile(8))), done = true))
        assertFalse(model.search.value.loading)
    }
}
