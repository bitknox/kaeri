package app.kaeri.ui

import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.store.FixtureScene
import app.kaeri.store.PrelaunchProgress
import app.kaeri.ui.reading.BOOK_SOURCES_ROUTE
import kotlinx.coroutines.runBlocking
import org.junit.Test
import org.robolectric.annotation.Config

/**
 * Did my server get it, on the prelaunch board (piece 4): the server's row in Your sources with
 * its second line, from the made-up server's facts. The page between chapters is drawn in the
 * reader's and the e-ink reader's own shots.
 */
class PrelaunchProgressShots : ReadingShotHarness() {
    private fun row(state: String, device: String) = shoot(
        "pl-$state-$device",
        FixtureScene.READING_HOME,
        Destination.Home,
        page = BOOK_SOURCES_ROUTE,
        medium = KaeriIsFor.READ,
        then = { store -> runBlocking { PrelaunchProgress.stage(store, state) } },
        act = {},
        acted = { it.rowLines.lines.value["books-home"] != null },
    ) { it.bookSources.view.value != null }

    @Test @Config(qualifiers = PHONE) fun p4_06_phone() = row("p4-06", "phone")
    @Test @Config(qualifiers = PHONE) fun p4_07_phone() = row("p4-07", "phone")
    @Test @Config(qualifiers = PHONE) fun p4_08_phone() = row("p4-08", "phone")
    @Test @Config(qualifiers = PHONE) fun p4_09_phone() = row("p4-09", "phone")
    @Test @Config(qualifiers = PHONE) fun p4_10_phone() = row("p4-10", "phone")

    @Test @Config(qualifiers = TABLET) fun p4_06_tablet() = row("p4-06", "tablet")
    @Test @Config(qualifiers = TABLET) fun p4_09_tablet() = row("p4-09", "tablet")

    companion object {
        const val PHONE = ReadingShotHarness.PHONE
        const val TABLET = ReadingShotHarness.TABLET
    }
}
