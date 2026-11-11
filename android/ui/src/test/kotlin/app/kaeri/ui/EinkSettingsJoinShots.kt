package app.kaeri.ui

import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isSelected
import androidx.compose.ui.test.onNodeWithText
import app.kaeri.eink.EinkLook
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.InvitePreview
import app.kaeri.engine.model.InvitedStore
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.StorePluginRow
import app.kaeri.engine.model.StorePluginStatus
import app.kaeri.store.FixtureScene
import org.junit.Test
import org.robolectric.annotation.Config

/**
 * Settings and the Join a plugin store dialog in the e-ink look on a 10.3 inch e-reader at 300 dpi,
 * the Onyx Boox Note Air 4C's 992 × 1323 dp, at the panel's own font scale of 0.85 and at 1.0;
 * Settings also on the 7 inch and 6 inch panels, which keep their one or two panes.
 */
class EinkSettingsJoinShots : ReadingShotHarness() {
    private fun settings(name: String, pane: SettingsPane, after: (() -> Unit)? = null) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, page = settingsRoute(pane), eink = EinkLook(),
        after = after,
        ready = { true },
    )

    /** The 10 inch Settings keeps the bar under it, with More marked. */
    private val moreMarked: () -> Unit = { compose.onNode(hasText("More") and isSelected()).assertExists() } // i18n-skip

    /** General in the reading app: the bar, and adult titles hidden where reading shows them. */
    private val general: () -> Unit = {
        moreMarked()
        compose.onNodeWithText("Titles AniList marks adult, in Search and Browse", useUnmergedTree = true).assertExists() // i18n-skip
    }

    /** Reading on an e-reader: what stays here is on this device, not this tablet. */
    private val reading: () -> Unit = {
        compose.onNodeWithText("Text size, margins, tap zones and refresh stay on this device.", useUnmergedTree = true).assertExists() // i18n-skip
    }

    /** The sixteen made-up plugins of the board, each with the hosts it may contact and its version. */
    private val plugins = listOf(
        "alpha" to "0.7.0", "beta" to "0.3.1", "gamma" to "0.3.0", "delta" to "0.1.0", "epsilon" to "0.3.0", "zeta" to "1.2.0", // i18n-skip
        "eta" to "0.4.2", "theta" to "0.2.0", "iota" to "0.5.1", "kappa" to "0.1.3", "lambda" to "0.9.0", "mu" to "0.2.4", // i18n-skip
        "nu" to "0.3.3", "xi" to "0.1.0", "omicron" to "0.6.0", "pi" to "0.2.1", // i18n-skip
    ).mapIndexed { i, (name, version) ->
        val hosts = when (i % 3) {
            0 -> listOf("$name.example", "cdn.$name.example") // i18n-skip
            1 -> listOf("api.$name.example") // i18n-skip
            else -> listOf("$name.example") // i18n-skip
        }
        StorePluginRow(id = name, name = name, version = version, hosts = hosts, status = StorePluginStatus.Available)
    }

    /** The Join a plugin store dialog over Settings, with a preview of a made-up store and its plugins. */
    private fun join(name: String) = shoot(
        name, FixtureScene.READING_HOME, Destination.Home, medium = KaeriIsFor.READ, page = settingsRoute(SettingsPane.Sources), eink = EinkLook(), screen = true,
        then = { store ->
            store.stores.openJoin(INVITE)
            val invited = InvitedStore(INVITE, "Owl Lane", "https://store.alpha.example", "store.alpha.example", "9f3c 1a2b 77de 0c41", alreadyJoined = false, owners = listOf("mika"), plugins = plugins, deviceLabel = "Android tablet") // i18n-skip
            store.apply(listOf(Event.StorePreview(InvitePreview.Store(invited))))
        },
        ready = { true },
    )

    // 10.3 inch at the panel's 0.85
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_settings_general() = settings("air-085-settings-general", SettingsPane.General, general)
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_settings_reading() = settings("air-085-settings-reading", SettingsPane.Reading, reading)
    @Test @Config(qualifiers = EREADER_10, fontScale = AIR_SCALE) fun air_join() = join("air-085-join-store")

    // 10.3 inch at 1.0
    @Test @Config(qualifiers = EREADER_10) fun air_settings_general_1() = settings("air-100-settings-general", SettingsPane.General, general)
    @Test @Config(qualifiers = EREADER_10) fun air_settings_reading_1() = settings("air-100-settings-reading", SettingsPane.Reading, reading)
    @Test @Config(qualifiers = EREADER_10) fun air_join_1() = join("air-100-join-store")

    // The smaller panels keep their panes, in the same look
    @Test @Config(qualifiers = EREADER_7) fun seven_settings() = settings("seven-settings-reading", SettingsPane.Reading)
    @Test @Config(qualifiers = EREADER) fun six_settings() = settings("six-settings-reading", SettingsPane.Reading)
    @Test @Config(qualifiers = EREADER_7) fun seven_join() = join("seven-join-store")

    private companion object {
        const val AIR_SCALE = 0.85f
        const val INVITE = "https://store.alpha.example/join/4kq2v9#9f3c1a2b77de0c41" // i18n-skip
    }
}
