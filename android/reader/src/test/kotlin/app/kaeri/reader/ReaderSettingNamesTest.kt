package app.kaeri.reader

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import app.kaeri.engine.model.ReaderSettingKey as Key
import app.kaeri.engine.model.ReaderValue
import app.kaeri.reader.phone.choiceLabel
import app.kaeri.reader.phone.phoneKeys
import app.kaeri.reader.phone.rowTitle
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** Every row the phone draws and every choice the engine can send has a written name. */
@RunWith(RobolectricTestRunner::class)
class ReaderSettingNamesTest {
    private val context: Context = ApplicationProvider.getApplicationContext()

    /** The engine's choices, from `choices_for` in the engine's reader settings. */
    private val choices = mapOf(
        Key.MODE to listOf("paged", "scrolled", "vertical", "strip"),
        Key.DIRECTION to listOf("rtl", "ltr"),
        Key.COLOUR to listOf("auto", "colour", "greys"),
        Key.LOOK to listOf("system", "paper", "light", "sepia", "dark"),
        Key.FONT to listOf("serif", "sans"),
        Key.FIT to listOf("screen", "width", "height"),
        Key.BACKGROUND to listOf("theme", "black", "white"),
        Key.PAGE_LAYOUT to listOf("auto", "single", "double"),
        Key.SPREADS to listOf("split", "rotate", "whole"),
        Key.TAP_LAYOUT to listOf("edges", "sides", "off"),
        Key.FOOT_VALUE to listOf("pages_left"),
    )

    @Test
    fun everyPhoneRowHasATitle() {
        phoneKeys.forEach { assertNotNull("no title for $it", rowTitle(context, it)) }
    }

    @Test
    fun everyChoiceTheEngineSendsHasAName() {
        choices.forEach { (key, names) ->
            names.forEach { assertNotNull("no name for $key $it", choiceLabel(context, key, ReaderValue.Choice(it))) }
        }
    }
}
