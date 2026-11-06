// The zones and keys of both boards, mirrored for each direction.

package app.kaeri.reader

import android.view.KeyEvent
import app.kaeri.engine.model.ReadDirection
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ReaderInputTest {
    private val rtl = ReadDirection.RIGHT_TO_LEFT
    private val ltr = ReadDirection.LEFT_TO_RIGHT

    @Test
    fun phone_zones_are_thirds_with_forward_at_the_reading_side() {
        val r = TapZones.phone(rtl)
        assertEquals(ReaderAction.FORWARD, r.at(0.1f))
        assertEquals(ReaderAction.CONTROLS, r.at(0.5f))
        assertEquals(ReaderAction.BACK, r.at(0.9f))
        val l = TapZones.phone(ltr)
        assertEquals(ReaderAction.BACK, l.at(0.1f))
        assertEquals(ReaderAction.CONTROLS, l.at(0.5f))
        assertEquals(ReaderAction.FORWARD, l.at(0.9f))
    }

    @Test
    fun eink_zones_are_forty_thirty_thirty() {
        val r = TapZones.eink(rtl)
        assertEquals(ReaderAction.FORWARD, r.at(0.39f))
        assertEquals(ReaderAction.CONTROLS, r.at(0.41f))
        assertEquals(ReaderAction.CONTROLS, r.at(0.69f))
        assertEquals(ReaderAction.BACK, r.at(0.71f))
        val l = TapZones.eink(ltr)
        assertEquals(ReaderAction.BACK, l.at(0.29f))
        assertEquals(ReaderAction.CONTROLS, l.at(0.31f))
        assertEquals(ReaderAction.FORWARD, l.at(0.61f))
    }

    @Test
    fun the_upper_page_button_goes_forward_and_both_are_taken() {
        val rules = ReaderKeyRules(rtl)
        assertEquals(ReaderAction.FORWARD, rules.action(KeyEvent.KEYCODE_VOLUME_UP)?.action)
        assertEquals(ReaderAction.BACK, rules.action(KeyEvent.KEYCODE_VOLUME_DOWN)?.action)
        val swapped = rules.copy(swapVolume = true)
        assertEquals(ReaderAction.BACK, swapped.action(KeyEvent.KEYCODE_VOLUME_UP)?.action)
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
}
