package app.kaeri.ui

import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import app.kaeri.eink.EinkLook
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.store.AppStore
import app.kaeri.store.FixtureScene
import org.junit.Test
import org.robolectric.annotation.Config

/**
 * The e-ink Library of the large panel board: on the 10.3 inch Air 4C at its font scale of 0.85 and
 * at 1.0, as a list, with the View sheet open and turned sideways; on the 6 inch, the 7 inch (with
 * its lists sheet) and the e-ink phone. Reading holds 23 made-up series, so a page fills and pages.
 */
class EinkLibraryShots : ReadingShotHarness() {
    /** Every list in, then Reading as 23 series made from the fixture's own tiles. */
    private val listsIn = { store: AppStore -> store.reading.lists.value.size >= READ_LISTS }
    private val filled = { store: AppStore -> store.reading.lists.value[LibrarySegmentRead.READING]?.size == TITLES.size }

    private fun fill(store: AppStore) {
        val lists = store.reading.lists.value
        val bases = lists.values.flatten().distinctBy { it.work }
        if (bases.isEmpty()) return
        val tiles = TITLES.mapIndexed { index, title ->
            val base = bases[index % bases.size]
            base.copy(
                work = "a:99${100 + index}", // i18n-skip
                title = title,
                medium = if (index % 4 == 3) Medium.NOVEL else Medium.MANGA,
                newChapter = index % 5 == 1,
            )
        }
        store.apply(Event.Reading(ReadingEvent.Library(LibrarySegmentRead.READING, tiles)))
    }

    private fun library(name: String, screen: Boolean = false, act: (() -> Unit)? = null) = shoot(
        name, FixtureScene.READING_HOME, Destination.Library, medium = KaeriIsFor.READ, eink = EinkLook(), screen = screen,
        then = ::fill,
        act = act,
        acted = filled,
        ready = listsIn,
    )

    private fun listView(name: String) = library(name) {
        compose.onNodeWithText("View").performClick() // i18n-skip
        compose.onNodeWithText("List").performClick() // i18n-skip
        compose.onNodeWithText("Done").performClick() // i18n-skip
    }

    private fun viewSheet(name: String) = library(name, screen = true) {
        compose.onNodeWithText("View").performClick() // i18n-skip
    }

    private fun listsSheet(name: String) = library(name, screen = true) {
        compose.onNodeWithText("Reading · ${TITLES.size}").performClick() // i18n-skip
    }

    // 10.3 inch

    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_library() = library("air-085-library")
    @Test @Config(qualifiers = EREADER_10) fun air_library_1() = library("air-100-library")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_list() = listView("air-085-library-list")
    @Test @Config(qualifiers = EREADER_10) fun air_list_1() = listView("air-100-library-list")
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_view() = viewSheet("air-085-library-view")
    @Test @Config(qualifiers = EREADER_10_LAND, fontScale = AIR_SCALE) fun air_land() = library("air-land-085-library")

    // The smaller panels

    @Test @Config(qualifiers = EREADER) fun six() = library("six-library")
    @Test @Config(qualifiers = EREADER_7) fun seven() = library("seven-library")
    @Test @Config(qualifiers = EREADER_7) fun seven_lists() = listsSheet("seven-library-lists")
    @Test @Config(qualifiers = EREADER_7) fun seven_view() = viewSheet("seven-library-view")
    @Test @Config(qualifiers = EINK_PHONE) fun phone() = library("eink-phone-library")
    @Test @Config(qualifiers = EINK_PHONE) fun phone_list() = listView("eink-phone-library-list")

    private companion object {
        const val AIR_SCALE = 0.85f
        const val EREADER_10_LAND = "w1323dp-h992dp-land-300dpi"
        const val READ_LISTS = 7
        val TITLES = listOf(
            "Fog Harbour Bakery", "Ember Post Office", "Kite Season", "Harbour of Moths", "Paper Comets",
            "Yorugawa Courier", "Umbrella Choir", "Tidewater Detective", "Saltglass Tower", "Lamplight Arithmetic",
            "Kumo no Shoten", "The Quiet Forge", "Kiln Street Almanac", "The Heron Clerk", "Glass Orchard",
            "Rooftop Gardeners", "Canal Lights", "Bellmaker's Daughter", "A Map of Snowfall", "Inkwell Station",
            "Night Ferry Club", "Quill of the Northern Hall", "Brookside Familiar",
        ) // i18n-skip
    }
}
