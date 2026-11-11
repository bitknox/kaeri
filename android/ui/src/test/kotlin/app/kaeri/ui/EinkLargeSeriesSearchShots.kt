package app.kaeri.ui

import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import app.kaeri.eink.EinkLook
import app.kaeri.engine.model.BookFolderRow
import app.kaeri.engine.model.BookFolderScan
import app.kaeri.engine.model.BookPluginRow
import app.kaeri.engine.model.BookServerKind
import app.kaeri.engine.model.BookServerRow
import app.kaeri.engine.model.BookSourceEvent
import app.kaeri.engine.model.BookSourcesView
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.store.AppStore
import app.kaeri.store.FixtureScene
import app.kaeri.store.LongChaptersStage
import app.kaeri.store.ReadDiscoveryStage
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.launch
import org.junit.Test
import org.robolectric.annotation.Config

/**
 * The large panel board's series page and Search on the 10.3 inch e-reader, at the panel's own
 * font scale of 0.85 and at 1.0, the results on its side too; and the series page on the 6 inch
 * and the 7 inch, which keep the page they had.
 */
class EinkLargeSeriesSearchShots : ReadingShotHarness() {
    private val lists = { store: AppStore -> store.reading.lists.value.isNotEmpty() }

    private fun series(name: String, chapters: Int? = null, after: (() -> Unit)? = null) = shoot(
        name, FixtureScene.READING_SERIES, Destination.Home, medium = KaeriIsFor.READ, page = "read/series/" + android.net.Uri.encode(MANGA), // i18n-skip
        eink = EinkLook(),
        then = { if (chapters != null) LongChaptersStage.stage(it, chapters) },
        after = after,
        ready = { store ->
            val list = store.reading.chapters.value[MANGA]
            store.reading.series.value[MANGA] != null && list != null && (chapters == null || list.volumes.sumOf { it.chapters.size } == chapters)
        },
    )

    /** Search with nothing asked and Your sources only on, as the board draws its start. */
    private fun start(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ, eink = EinkLook(),
        then = { it.reading.runSearch(app.kaeri.store.ReadingModel.EMPTY_QUERY.copy(sourcesOnly = true)) },
        // The fixture has no book sources: three made-up ones, so Search in has places to name.
        act = { compose.runOnUiThread { requireNotNull(store).apply(listOf(Event.BookSources(BookSourceEvent.View(SOURCES)))) } },
        acted = { it.bookSources.view.value?.servers?.isNotEmpty() == true },
        ready = { true },
    )

    private fun results(name: String) {
        val staged = AtomicBoolean(false)
        shoot(
            name, FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ, eink = EinkLook(),
            act = { scope.launch { staged.set(ReadDiscoveryStage.stage(requireNotNull(store), "rd-search-done")) } },
            acted = { staged.get() && it.reading.search.value.sources != null && lists(it) },
            ready = { true },
        )
    }

    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun series_085() = series("series-085")
    @Test @Config(qualifiers = EREADER_10) fun series_100() = series("series-100")

    // A long series, to see a page of chapters full: 41 of them at 0.85 and 1.0.
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun series_long_085() = series("series-long-085", chapters = 41)
    @Test @Config(qualifiers = EREADER_10) fun series_long_100() = series("series-long-100", chapters = 41)

    /** The top line says where Back goes in words, the tab under the page, and the menu stays. */
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun series_back() = series("series-back-085") {
        compose.onNodeWithText("‹ Home").assertExists() // i18n-skip
        compose.onNodeWithContentDescription("More").assertExists() // i18n-skip
    }
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun start_085() = start("search-start-085")
    @Test @Config(qualifiers = EREADER_10) fun start_100() = start("search-start-100")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun results_085() = results("search-results-085")
    @Test @Config(qualifiers = EREADER_10) fun results_100() = results("search-results-100")
    @Test @Config(qualifiers = EREADER_10_LAND, fontScale = AIR_SCALE) fun results_land_085() = results("search-results-land-085")

    // The smaller panels keep today's series page.
    @Test @Config(qualifiers = EREADER) fun series_six() = series("series-six")
    @Test @Config(qualifiers = EREADER_7) fun series_seven() = series("series-seven")

    private companion object {
        const val MANGA = "a:900301"
        const val AIR_SCALE = 0.85f
        const val EREADER_10_LAND = "w1323dp-h992dp-land-300dpi"

        val SOURCES = BookSourcesView(
            folders = listOf(BookFolderRow(id = "f1", name = "Comics folder", scan = BookFolderScan.Done)), // i18n-skip
            servers = listOf(BookServerRow(id = "home-library", kind = BookServerKind.OPDS, name = "Home library", address = "books.home.example")), // i18n-skip
            plugins = listOf(BookPluginRow(id = "northwind", name = "Northwind", version = "1.0.0")), // i18n-skip
        )
    }
}
