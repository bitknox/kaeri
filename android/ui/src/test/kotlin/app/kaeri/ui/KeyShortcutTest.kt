package app.kaeri.ui

import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Which keys are the app's shortcuts: Ctrl alone with a number, F, comma or [. */
class KeyShortcutTest {
    @Test
    fun ctrlWithANumberGoesToThatSection() {
        assertEquals(KeyShortcut.Section(0), keyShortcut(KeyEvent.KEYCODE_1, ctrl = true))
        assertEquals(KeyShortcut.Section(4), keyShortcut(KeyEvent.KEYCODE_5, ctrl = true))
    }

    @Test
    fun ctrlWithFCommaOrBracketSearchesSettlesOrGoesBack() {
        assertEquals(KeyShortcut.Search, keyShortcut(KeyEvent.KEYCODE_F, ctrl = true))
        assertEquals(KeyShortcut.Settings, keyShortcut(KeyEvent.KEYCODE_COMMA, ctrl = true))
        assertEquals(KeyShortcut.Back, keyShortcut(KeyEvent.KEYCODE_LEFT_BRACKET, ctrl = true))
    }

    @Test
    fun keysWithoutCtrlOrWithMoreModifiersAreLeftToTheScreens() {
        assertNull(keyShortcut(KeyEvent.KEYCODE_F, ctrl = false))
        assertNull(keyShortcut(KeyEvent.KEYCODE_F, ctrl = true, shift = true))
        assertNull(keyShortcut(KeyEvent.KEYCODE_1, ctrl = true, alt = true))
        assertNull(keyShortcut(KeyEvent.KEYCODE_A, ctrl = true))
    }
}
