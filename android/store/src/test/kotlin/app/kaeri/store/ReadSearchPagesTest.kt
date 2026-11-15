// Reading's search adds AniList's later pages as they come. Two pages can overlap when a work moves
// down a place between the asks, and the grids key by work, so each work is listed once, where it
// first came, with words and when browsing.

package app.kaeri.store

import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.WorkTile
import org.junit.Assert.assertEquals
import org.junit.Test

class ReadSearchPagesTest {
    private val model = ReadingModel { _: Command -> }

    private fun tile(n: Int) = WorkTile(work = "a:$n", title = "Paper Kite $n", medium = Medium.MANGA)

    private fun twoPages(query: ReadSearchQuery, second: List<WorkTile>): List<String> {
        model.runSearch(query)
        model.apply(ReadingEvent.Search(query, 0u, listOf(tile(1), tile(2), tile(3)), true, null, null))
        model.searchMore()
        model.apply(ReadingEvent.Search(query, 1u, second, false, null, null))
        return model.search.value.tiles.map { it.work }
    }

    @Test
    fun with_words_a_work_on_two_pages_is_listed_once_where_it_first_came() {
        val works = twoPages(ReadingModel.EMPTY_QUERY.copy(text = "kite"), listOf(tile(3), tile(4), tile(5)))
        assertEquals(listOf("a:1", "a:2", "a:3", "a:4", "a:5"), works)
        assertEquals(3, model.search.value.firstPage)
    }

    @Test
    fun browsing_a_work_on_two_pages_is_listed_once_where_it_first_came() {
        val works = twoPages(ReadingModel.EMPTY_QUERY.copy(genres = listOf("Drama")), listOf(tile(3), tile(4), tile(4)))
        assertEquals(listOf("a:1", "a:2", "a:3", "a:4"), works)
    }

    @Test
    fun a_first_page_that_names_a_work_twice_lists_it_once() {
        val query = ReadingModel.EMPTY_QUERY.copy(text = "kite")
        model.runSearch(query)
        model.apply(ReadingEvent.Search(query, 0u, listOf(tile(1), tile(2), tile(1)), false, null, null))
        assertEquals(listOf("a:1", "a:2"), model.search.value.tiles.map { it.work })
        assertEquals(2, model.search.value.firstPage)
    }
}
