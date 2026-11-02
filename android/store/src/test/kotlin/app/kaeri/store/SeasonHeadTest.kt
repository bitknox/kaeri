package app.kaeri.store

import android.graphics.Bitmap
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.LinkedTitle
import app.kaeri.engine.model.TitleDetail

/**
 * A season's head sent ahead of its page (a partial page): kept until the whole page comes, never
 * over it, and never taken for the whole page; and the seasons' art fetched ahead to the disk.
 */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class SeasonHeadTest {
    private fun season(id: UInt) = LinkedTitle(
        id, "Season of $id", null, null, null, null, "TV", coverUrl = null, coverColor = null, season = null,
        year = null, listStatus = null,
    )

    private fun page(id: UInt, partial: Boolean, synopsis: String = "Whole") = TestEntries.entry(id).copy(
        synopsis = synopsis,
        detail = TitleDetail(
            tags = emptyList(), schedule = emptyList(), relations = if (partial) emptyList() else listOf(season(920001u)),
            seasons = listOf(season(920001u), season(920002u)), recommendations = emptyList(),
            rankings = emptyList(), characters = emptyList(), staff = emptyList(), partial = partial,
        ),
    )

    @Test
    fun a_head_is_kept_as_a_head_until_the_whole_page_replaces_it() {
        val titles = TitleCache()
        titles.merge(detail = page(920002u, partial = true, synopsis = "Head"))
        assertTrue(920002u in titles.headed)
        assertFalse(920002u in titles.detailed)
        assertTrue(titles.hasHead(920002u))
        assertEquals("Head", titles[920002u]?.synopsis)

        titles.merge(detail = page(920002u, partial = false))
        assertFalse(920002u in titles.headed)
        assertTrue(920002u in titles.detailed)
        assertEquals("Whole", titles[920002u]?.synopsis)
        assertEquals(false, titles[920002u]?.detail?.partial)
    }

    @Test
    fun a_head_after_the_whole_page_is_ignored() {
        val titles = TitleCache()
        titles.merge(detail = page(920002u, partial = false))
        titles.merge(detail = page(920002u, partial = true, synopsis = "Head"))
        assertFalse(920002u in titles.headed)
        assertEquals("Whole", titles[920002u]?.synopsis)
        assertEquals(1, titles[920002u]?.detail?.relations?.size)
    }

    @Test
    fun a_fetch_that_brought_only_the_head_did_not_answer() {
        val titles = TitleCache()
        titles.detailAsked(920002u)
        titles.merge(detail = page(920002u, partial = true))
        titles.detailEnded(920002u)
        assertTrue("the whole page was still awaited", 920002u in titles.failed.value)
        // Its head stays to show above the failure.
        assertTrue(titles.hasHead(920002u))
    }

    @Test
    fun a_fetch_that_brought_the_whole_page_after_its_head_answered() {
        val titles = TitleCache()
        titles.detailAsked(920002u)
        titles.merge(detail = page(920002u, partial = true))
        titles.merge(detail = page(920002u, partial = false))
        titles.detailEnded(920002u)
        assertFalse(920002u in titles.failed.value)
    }

    @Test
    fun the_seasons_cards_leave_a_heads_seasons_be() {
        val titles = TitleCache()
        titles.merge(detail = page(920002u, partial = true))
        // The seasons' headers said again as cards, as the engine does once a page knows them.
        titles.merge(list = listOf(TestEntries.entry(920002u)))
        assertEquals(listOf(920001u, 920002u), titles[920002u]?.detail?.seasons?.map { it.id })
        assertTrue(920002u in titles.headed)
    }

    @Test
    fun art_fetched_ahead_is_not_decoded_until_a_screen_asks() {
        val dispatcher = UnconfinedTestDispatcher()
        val asked = mutableListOf<String>()
        var decoded = 0
        val art = ArtworkStore(
            TestScope(dispatcher),
            decoder = { _, size ->
                decoded += 1
                Artwork(Bitmap.createBitmap(16, 9, Bitmap.Config.ARGB_8888), size, 16)
            },
            now = { 0L },
            decoding = dispatcher,
            main = dispatcher,
        ) { url -> asked += url; true }
        art.warm(BANNER)
        art.warm(BANNER)
        art.ready(BANNER, ByteArray(0))
        assertEquals("asked once", listOf(BANNER), asked)
        assertEquals("kept on the disk, not in memory", 0, decoded)
        assertEquals(null, art.image(BANNER))

        art.request(BANNER, 1080)
        art.ready(BANNER, ByteArray(0))
        assertEquals(1, decoded)
        assertTrue(art.image(BANNER) != null)
    }

    @Test
    fun art_asked_for_while_it_is_fetched_ahead_is_decoded_when_it_lands() {
        val dispatcher = UnconfinedTestDispatcher()
        val asked = mutableListOf<String>()
        val art = ArtworkStore(
            TestScope(dispatcher),
            decoder = { _, size -> Artwork(Bitmap.createBitmap(16, 9, Bitmap.Config.ARGB_8888), size, 16) },
            now = { 0L },
            decoding = dispatcher,
            main = dispatcher,
        ) { url -> asked += url; true }
        art.warm(BANNER)
        art.request(BANNER, 1080)
        art.ready(BANNER, ByteArray(0))
        assertEquals(listOf(BANNER), asked)
        assertTrue(art.image(BANNER) != null)
    }

    private companion object {
        const val BANNER = "https://img.example/banner/920002.jpg"
    }
}
