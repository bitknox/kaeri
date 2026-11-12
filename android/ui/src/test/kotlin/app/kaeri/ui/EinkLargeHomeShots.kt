package app.kaeri.ui

import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.kaeri.eink.EinkLook
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.Notice
import app.kaeri.engine.model.NoticeKind
import app.kaeri.engine.model.ReadWeekDay
import app.kaeri.engine.model.ReadWeekEntry
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.store.AppStore
import app.kaeri.store.FixtureScene
import kotlin.math.abs
import org.junit.Test
import org.robolectric.annotation.Config

/**
 * Home, Browse, Downloads and More on a 10.3 inch e-reader, the Onyx Boox Note Air 4C's
 * 992 × 1323 dp, at the panel's font scale of 0.85 and at 1.0, as the large panel board draws
 * them; and Home on the 6 inch, the 7 inch and the e-ink phone, which keep their own Home.
 *
 * The fixtures hold one series in Continue and one new chapter, so the 10 inch shots also take a
 * fuller Home, Downloads and messages, made from the fixtures' own series under made-up titles.
 */
class EinkLargeHomeShots : ReadingShotHarness() {
    private val homeRows = { store: AppStore -> store.reading.home.value?.rows?.any { it.tiles.isNotEmpty() } == true }

    /** Home as the fixtures have it. */
    private fun home(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, eink = EinkLook(), ready = homeRows,
    )

    /** Home with Continue and New chapters filled from the discovery rows' series. */
    private fun fullHome(name: String, after: (() -> Unit)? = null) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, eink = EinkLook(),
        then = { store -> fill(store) },
        after = after,
        ready = { homeRows(it) && (it.reading.home.value?.continueReading?.size ?: 0) >= 1 },
    )

    /**
     * On its side the large Home is widened, with Continue reaching a fifth series and New
     * chapters beside the hero: right of 800 dp and above Continue, its heading on the hero's
     * "Continue reading" line, its link kept.
     */
    private fun landHome(name: String) = fullHome(name) {
        compose.onNodeWithText(CONTINUE_TITLES.last()).assertExists()
        val continueTop = compose.onNodeWithText("Continue").getBoundsInRoot().top // i18n-skip
        val link = compose.onNodeWithText("All new chapters").getBoundsInRoot() // i18n-skip
        check(link.left >= 800.dp && link.bottom <= continueTop) { "All new chapters is not beside the hero" } // i18n-skip
        // The Quiet Forge, a made-up title the hero doesn't share; placed where the list is.
        val rows = compose.onAllNodesWithText(NEW_TITLES.first(), substring = true).fetchSemanticsNodes().map { node ->
            with(compose.density) { node.boundsInRoot.left.toDp() to node.boundsInRoot.bottom.toDp() }
        }
        check(rows.any { (left, bottom) -> left >= 800.dp && bottom <= continueTop }) { "New chapters is not beside the hero: $rows" } // i18n-skip
        val heading = baseline("New chapters") // i18n-skip
        val kicker = baseline("Continue reading") // i18n-skip
        check(abs((heading - kicker).value) <= 1f) { "New chapters baseline at $heading, Continue reading at $kicker" } // i18n-skip
    }

    /** Where [text]'s first line sits on the page: its node's top and the first baseline. */
    private fun baseline(text: String): Dp {
        val node = compose.onNodeWithText(text, useUnmergedTree = true).fetchSemanticsNode()
        val layouts = mutableListOf<TextLayoutResult>()
        node.config[SemanticsActions.GetTextLayoutResult].action?.invoke(layouts)
        return with(compose.density) { (node.boundsInRoot.top + layouts.first().firstBaseline).toDp() }
    }

    private fun browse(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, page = "home/read/browse", eink = EinkLook(), ready = homeRows, // i18n-skip
    )

    private fun downloads(name: String) = shoot(
        name, FixtureScene.READING_DOWNLOADS, Destination.Downloads, medium = KaeriIsFor.READ, eink = EinkLook(),
        then = { store -> fillDownloads(store) },
        ready = { it.reading.downloads.value != null },
    )

    private fun more(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.More, medium = KaeriIsFor.READ, eink = EinkLook(),
        then = { store ->
            MESSAGES.forEachIndexed { i, words -> store.apply(Event.Toast(Notice(if (i == 1) NoticeKind.ALERT else NoticeKind.INFO, words, null))) }
            store.notices.current.value?.let { store.notices.dismiss(it) }
        },
        ready = { it.bookSources.view.value != null },
    )

    /** Continue and This week made from Home's discovery rows, renamed, as a reader with a full shelf has them. */
    private fun fill(store: AppStore) {
        val view = store.reading.home.value ?: return
        val art = view.rows.flatMap { it.tiles }.distinctBy { it.work }
        if (art.isEmpty()) return
        val continuing = view.continueReading + CONTINUE_TITLES.mapIndexed { i, title -> art[i % art.size].copy(title = title) }
        val now = System.currentTimeMillis() / 1000
        val entries = NEW_TITLES.mapIndexed { i, title ->
            ReadWeekEntry(tile = art[(i + 3) % art.size].copy(title = title), outAt = now - i * 3600L, usually = false, chapter = null)
        }
        val today = view.week.firstOrNull { it.today } ?: ReadWeekDay(day = now - now % 86_400, today = true, entries = emptyList())
        val week = listOf(today.copy(entries = today.entries + entries)) + view.week.filter { !it.today }
        store.apply(Event.Reading(ReadingEvent.Home(view.copy(continueReading = continuing, week = week))))
    }

    /**
     * Offline with no New chapters: the offline line under the title, and Browse right under
     * Continue rather than at the foot of the page with empty room between.
     */
    private fun quietHome(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, eink = EinkLook(),
        then = { store ->
            fill(store)
            val view = store.reading.home.value
            if (view != null) {
                val since = System.currentTimeMillis() / 1000 - 3600
                store.apply(Event.Reading(ReadingEvent.Home(view.copy(week = emptyList(), offline = true, offlineSince = since))))
            }
        },
        after = {
            compose.onNodeWithText("Offline.", substring = true).assertExists() // i18n-skip
            val captions = compose.onNodeWithText(CONTINUE_TITLES.first()).getBoundsInRoot().bottom
            val browse = compose.onNodeWithText("Browse").getBoundsInRoot().top // i18n-skip
            check(browse - captions <= 120.dp) { "Browse starts ${browse - captions} under Continue" } // i18n-skip
        },
        // Ready before the offline Home is applied too: the harness waits for it on both sides.
        ready = { homeRows(it) && (it.reading.home.value?.continueReading?.size ?: 0) >= 1 },
    )

    /** More series on this device, made from the fixtures' own under made-up titles, for the two paged columns. */
    private fun fillDownloads(store: AppStore) {
        val view = store.reading.downloads.value ?: return
        val here = view.series.filter { it.chapters > 0u && it.queued == 0u && it.failed.isEmpty() }.ifEmpty { view.series }
        if (here.isEmpty()) return
        val more = DOWNLOAD_TITLES.mapIndexed { i, title -> here[i % here.size].copy(title = title) }
        store.apply(Event.Reading(ReadingEvent.Downloads(view.copy(series = view.series + more))))
    }

    // 10.3 inch at the panel's 0.85

    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_home() = home("b-air-085-home")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_home_full() = fullHome("b-air-085-home-full")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_home_offline_quiet() = quietHome("b-air-085-home-offline")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_browse() = browse("b-air-085-browse")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_downloads() = downloads("b-air-085-downloads")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_more() = more("b-air-085-more")

    // On its side: the large Home widened, New chapters beside the hero, Continue on five columns.
    @Test @Config(qualifiers = AIR_LAND, fontScale = AIR_SCALE) fun air_land_home() = landHome("air-land-085-home")

    // 10.3 inch at 1.0

    @Test @Config(qualifiers = EREADER_10) fun air_home_1() = home("b-air-100-home")
    @Test @Config(qualifiers = EREADER_10) fun air_home_full_1() = fullHome("b-air-100-home-full")
    @Test @Config(qualifiers = EREADER_10) fun air_browse_1() = browse("b-air-100-browse")
    @Test @Config(qualifiers = EREADER_10) fun air_downloads_1() = downloads("b-air-100-downloads")
    @Test @Config(qualifiers = EREADER_10) fun air_more_1() = more("b-air-100-more")
    @Test @Config(qualifiers = AIR_LAND) fun air_land_home_1() = landHome("air-land-100-home")

    // Home on the smaller panels, as before

    @Test @Config(qualifiers = EREADER) fun six_home() = home("b-six-home")
    @Test @Config(qualifiers = EREADER_7) fun seven_home() = home("b-seven-home")
    @Test @Config(qualifiers = EINK_PHONE) fun phone_home() = home("b-eink-phone-home")

    private companion object {
        const val AIR_SCALE = 0.85f
        const val AIR_LAND = "w1323dp-h992dp-land-300dpi" // i18n-skip
        val CONTINUE_TITLES = listOf("Saltglass Tower", "Lamplight Arithmetic", "Kumo no Shoten", "Tidewater Detective") // i18n-skip
        val NEW_TITLES = listOf("The Quiet Forge", "Umbrella Choir", "Harbour of Moths", "Ember Post Office", "Paper Comets", "The Heron Clerk", "Canal Lights", "Rooftop Gardeners", "Bellmaker's Daughter", "A Map of Snowfall", "Glass Orchard", "Fog Harbour Bakery") // i18n-skip
        val DOWNLOAD_TITLES = listOf("Yorugawa Courier", "Kite Season", "The Quiet Forge", "Tidewater Detective", "Kumo no Shoten", "Fog Harbour Bakery", "Ember Post Office", "Harbour of Moths", "Paper Comets", "The Heron Clerk", "Canal Lights", "Night Ferry Club", "Rooftop Gardeners", "Inkwell Station") // i18n-skip
        val MESSAGES = listOf(
            "Synced progress for 3 series from the phone.", // i18n-skip
            "Northwind didn't answer within 10 seconds for chapter 1 of Lamplight Arithmetic.", // i18n-skip
            "Downloaded chapters 13 to 15 of Yorugawa Courier to keep them ready.", // i18n-skip
            "AniList updated: Kite Season, chapter 19 read.", // i18n-skip
            "Home library at books.home.example didn't answer. It answered again 4 minutes later.", // i18n-skip
        ).reversed()
    }
}
