package app.kaeri.ui

import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import app.kaeri.eink.EinkLook
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.store.AppStore
import app.kaeri.store.FixtureScene
import app.kaeri.store.ReadDiscoveryStage
import app.kaeri.store.text
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.launch
import org.junit.Test
import org.robolectric.annotation.Config

/**
 * The discovery board on Android: Read's Home rows, View all, Search with words and its source
 * rows in the board's states, and e-ink's Browse, a row's grid and Search. Search's states are
 * staged over the engine's own answer by [ReadDiscoveryStage].
 */
class ReadDiscoveryShots : ReadingShotHarness() {
    private val homeRows = { store: AppStore -> store.reading.home.value?.rows?.any { it.tiles.isNotEmpty() } == true }

    /** Scroll Home's list until [words] shows. */
    private fun scrollTo(words: String) {
        compose.onAllNodes(hasScrollAction())[0].performScrollToNode(hasText(words, substring = true))
    }

    private fun firstRowTitle(store: AppStore): String {
        val row = store.reading.home.value?.rows?.first { it.tiles.isNotEmpty() } ?: return ""
        return row.title.text(compose.activity)
    }

    // Home's rows

    @Test
    @Config(qualifiers = PHONE)
    fun phone_rows_history() {
        var title = ""
        shoot(
            "phone-discovery-rows-history", FixtureScene.READING_HOME, Destination.Read,
            act = { title = firstRowTitle(requireNotNull(store)); scrollTo(title) },
            ready = homeRows,
        )
    }

    @Test
    @Config(qualifiers = PHONE)
    fun phone_rows_first_run() = shoot(
        "phone-discovery-rows-first-run", FixtureScene.READING_DEMO, Destination.Home, medium = KaeriIsFor.READ,
        act = { scrollTo(firstRowTitle(requireNotNull(store))) },
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_rows_history() = shoot(
        "tablet-discovery-rows-history", FixtureScene.READING_HOME, Destination.Read,
        act = { scrollTo(firstRowTitle(requireNotNull(store))) },
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = PHONE)
    fun phone_view_all() = shoot(
        "phone-discovery-view-all", FixtureScene.READING_HOME, Destination.Read,
        act = {
            scrollTo(firstRowTitle(requireNotNull(store)))
            compose.onAllNodesWithText("View all")[0].performClick() // i18n-skip
        },
        acted = { it.reading.viewAll.value.tiles.isNotEmpty() },
        ready = homeRows,
    )

    // Search with words

    private fun search(name: String, state: String, qualifiers: String = PHONE) {
        val staged = AtomicBoolean(false)
        shoot(
            name, FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ,
            act = { scope.launch { staged.set(ReadDiscoveryStage.stage(requireNotNull(store), state)) } },
            acted = { staged.get() && it.reading.search.value.sources != null },
            ready = { it.reading.home.value != null || it.reading.search.value.tiles.isNotEmpty() },
        )
    }

    @Test @Config(qualifiers = PHONE)
    fun phone_search_mid() = search("phone-discovery-search-mid", "rd-search-mid")

    @Test @Config(qualifiers = PHONE)
    fun phone_search_done() = search("phone-discovery-search-done", "rd-search-done")

    @Test @Config(qualifiers = PHONE)
    fun phone_search_unreachable() = search("phone-discovery-search-unreachable", "rd-search-unreachable")

    @Test @Config(qualifiers = PHONE)
    fun phone_search_sources_only() = search("phone-discovery-search-sources-only", "rd-search-sources-only")

    @Test @Config(qualifiers = PHONE)
    fun phone_search_nothing() = search("phone-discovery-search-nothing", "rd-search-nothing")

    @Test @Config(qualifiers = TABLET)
    fun tablet_search_mid() = search("tablet-discovery-search-mid", "rd-search-mid", TABLET)

    // E-ink

    @Test
    @Config(qualifiers = EREADER)
    fun eink_home_browse_row() = shoot(
        "eink-reader-home-browse", FixtureScene.READING_HOME, Destination.Read, eink = EinkLook(),
        act = { scrollTo("Browse") }, // i18n-skip
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = EREADER)
    fun eink_browse() = shoot(
        "eink-reader-browse", FixtureScene.READING_HOME, Destination.Read, page = "read/read/browse", eink = EinkLook(), // i18n-skip
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = EREADER)
    fun eink_row() = shoot(
        "eink-reader-row", FixtureScene.READING_HOME, Destination.Read, page = "read/read/row:trending-manga", eink = EinkLook(), // i18n-skip
        ready = { store -> store.reading.home.value?.rows?.any { it.id == "trending-manga" && it.tiles.isNotEmpty() } == true }, // i18n-skip
    )

    @Test
    @Config(qualifiers = EINK_PHONE)
    fun eink_phone_browse() = shoot(
        "eink-phone-browse", FixtureScene.READING_HOME, Destination.Read, page = "read/read/browse", eink = EinkLook(), // i18n-skip
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = EREADER)
    fun eink_search() {
        val staged = AtomicBoolean(false)
        shoot(
            "eink-reader-search", FixtureScene.READING_HOME, Destination.Read, page = "read/read/search", eink = EinkLook(), // i18n-skip
            act = { scope.launch { staged.set(ReadDiscoveryStage.stage(requireNotNull(store), "rd-search-done")) } },
            acted = { staged.get() && it.reading.search.value.sources != null },
            ready = { it.reading.home.value != null },
        )
    }
}
