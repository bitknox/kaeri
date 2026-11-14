package app.kaeri.ui

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasScrollToIndexAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import app.kaeri.eink.EinkLook
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.store.AppStore
import app.kaeri.store.FixtureScene
import app.kaeri.store.ReadDiscoveryStage
import app.kaeri.store.ReadingModel
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

    /** Scroll until the first row shows whole: to the next row's heading when there is one. */
    private fun showFirstRow(store: AppStore) {
        val rows = store.reading.home.value?.rows?.filter { it.tiles.isNotEmpty() }.orEmpty()
        scrollTo(firstRowTitle(store))
        rows.getOrNull(1)?.let { scrollTo(it.title.text(compose.activity)) }
    }

    // Home's rows

    @Test
    @Config(qualifiers = PHONE)
    fun phone_rows_history() = shoot(
        "phone-discovery-rows-history", FixtureScene.READING_HOME, Destination.Read,
        act = { showFirstRow(requireNotNull(store)) },
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = PHONE)
    fun phone_rows_first_run() = shoot(
        "phone-discovery-rows-first-run", FixtureScene.READING_DEMO, Destination.Home, medium = KaeriIsFor.READ,
        act = { showFirstRow(requireNotNull(store)) },
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = TABLET)
    fun tablet_rows_history() = shoot(
        "tablet-discovery-rows-history", FixtureScene.READING_HOME, Destination.Read,
        act = { showFirstRow(requireNotNull(store)) },
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = PHONE)
    fun phone_view_all() = shoot(
        "phone-discovery-view-all", FixtureScene.READING_HOME, Destination.Read,
        act = {
            // The first discovery row's own View all, not another section's.
            val row = requireNotNull(store).reading.home.value?.rows?.first { it.tiles.isNotEmpty() && it.viewAll != null }
            val title = row?.title?.text(compose.activity).orEmpty()
            scrollTo(title)
            compose.onNode(hasContentDescription(", $title", substring = true) and hasClickAction()).performClick()
        },
        // Answered: View all asked, and the engine's first page (or its failure) is in.
        acted = { it.reading.viewAll.value.let { all -> all.query != ReadingModel.EMPTY_QUERY && !all.loading } },
        ready = homeRows,
    )

    // Search with words

    private fun search(name: String, state: String) {
        val staged = AtomicBoolean(false)
        shoot(
            name, FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ,
            act = { scope.launch { staged.set(ReadDiscoveryStage.stage(requireNotNull(store), state)) } },
            acted = { staged.get() && it.reading.search.value.sources != null },
            ready = { true },
        )
    }

    @Test @Config(qualifiers = PHONE)
    fun phone_search_mid() = search("phone-discovery-search-mid", "rd-search-mid")

    /** A source with eight hits: five covers, then Show 3 more in the next slot. A tablet's row shows all six. */
    @Test @Config(qualifiers = TABLET)
    fun tablet_search_source_more() {
        val staged = AtomicBoolean(false)
        shoot(
            "tablet-discovery-search-source-more", FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ,
            act = { scope.launch { staged.set(ReadDiscoveryStage.stage(requireNotNull(store), "rd-search-done")) } },
            acted = { staged.get() && it.reading.search.value.sources != null },
            after = {
                compose.onNode(hasScrollToIndexAction() and SemanticsMatcher.keyIsDefined(SemanticsProperties.VerticalScrollAxisRange))
                    .performScrollToNode(hasText("Show 3 more")) // i18n-skip
            },
            ready = { true },
        )
    }

    @Test @Config(qualifiers = PHONE)
    fun phone_search_done() = search("phone-discovery-search-done", "rd-search-done")

    @Test @Config(qualifiers = PHONE)
    fun phone_search_unreachable() = search("phone-discovery-search-unreachable", "rd-search-unreachable")

    @Test @Config(qualifiers = PHONE)
    fun phone_search_sources_only() = search("phone-discovery-search-sources-only", "rd-search-sources-only")

    @Test @Config(qualifiers = PHONE)
    fun phone_search_nothing() = search("phone-discovery-search-nothing", "rd-search-nothing")

    @Test @Config(qualifiers = TABLET)
    fun tablet_search_mid() = search("tablet-discovery-search-mid", "rd-search-mid")

    // E-ink

    @Test
    @Config(qualifiers = EREADER)
    fun eink_home_browse_row() = shoot(
        "eink-reader-home-browse", FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, eink = EinkLook(),
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = EREADER_7)
    fun eink_home_browse_row_7_inch() = shoot(
        "eink-reader-7-home-browse", FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, eink = EinkLook(),
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = EINK_PHONE)
    fun eink_phone_home() = shoot(
        "eink-phone-home", FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, eink = EinkLook(),
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = EREADER)
    fun eink_home_first_run() = shoot(
        "eink-reader-home-first-run", FixtureScene.READING_DEMO, Destination.Home, medium = KaeriIsFor.READ, eink = EinkLook(),
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = EREADER)
    fun eink_downloads() = shoot(
        "eink-reader-downloads", FixtureScene.READING_DOWNLOADS, Destination.Downloads, medium = KaeriIsFor.READ, eink = EinkLook(),
        ready = { it.reading.downloads.value != null },
    )

    @Test
    @Config(qualifiers = EREADER)
    fun eink_more() = shoot(
        "eink-reader-more", FixtureScene.READING_HOME, Destination.More, medium = KaeriIsFor.READ, eink = EinkLook(),
        ready = { it.bookSources.view.value != null },
    )

    @Test
    @Config(qualifiers = EREADER)
    fun eink_browse() = shoot(
        "eink-reader-browse", FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, page = "home/read/browse", eink = EinkLook(), // i18n-skip
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = EREADER)
    fun eink_row() = shoot(
        "eink-reader-row", FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, page = "home/read/row:trending-manga", eink = EinkLook(), // i18n-skip
        ready = { store -> store.reading.home.value?.rows?.any { it.id == "trending-manga" && it.tiles.isNotEmpty() } == true }, // i18n-skip
    )

    @Test
    @Config(qualifiers = EINK_PHONE)
    fun eink_phone_browse() = shoot(
        "eink-phone-browse", FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, page = "home/read/browse", eink = EinkLook(), // i18n-skip
        ready = homeRows,
    )

    @Test
    @Config(qualifiers = EREADER)
    fun eink_search() {
        val staged = AtomicBoolean(false)
        shoot(
            "eink-reader-search", FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ, eink = EinkLook(),
            act = { scope.launch { staged.set(ReadDiscoveryStage.stage(requireNotNull(store), "rd-search-done")) } },
            acted = { staged.get() && it.reading.search.value.sources != null },
            ready = { true },
        )
    }
}
