package app.kaeri.ui

import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.store.AppStore
import app.kaeri.store.FixtureScene
import app.kaeri.store.ReadingModel
import app.kaeri.ui.reading.ReadPlaces
import org.junit.Test
import org.robolectric.annotation.Config

/** Reading's Home at full length, Updates, and the reading side of Library and Search. */
class ReadBrowseShots : ReadingShotHarness() {
    private fun places(library: Boolean = false, search: Boolean = false, list: LibrarySegmentRead = LibrarySegmentRead.READING) {
        ReadPlaces.libraryReading = library
        ReadPlaces.searchReading = search
        ReadPlaces.list = list
    }

    @Test
    @Config(qualifiers = TALL)
    fun phone_read_home_full() {
        places()
        shoot("phone-read-home-full", FixtureScene.READING_HOME, Destination.Read) { it.reading.home.value?.pending?.isEmpty() == true }
    }

    @Test
    @Config(qualifiers = TABLET_TALL)
    fun tablet_read_home_full() {
        places()
        shoot("tablet-read-home-full", FixtureScene.READING_HOME, Destination.Read) { it.reading.home.value?.pending?.isEmpty() == true }
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_updates() {
        places()
        shoot("phone-updates", FixtureScene.READING_HOME, Destination.Updates, medium = KaeriIsFor.READ, ready = updatesWithCovers())
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE, fontScale = 1.3f)
    fun phone_updates_text_130() {
        places()
        shoot("phone-updates-text-130", FixtureScene.READING_HOME, Destination.Updates, medium = KaeriIsFor.READ, ready = updatesWithCovers())
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE, fontScale = 2.0f)
    fun phone_updates_text_200() {
        places()
        shoot("phone-updates-text-200", FixtureScene.READING_HOME, Destination.Updates, medium = KaeriIsFor.READ, ready = updatesWithCovers())
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.TABLET)
    fun tablet_updates() {
        places()
        shoot("tablet-updates", FixtureScene.READING_HOME, Destination.Updates, medium = KaeriIsFor.READ, ready = updatesWithCovers())
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_library_read_alone() {
        places()
        shoot("phone-library-read-alone", FixtureScene.READING_HOME, Destination.Library, medium = KaeriIsFor.READ) {
            it.reading.lists.value[LibrarySegmentRead.READING] != null
        }
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_library_both_reading() {
        places(library = true)
        shoot("phone-library-both-reading", FixtureScene.READING_HOME, Destination.Library) {
            it.reading.lists.value[LibrarySegmentRead.READING] != null
        }
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_library_files_empty() {
        places(library = true, list = LibrarySegmentRead.YOUR_FILES)
        shoot("phone-library-files", FixtureScene.READING_HOME, Destination.Library, medium = KaeriIsFor.READ) {
            it.reading.lists.value[LibrarySegmentRead.YOUR_FILES] != null
        }
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.TABLET)
    fun tablet_library_read_alone() {
        places()
        shoot("tablet-library-read-alone", FixtureScene.READING_HOME, Destination.Library, medium = KaeriIsFor.READ) {
            it.reading.lists.value[LibrarySegmentRead.READING] != null
        }
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_search_read_alone_browse() {
        places()
        shoot("phone-search-read-browse", FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ) { searched(it) }
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_search_read_alone_query() {
        places()
        shoot(
            "phone-search-read-query", FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ,
            then = { it.reading.runSearch(ReadingModel.EMPTY_QUERY.copy(text = "ferry")) },
        ) { searched(it) }
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_search_both_reading_query() {
        places(search = true)
        shoot(
            "phone-search-both-reading", FixtureScene.READING_HOME, Destination.Search,
            then = { it.reading.runSearch(ReadingModel.EMPTY_QUERY.copy(text = "juniper")) },
        ) { searched(it) }
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_search_nothing() {
        places(search = true)
        shoot(
            "phone-search-read-nothing", FixtureScene.READING_HOME, Destination.Search,
            then = { it.reading.runSearch(ReadingModel.EMPTY_QUERY.copy(text = "qqzzx", formats = listOf("manga"))) },
        ) { !it.reading.search.value.loading }
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.TABLET)
    fun tablet_search_read_alone_browse() {
        places()
        shoot("tablet-search-read-browse", FixtureScene.READING_HOME, Destination.Search, medium = KaeriIsFor.READ) { searched(it) }
    }

    private fun searched(store: AppStore) = store.reading.search.value.let { !it.loading && it.tiles.isNotEmpty() }

    companion object {
        const val TALL = "w412dp-h3000dp-port-420dpi"
        const val TABLET_TALL = "w1280dp-h1800dp-land-xhdpi"
    }

    /**
     * Updates once Home has run: the scene keeps the books without art, and Home's catalogue
     * answer keeps them with their covers, as a first launch does before Updates opens.
     */
    private fun updatesWithCovers(): (app.kaeri.store.AppStore) -> Boolean {
        var home = false
        var asked = false
        return { store ->
            if (!home) {
                home = true
                store.reading.loadHome()
            }
            if (!asked && store.reading.home.value != null) {
                asked = true
                store.reading.loadUpdates()
            }
            asked && store.reading.updates.value.toString().contains("cover=http")
        }
    }
}
