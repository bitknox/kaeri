// The zones the reader falls back on before the engine sends its own, and the keys, mirrored for each direction.

package app.kaeri.reader

import android.view.KeyEvent
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.TapAction
import app.kaeri.engine.model.TapReading
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ReaderInputTest {
    private val rtl = ReadDirection.RIGHT_TO_LEFT
    private val ltr = ReadDirection.LEFT_TO_RIGHT

    private fun paged(direction: ReadDirection) = TapReading(ReaderMode.PAGED, direction, false, false)

    @Test
    fun phone_zones_before_the_engine_sends_them_are_thirds_with_forward_at_the_reading_side() {
        val zones = ReaderState().tapZones()
        assertEquals(TapAction.NEXT, zones.actionAt(paged(rtl), 0.1f, 0.5f))
        assertEquals(TapAction.CONTROLS, zones.actionAt(paged(rtl), 0.5f, 0.5f))
        assertEquals(TapAction.PREVIOUS, zones.actionAt(paged(rtl), 0.9f, 0.5f))
        assertEquals(TapAction.PREVIOUS, zones.actionAt(paged(ltr), 0.1f, 0.5f))
        assertEquals(TapAction.NEXT, zones.actionAt(paged(ltr), 0.9f, 0.5f))
    }

    @Test
    fun eink_zones_before_the_engine_sends_them_are_forty_thirty_thirty() {
        val zones = ReaderState().tapZones(eink = true)
        assertEquals(TapAction.NEXT, zones.actionAt(paged(rtl), 0.39f, 0.5f))
        assertEquals(TapAction.CONTROLS, zones.actionAt(paged(rtl), 0.41f, 0.5f))
        assertEquals(TapAction.PREVIOUS, zones.actionAt(paged(rtl), 0.71f, 0.5f))
        assertEquals(TapAction.PREVIOUS, zones.actionAt(paged(ltr), 0.29f, 0.5f))
        assertEquals(TapAction.NEXT, zones.actionAt(paged(ltr), 0.61f, 0.5f))
    }

    @Test
    fun a_screen_reader_makes_every_tap_the_controls() {
        val zones = ReaderState().tapZones()
        assertEquals(TapAction.CONTROLS, zones.actionAt(TapReading(ReaderMode.PAGED, rtl, false, true), 0.05f, 0.5f))
    }

    @Test
    fun the_upper_page_button_goes_forward_and_both_are_taken() {
        val rules = ReaderKeyRules(rtl)
        assertEquals(ReaderAction.FORWARD, rules.action(KeyEvent.KEYCODE_VOLUME_UP)?.action)
        assertEquals(ReaderAction.BACK, rules.action(KeyEvent.KEYCODE_VOLUME_DOWN)?.action)
        // A phone's default: volume down goes forward, under the thumb.
        val phone = rules.copy(volumeDownForward = true)
        assertEquals(ReaderAction.BACK, phone.action(KeyEvent.KEYCODE_VOLUME_UP)?.action)
        assertEquals(ReaderAction.FORWARD, phone.action(KeyEvent.KEYCODE_VOLUME_DOWN)?.action)
        // Off: the volume keys stay the system's.
        assertNull(rules.copy(volumeTurns = false).action(KeyEvent.KEYCODE_VOLUME_UP))
        assertEquals(true, rules.takesKeyUp(KeyEvent.KEYCODE_VOLUME_DOWN))
    }

    @Test
    fun arrows_follow_the_direction() {
        assertEquals(ReaderAction.FORWARD, ReaderKeyRules(rtl).action(KeyEvent.KEYCODE_DPAD_LEFT)?.action)
        assertEquals(ReaderAction.FORWARD, ReaderKeyRules(ltr).action(KeyEvent.KEYCODE_DPAD_RIGHT)?.action)
        assertEquals(ReaderAction.BACK, ReaderKeyRules(ltr).action(KeyEvent.KEYCODE_SPACE, shift = true)?.action)
        assertEquals(ReaderAction.FORWARD, ReaderKeyRules(ltr).action(KeyEvent.KEYCODE_PAGE_DOWN)?.action)
        assertNull(ReaderKeyRules(ltr).action(KeyEvent.KEYCODE_A))
    }

    @Test
    fun a_held_forward_button_cleans_once_and_never_races() {
        val rules = ReaderKeyRules(rtl, holdCleans = true)
        assertEquals(ReaderAction.CLEAN, rules.action(KeyEvent.KEYCODE_VOLUME_UP, repeat = 1)?.action)
        assertEquals(KeyAnswer(null), rules.action(KeyEvent.KEYCODE_VOLUME_UP, repeat = 2))
        assertEquals(KeyAnswer(null), ReaderKeyRules(rtl).action(KeyEvent.KEYCODE_VOLUME_UP, repeat = 1))
    }

    @Test
    fun home_end_escape_and_g_have_their_own_actions() {
        val rules = ReaderKeyRules(rtl)
        assertEquals(ReaderAction.FIRST, rules.action(KeyEvent.KEYCODE_MOVE_HOME)?.action)
        assertEquals(ReaderAction.LAST, rules.action(KeyEvent.KEYCODE_MOVE_END)?.action)
        assertEquals(ReaderAction.ESCAPE, rules.action(KeyEvent.KEYCODE_ESCAPE)?.action)
        assertEquals(ReaderAction.GO_TO, rules.action(KeyEvent.KEYCODE_G)?.action)
        // A held key does not repeat them.
        assertEquals(KeyAnswer(null), rules.action(KeyEvent.KEYCODE_MOVE_END, repeat = 1))
    }
}
