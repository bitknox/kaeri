package app.kaeri.reader.phone

import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ReaderSettingKey as Key
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.SettingScope
import app.kaeri.reader.ReaderFixtures
import app.kaeri.reader.ReaderState
import org.junit.Assert.assertEquals
import org.junit.Test

// A switch in the phone reader's sheet writes where the value in effect is kept. Crop borders a
// series keeps on this device (the large e-ink reader sets it there) changes for that series,
// rather than the device default the series' own value still overrides.
class ReaderToggleScopeTest {
    private fun toggle(key: Key, scope: SettingScope) = ReaderSettingRow(key, ReaderValue.Toggle(true), scope, false, scope == SettingScope.SERIES_HERE, emptyList())

    private val state = ReaderState(
        settings = ReaderSettingsView(
            Medium.MANGA, ReaderFixtures.WORK,
            listOf(toggle(Key.CROP_BORDERS, SettingScope.SERIES_HERE), toggle(Key.SKIP_CREDIT_PAGES, SettingScope.MEDIUM)),
            null,
        ),
    )

    @Test
    fun a_switch_writes_where_its_value_is_kept() {
        assertEquals(SettingScope.SERIES_HERE, state.toggleScope(Key.CROP_BORDERS))
        assertEquals(SettingScope.MEDIUM, state.toggleScope(Key.SKIP_CREDIT_PAGES))
    }

    @Test
    fun a_switch_with_no_row_keeps_to_this_device() {
        assertEquals(SettingScope.DEVICE, state.toggleScope(Key.QUIET_PHONE))
        assertEquals(SettingScope.DEVICE, ReaderState().toggleScope(Key.CROP_BORDERS))
    }
}
