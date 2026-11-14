// What the reader needs to know before the engine answers: whether Paper mode turns pages at once,
// so the reader comes in at once too. Kept from the last settings the engine sent. Also the phone's
// own volume key swap, which only the phone has and so stays on this device.

package app.kaeri.reader.phone

import android.content.Context
import androidx.compose.runtime.mutableStateOf
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderValue

object ReaderLook {
    private const val PREFS = "kaeri_reader_ui"
    private const val PAPER = "paper_instant"
    private const val SWAP = "volume_up_forward"
    private val swapped = mutableStateOf<Boolean?>(null)

    /** Whether the reader last read in Paper mode with pages turning at once. */
    fun paper(context: Context): Boolean =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean(PAPER, false)

    /** Keep what [settings] say for the next open. */
    fun remember(context: Context, settings: ReaderSettingsView) {
        val instant = settings.toggle(ReaderSettingKey.PAPER_MODE) && settings.toggle(ReaderSettingKey.PAPER_INSTANT)
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        if (prefs.getBoolean(PAPER, false) != instant) prefs.edit().putBoolean(PAPER, instant).apply()
    }

    /**
     * Volume up turns pages forward instead of volume down. Read in composition, so a change
     * reaches the reader and the switch at once.
     */
    fun volumeSwapped(context: Context): Boolean =
        swapped.value ?: context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean(SWAP, false)

    fun swapVolume(context: Context, on: Boolean) {
        swapped.value = on
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putBoolean(SWAP, on).apply()
    }

    private fun ReaderSettingsView.toggle(key: ReaderSettingKey): Boolean =
        (rows.firstOrNull { it.key == key }?.value as? ReaderValue.Toggle)?.on == true
}
