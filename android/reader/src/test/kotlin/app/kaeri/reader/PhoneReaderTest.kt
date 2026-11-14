package app.kaeri.reader

import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderValue
import app.kaeri.reader.phone.ReaderLauncher
import app.kaeri.reader.phone.ReaderRequest
import app.kaeri.reader.phone.choice
import app.kaeri.reader.phone.toggle
import app.kaeri.engine.model.ReaderSettingRow
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.core.Medium
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PhoneReaderTest {
    @Test
    fun theLauncherHoldsOneRequestUntilItIsClosed() {
        assertNull(ReaderLauncher.request.value)
        ReaderLauncher.open(ReaderRequest("local:lantern-stair"))
        assertEquals("local:lantern-stair", ReaderLauncher.request.value?.work)
        ReaderLauncher.close()
        assertNull(ReaderLauncher.request.value)
    }

    @Test
    fun theStateReadsItsChoicesAndSwitchesFromTheEnginesRows() {
        val rows = listOf(
            ReaderSettingRow(ReaderSettingKey.FIT, ReaderValue.Choice("width"), SettingScope.DEVICE, false, false, emptyList()),
            ReaderSettingRow(ReaderSettingKey.PAPER_MODE, ReaderValue.Toggle(true), SettingScope.DEVICE, false, false, emptyList()),
        )
        val state = ReaderState(settings = ReaderSettingsView(Medium.MANGA, null, rows))
        assertEquals("width", state.choice(ReaderSettingKey.FIT))
        assertTrue(state.toggle(ReaderSettingKey.PAPER_MODE))
        assertFalse(state.toggle(ReaderSettingKey.QUIET_PHONE))
        assertNull(ReaderState().choice(ReaderSettingKey.FIT))
    }
}
