package app.kaeri.ui

import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.i18n.Strings
import app.kaeri.store.AppStore
import app.kaeri.store.ExportJob
import app.kaeri.store.FixtureScene
import app.kaeri.store.GroupsStage
import app.kaeri.store.ReadFromStage
import app.kaeri.store.SendHoldStage
import app.kaeri.ui.reading.SeriesShots
import org.junit.Assert.assertTrue
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

    /** Read from with a source that has no series by the work's names and one with another edition. */
    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_read_from_misses() = shoot(
        "phone-read-series-read-from-misses",
        FixtureScene.READING_SERIES,
        Destination.Read,
        page = page(MANGA),
        screen = true,
        then = {
            ReadFromStage.stage(it, MANGA)
            SeriesShots.open = "read-from"
        },
        ready = loaded(MANGA),
    )

    /** A source with three groups: the Group row, rows from other groups and in parts, and the picker. */
    private fun groups(name: String, open: String?, eink: app.kaeri.eink.EinkLook? = null) = shoot(
        name,
        FixtureScene.READING_SERIES,
        Destination.Read,
        page = page(MANGA),
        screen = open != null,
        then = {
            GroupsStage.stage(it)
            if (open != null) SeriesShots.open = open
        },
        eink = eink,
        ready = loaded(MANGA),
    )

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_groups() {
        groups("phone-read-series-groups", null)
        compose.spoken("phone-groups")
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_group_sheet() {
        groups("phone-read-series-group-sheet", "group")
        compose.spoken("phone-group-sheet")
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.TABLET)
    fun tablet_read_series_group_menu() = groups("tablet-read-series-group-menu", "group")

    @Test
    @Config(qualifiers = ReadingShotHarness.EINK_PHONE)
    fun eink_read_series_groups() = groups("eink-read-series-groups", null, app.kaeri.eink.EinkLook())

    @Test
    @Config(qualifiers = ReadingShotHarness.EINK_PHONE)
    fun eink_read_series_group_page() {
        groups("eink-read-series-group-page", "group", app.kaeri.eink.EinkLook())
        compose.spoken("eink-group-page")
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_tracking() = series("phone-read-series-tracking", open = "tracking")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_choose() = series("phone-read-series-choose", open = "choose")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_status() = series("phone-read-series-status", open = "status")

    /**
     * On a phone network with chapters kept to Wi-Fi, a send of chapters to fetch holds until
     * Wi-Fi. The scene's chapters are files in a folder, which go at once, so the hold is staged.
     */
    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_send_waits() = shoot(
        "phone-read-series-send-waits",
        FixtureScene.READING_SERIES,
        Destination.Read,
        page = page(MANGA),
        screen = true,
        then = {
            it.networkPath(cellular = true, constrained = false)
            SendHoldStage.stage(it)
            SeriesShots.open = "send"
        },
        act = { compose.onNodeWithText(Strings.sendPrepare(compose.activity)).performClick() },
        acted = { store -> store.export.jobs.value.values.any { it is ExportJob.Waiting } },
        ready = loaded(MANGA),
    )

    /**
     * Closing the sheet on a send held for Wi-Fi leaves it held: it goes on by itself as the sheet
     * says, and the page shows it with Cancel send. Only that stops it.
     */
    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_send_held_after_close() {
        shoot(
            "phone-read-series-send-held",
            FixtureScene.READING_SERIES,
            Destination.Read,
            page = page(MANGA),
            screen = true,
            then = {
                it.networkPath(cellular = true, constrained = false)
                SendHoldStage.stage(it)
                SeriesShots.open = "send"
            },
            act = {
                compose.onNodeWithText(Strings.sendPrepare(compose.activity)).performClick()
                pump("the send held for Wi-Fi") { store -> store.export.jobs.value.values.any { it is ExportJob.Waiting } }
                compose.onNodeWithText(Strings.actionClose(compose.activity)).performClick()
            },
            acted = { store -> store.export.jobs.value.values.any { it is ExportJob.Waiting } },
            ready = loaded(MANGA),
        )
        val held = requireNotNull(store).export.jobs.value.values
        assertTrue("the send is still held: $held", held.singleOrNull() is ExportJob.Waiting)
        compose.onNodeWithText(Strings.sendPrepare(compose.activity)).assertDoesNotExist()
        compose.onNodeWithText(Strings.sendWaitingCancel(compose.activity)).assertExists()
    }

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun phone_read_series_remove() = series("phone-read-series-remove", open = "remove")

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

    // Large text: the page, the chapter sheet and the send sheet at 130% and 200%.

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE, fontScale = 1.3f)
    fun phone_read_series_text_130() = series("phone-read-series-text-130")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE, fontScale = 2.0f)
    fun phone_read_series_text_200() = series("phone-read-series-text-200")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE, fontScale = 1.3f)
    fun phone_read_series_chapter_text_130() = series("phone-read-series-chapter-text-130", open = "chapter")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE, fontScale = 2.0f)
    fun phone_read_series_chapter_text_200() = series("phone-read-series-chapter-text-200", open = "chapter")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE, fontScale = 1.3f)
    fun phone_read_series_send_text_130() = series("phone-read-series-send-text-130", open = "send")

    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE, fontScale = 2.0f)
    fun phone_read_series_send_text_200() = series("phone-read-series-send-text-200", open = "send")

    private companion object {
        const val MANGA = "a:900301"
        const val NOVEL = "a:900302"
    }
}
