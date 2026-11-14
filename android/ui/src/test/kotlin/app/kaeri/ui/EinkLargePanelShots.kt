package app.kaeri.ui

import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import app.kaeri.eink.EinkLook
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.InvitePreview
import app.kaeri.engine.model.InvitedStore
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.store.AppStore
import app.kaeri.store.FixtureScene
import app.kaeri.store.ReadDiscoveryStage
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.launch
import org.junit.Test
import org.robolectric.annotation.Config

/**
 * The e-ink screens on a 10.3 inch e-reader at 300 dpi, the Onyx Boox Note Air 4C's 992 × 1323 dp,
 * at the panel's own font scale of 0.85 and at 1.0; a row's grid and the Library also on the
 * 6 inch, the 7 inch and the e-ink phone, so one sizing serves every panel.
 */
class EinkLargePanelShots : ReadingShotHarness() {
    private val homeRows = { store: AppStore -> store.reading.home.value?.rows?.any { it.tiles.isNotEmpty() } == true }
    private val trending = { store: AppStore -> store.reading.home.value?.rows?.any { it.id == ROW && it.tiles.isNotEmpty() } == true }
    private val lists = { store: AppStore -> store.reading.lists.value.isNotEmpty() }

    private fun home(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, eink = EinkLook(), ready = homeRows,
    )

    private fun browse(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, page = "home/read/browse", eink = EinkLook(), ready = homeRows, // i18n-skip
    )

    private fun row(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, page = "home/read/row:$ROW", eink = EinkLook(), ready = trending, // i18n-skip
    )

    private fun library(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Library, medium = KaeriIsFor.READ, eink = EinkLook(), ready = lists,
    )

    private fun search(name: String) {
        val staged = AtomicBoolean(false)
        shoot(
            name, FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ, eink = EinkLook(),
            act = { scope.launch { staged.set(ReadDiscoveryStage.stage(requireNotNull(store), "rd-search-done")) } },
            acted = { staged.get() && it.reading.search.value.sources != null },
            ready = { true },
        )
    }

    /** Search with nothing asked and Your sources only on, as Johan's panel showed it. */
    private fun searchStart(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ, eink = EinkLook(),
        then = {
            it.reading.loadHome()
            it.reading.runSearch(app.kaeri.store.ReadingModel.EMPTY_QUERY.copy(sourcesOnly = true))
        },
        // Ready at once on arrival, then once Home's rows are in after the step above.
        ready = { !it.reading.search.value.query.sourcesOnly || homeRows(it) },
    )

    private fun downloads(name: String) = shoot(
        name, FixtureScene.READING_DOWNLOADS, Destination.Downloads, medium = KaeriIsFor.READ, eink = EinkLook(),
        ready = { it.reading.downloads.value != null },
    )

    private fun more(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.More, medium = KaeriIsFor.READ, eink = EinkLook(),
        ready = { it.bookSources.view.value != null },
    )

    private fun series(name: String) = shoot(
        name, FixtureScene.READING_SERIES, Destination.Home, medium = KaeriIsFor.READ, page = "read/series/" + android.net.Uri.encode(MANGA), // i18n-skip
        eink = EinkLook(),
        ready = { it.reading.series.value[MANGA] != null && it.reading.chapters.value[MANGA] != null },
    )

    private fun filters(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ, eink = EinkLook(), screen = true,
        act = { compose.onNodeWithText("Filters").performClick() }, // i18n-skip
        ready = { true },
    )

    private fun settings(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, page = settingsRoute(SettingsPane.Reading), eink = EinkLook(),
        ready = { true },
    )

    /** The Join a plugin store sheet over Settings, with a preview of a made-up store. */
    private fun join(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, page = settingsRoute(SettingsPane.Sources), eink = EinkLook(), screen = true,
        then = { store ->
            store.stores.openJoin(INVITE)
            val invited = InvitedStore(INVITE, "Hotaru Plugins", "https://plugins.hotaru.example", "plugins.hotaru.example", "9f3c 1a2b 77de 0c41", alreadyJoined = false, owners = listOf("Ren"), plugins = emptyList(), deviceLabel = "Android tablet") // i18n-skip
            store.apply(listOf(Event.StorePreview(InvitePreview.Store(invited))))
        },
        ready = { true },
    )

    // 10.3 inch at the panel's 0.85

    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_home() = home("air-085-home")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_browse() = browse("air-085-browse")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_row() = row("air-085-row")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_library() = library("air-085-library")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_search() = search("air-085-search")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_search_start() = searchStart("air-085-search-start")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_downloads() = downloads("air-085-downloads")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_more() = more("air-085-more")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_series() = series("air-085-series")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_filters() = filters("air-085-filters")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_settings() = settings("air-085-settings")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_join() = join("air-085-join")

    // 10.3 inch at 1.0

    @Test @Config(qualifiers = EREADER_10) fun air_home_1() = home("air-100-home")
    @Test @Config(qualifiers = EREADER_10) fun air_row_1() = row("air-100-row")
    @Test @Config(qualifiers = EREADER_10) fun air_library_1() = library("air-100-library")
    @Test @Config(qualifiers = EREADER_10) fun air_search_1() = search("air-100-search")
    @Test @Config(qualifiers = EREADER_10) fun air_downloads_1() = downloads("air-100-downloads")
    @Test @Config(qualifiers = EREADER_10) fun air_series_1() = series("air-100-series")

    // 10.3 inch in landscape

    @Test @Config(qualifiers = EREADER_10_LAND, fontScale = AIR_SCALE) fun air_land_row() = row("air-land-085-row")
    @Test @Config(qualifiers = EREADER_10_LAND, fontScale = AIR_SCALE) fun air_land_library() = library("air-land-085-library")

    // The same grids on the smaller panels

    @Test @Config(qualifiers = EREADER) fun six_row() = row("six-row")
    @Test @Config(qualifiers = EREADER) fun six_library() = library("six-library")
    @Test @Config(qualifiers = EREADER_7) fun seven_row() = row("seven-row")
    @Test @Config(qualifiers = EREADER_7) fun seven_library() = library("seven-library")
    @Test @Config(qualifiers = EINK_PHONE) fun phone_row() = row("eink-phone-row")
    @Test @Config(qualifiers = EINK_PHONE) fun phone_library() = library("eink-phone-library")

    private companion object {
        const val ROW = "trending-manga"
        const val MANGA = "a:900301"
        const val AIR_SCALE = 0.85f
        const val INVITE = "https://plugins.hotaru.example/join/4kq2v9#9f3c1a2b77de0c41" // i18n-skip
        const val EREADER_10_LAND = "w1323dp-h992dp-land-300dpi"
    }
}
