package app.kaeri.ui

import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.store.AppStore
import app.kaeri.store.FixtureScene
import app.kaeri.ui.reading.SeriesShots
import org.junit.Test
import org.robolectric.annotation.Config

/** A reading series' page, its sheets and menus, and reading's part of Downloads: phone, then tablet. */
class ReadSeriesShots : ReadingShotHarness() {
    private fun page(work: String) = "read/series/" + android.net.Uri.encode(work)

    private fun loaded(work: String): (AppStore) -> Boolean = { it.reading.series.value[work] != null && it.reading.chapters.value[work] != null }

    private fun series(name: String, work: String = MANGA, open: String? = null) = shoot(
        name,
        FixtureScene.READING_SERIES,
        Destination.Read,
        page = page(work),
        screen = open != null,
        then = { if (open != null) SeriesShots.open = open },
        ready = loaded(work),
    )

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series() = series("phone-read-series")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_novel() = series("phone-read-series-novel", NOVEL)

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_chapter() = series("phone-read-series-chapter", open = "chapter")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_read_from() = series("phone-read-series-read-from", open = "read-from")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_tracking() = series("phone-read-series-tracking", open = "tracking")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_choose() = series("phone-read-series-choose", open = "choose")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_status() = series("phone-read-series-status", open = "status")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_download() = series("phone-read-series-download", open = "download")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_read_again() = series("phone-read-series-read-again", open = "read-again")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_send() = series("phone-read-series-send", open = "send")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_downloads() = shoot("phone-read-downloads", FixtureScene.READING_DOWNLOADS, Destination.Read, page = DOWNLOADS_ROUTE) {
        it.reading.downloads.value != null
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_alone_downloads() = shoot(
        "phone-read-alone-downloads",
        FixtureScene.READING_SERIES,
        Destination.Home,
        page = DOWNLOADS_ROUTE,
        medium = KaeriIsFor.READ,
    ) { it.reading.downloads.value != null }

    @Test
    @Config(qualifiers = ReadingShotHarness.TABLET)
    fun tablet_read_series() = series("tablet-read-series")

    @Test
    @Config(qualifiers = ReadingShotHarness.TABLET)
    fun tablet_read_series_chapter() = series("tablet-read-series-chapter", open = "chapter")

    @Test
    @Config(qualifiers = ReadingShotHarness.TABLET)
    fun tablet_read_downloads() = shoot("tablet-read-downloads", FixtureScene.READING_DOWNLOADS, Destination.Read, page = DOWNLOADS_ROUTE) {
        it.reading.downloads.value != null
    }

    private companion object {
        const val MANGA = "a:900301"
        const val NOVEL = "a:900302"
    }
}
