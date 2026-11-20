package app.kaeri.tv

import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import app.kaeri.store.SettingsModel
import app.kaeri.tv.Gamepad.Fate
import app.kaeri.tv.Gamepad.Settings

/** A game controller's keys, as Settings' Controller group has them. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class GamepadTest {
    private val on = Settings(enabled = true, swapFace = false)
    private val swapped = Settings(enabled = true, swapFace = true)
    private val off = Settings(enabled = false, swapFace = true)

    @Test
    fun a_controller_goes_on_as_android_maps_it_until_a_setting_says_otherwise() {
        for (code in listOf(KeyEvent.KEYCODE_BUTTON_A, KeyEvent.KEYCODE_BUTTON_B, KeyEvent.KEYCODE_DPAD_LEFT)) {
            assertEquals(Fate.Same, Gamepad.fate(code, controller = true, settings = on))
        }
    }

    @Test
    fun swapped_a_goes_back_and_b_confirms() {
        assertEquals(Fate.As(KeyEvent.KEYCODE_BACK), Gamepad.fate(KeyEvent.KEYCODE_BUTTON_A, controller = true, settings = swapped))
        assertEquals(Fate.As(KeyEvent.KEYCODE_DPAD_CENTER), Gamepad.fate(KeyEvent.KEYCODE_BUTTON_B, controller = true, settings = swapped))
        // The rest of the controller is as it was.
        assertEquals(Fate.Same, Gamepad.fate(KeyEvent.KEYCODE_BUTTON_X, controller = true, settings = swapped))
        assertEquals(Fate.Same, Gamepad.fate(KeyEvent.KEYCODE_DPAD_UP, controller = true, settings = swapped))
    }

    @Test
    fun switched_off_the_controller_does_nothing_and_the_remote_goes_on() {
        for (code in listOf(KeyEvent.KEYCODE_BUTTON_A, KeyEvent.KEYCODE_BUTTON_B, KeyEvent.KEYCODE_DPAD_DOWN)) {
            assertEquals(Fate.Ignored, Gamepad.fate(code, controller = true, settings = off))
        }
        for (code in listOf(KeyEvent.KEYCODE_DPAD_CENTER, KeyEvent.KEYCODE_BACK, KeyEvent.KEYCODE_DPAD_DOWN)) {
            assertEquals(Fate.Same, Gamepad.fate(code, controller = false, settings = off))
        }
    }

    @Test
    fun before_the_engine_says_the_settings_the_controller_works_unswapped() {
        assertEquals(Settings(enabled = true, swapFace = false), Settings.of(SettingsModel()))
    }

    @Test
    fun a_retyped_key_keeps_the_press_it_came_from() {
        val down = KeyEvent(100L, 120L, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BUTTON_A, 2, 0, 7, 304, 0, 0x00000401)
        val back = Gamepad.retyped(down, KeyEvent.KEYCODE_BACK)
        assertEquals(KeyEvent.KEYCODE_BACK, back.keyCode)
        assertEquals(listOf(100L, 120L, KeyEvent.ACTION_DOWN.toLong(), 2L, 7L, 304L, 0x00000401L),
            listOf(back.downTime, back.eventTime, back.action.toLong(), back.repeatCount.toLong(), back.deviceId.toLong(), back.scanCode.toLong(), back.source.toLong()))
    }
}
