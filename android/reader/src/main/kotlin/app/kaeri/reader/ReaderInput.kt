// Taps and keys to reader actions, apart from any view so both readers and their tests share them.
// The boards' rules: thirds on a phone, mirrored for right to left; on e-ink the forward zone is
// 40% wide at the reading side, controls 30% in the middle and back 30%. Page buttons arrive as
// the volume keys, the upper one forward.

package app.kaeri.reader

import android.view.KeyEvent
import app.kaeri.engine.model.ReadDirection

/** What a tap or a key asks of the reader. */
enum class ReaderAction { FORWARD, BACK, CONTROLS, CLEAN }

/** The bands across the screen, from the left edge, as fractions of its width. */
data class TapZones(val forward: ClosedFloatingPointRange<Float>, val controls: ClosedFloatingPointRange<Float>) {
    /** The action a tap at [x] (from 0 at the left to 1 at the right) asks for. */
    fun at(x: Float): ReaderAction = when (x.coerceIn(0f, 1f)) {
        in forward -> ReaderAction.FORWARD
        in controls -> ReaderAction.CONTROLS
        else -> ReaderAction.BACK
    }

    companion object {
        /** Thirds: forward on the reading side, controls in the middle, back on the other. */
        fun phone(direction: ReadDirection): TapZones = zones(direction, forward = 1f / 3, controls = 1f / 3)

        /** The e-ink reader's bands: forward 40%, controls 30%, back 30%. */
        fun eink(direction: ReadDirection): TapZones = zones(direction, forward = 0.4f, controls = 0.3f)

        private fun zones(direction: ReadDirection, forward: Float, controls: Float): TapZones =
            if (direction.rightToLeft) {
                TapZones(0f..forward, forward..forward + controls)
            } else {
                TapZones(1f - forward..1f, 1f - forward - controls..1f - forward)
            }
    }
}

/** Pages move leftwards to go on: forward is at the left of the screen. */
val ReadDirection.rightToLeft: Boolean get() = this == ReadDirection.RIGHT_TO_LEFT

/** How the reader reads keys: the setting's choices and whether a book is open. */
data class ReaderKeyRules(
    val direction: ReadDirection,
    /** Volume keys turn pages; with a book open they are taken from the volume. */
    val volumeTurns: Boolean = true,
    /** The upper key goes back instead of forward. */
    val swapVolume: Boolean = false,
    /** Holding the forward key cleans the screen (e-ink). */
    val holdCleans: Boolean = false,
)

/**
 * The action for a key press, or null for keys the reader leaves to the system. A repeat of a
 * held forward key is [ReaderAction.CLEAN] once when [ReaderKeyRules.holdCleans], and otherwise
 * taken without turning again, so a held button does not race through the chapter.
 */
fun ReaderKeyRules.action(keyCode: Int, shift: Boolean = false, repeat: Int = 0, longPress: Boolean = false): KeyAnswer? {
    val forwardArrow = if (direction.rightToLeft) KeyEvent.KEYCODE_DPAD_LEFT else KeyEvent.KEYCODE_DPAD_RIGHT
    val backArrow = if (direction.rightToLeft) KeyEvent.KEYCODE_DPAD_RIGHT else KeyEvent.KEYCODE_DPAD_LEFT
    val action = when (keyCode) {
        KeyEvent.KEYCODE_VOLUME_UP -> if (!volumeTurns) return null else if (swapVolume) ReaderAction.BACK else ReaderAction.FORWARD
        KeyEvent.KEYCODE_VOLUME_DOWN -> if (!volumeTurns) return null else if (swapVolume) ReaderAction.FORWARD else ReaderAction.BACK
        KeyEvent.KEYCODE_PAGE_DOWN, KeyEvent.KEYCODE_DPAD_DOWN, forwardArrow -> ReaderAction.FORWARD
        KeyEvent.KEYCODE_PAGE_UP, KeyEvent.KEYCODE_DPAD_UP, backArrow -> ReaderAction.BACK
        KeyEvent.KEYCODE_SPACE -> if (shift) ReaderAction.BACK else ReaderAction.FORWARD
        KeyEvent.KEYCODE_MENU, KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_DPAD_CENTER -> ReaderAction.CONTROLS
        else -> return null
    }
    if (repeat > 0 || longPress) {
        val cleans = holdCleans && action == ReaderAction.FORWARD && (longPress || repeat == 1)
        return KeyAnswer(if (cleans) ReaderAction.CLEAN else null)
    }
    return KeyAnswer(action)
}

/** A key the reader takes: [action] is null for one it takes without acting, such as a repeat. */
data class KeyAnswer(val action: ReaderAction?)

/** The reader's answer to an Android key event, for any view or activity to call. */
fun ReaderKeyRules.onKeyDown(event: KeyEvent): KeyAnswer? =
    action(event.keyCode, event.isShiftPressed, event.repeatCount, event.isLongPress)

/** A key up the reader takes too, so the system never sees half of a press. */
fun ReaderKeyRules.takesKeyUp(keyCode: Int): Boolean = action(keyCode) != null
