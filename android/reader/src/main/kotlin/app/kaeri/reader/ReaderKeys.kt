// Keys for a reader drawn in Compose: the reader takes focus so page buttons, the volume keys and
// arrows reach it before the system, and it consumes both halves of every key it acts on. While a
// control inside the reader holds the focus, such as a button in the bars reached with Tab, Enter
// and the D-pad centre act on that control; the arrows, Space and the page keys still turn. A
// reader whose controls take the D-pad while they show (e-ink) passes [ReaderControlKeys]: then
// the arrows move the focus among the controls instead, and the page keys still turn.

package app.kaeri.reader

import android.view.KeyEvent
import androidx.compose.foundation.focusable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.nativeKeyCode
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalFocusManager

/** The keys a focused control answers itself, where the reader would show or hide its controls. */
private val controlKeys = setOf(KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER, KeyEvent.KEYCODE_DPAD_CENTER)

/** The D-pad's arrows, and the way each moves the focus. */
private val arrows = mapOf(
    KeyEvent.KEYCODE_DPAD_UP to FocusDirection.Up,
    KeyEvent.KEYCODE_DPAD_DOWN to FocusDirection.Down,
    KeyEvent.KEYCODE_DPAD_LEFT to FocusDirection.Left,
    KeyEvent.KEYCODE_DPAD_RIGHT to FocusDirection.Right,
)

/** The way a D-pad arrow moves the focus, or null for any other key. */
fun arrowDirection(keyCode: Int): FocusDirection? = arrows[keyCode]

/** Where a key goes while the reader is open. */
enum class KeyRoute {
    /** The reader's own: a turn, the controls, Escape and the rest of [ReaderKeyRules]. */
    READER,

    /** The focused control's, as Enter on a button. */
    CONTROL,

    /** An arrow while the controls show and the reader holds the focus: it goes to their first control. */
    INTO_CONTROLS,

    /** An arrow while the controls show and one of them holds the focus: it moves to the next one that way. */
    MOVE_FOCUS,
}

/**
 * Where [keyCode] goes: [readerHolds] says the reader itself holds the focus, as against a
 * control inside it, and [controlsTakeArrows] that the controls show and take the D-pad.
 */
fun keyRoute(keyCode: Int, readerHolds: Boolean, controlsTakeArrows: Boolean): KeyRoute = when {
    controlsTakeArrows && keyCode in arrows -> if (readerHolds) KeyRoute.INTO_CONTROLS else KeyRoute.MOVE_FOCUS
    !readerHolds && keyCode in controlKeys -> KeyRoute.CONTROL
    else -> KeyRoute.READER
}

/**
 * The D-pad among a reader's controls: while [shown], the arrows move the focus among them, and
 * from the reader itself [enter] puts the focus on their first control.
 */
class ReaderControlKeys(val shown: Boolean, val enter: () -> Unit)

/** Where the reader itself holds the focus, as against a control inside it. */
private class OwnFocus {
    var held = false
}

/**
 * Turns [rules]' keys into [onAction] while this reader is on screen. [focus] puts the focus back
 * on the reader, for a reader whose focused control went away with the bars. [controls], when
 * given, hands the arrows to the reader's controls while they show.
 */
@Composable
fun Modifier.readerKeys(
    rules: ReaderKeyRules,
    onAction: (ReaderAction) -> Unit,
    focus: FocusRequester = remember { FocusRequester() },
    controls: ReaderControlKeys? = null,
): Modifier {
    val own = remember { OwnFocus() }
    val focusManager = LocalFocusManager.current
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    return this
        .onFocusChanged { own.held = it.isFocused }
        .focusRequester(focus)
        .onPreviewKeyEvent { event ->
            val native = event.nativeKeyEvent
            when (keyRoute(native.keyCode, own.held, controls?.shown == true)) {
                // A control inside the reader has the focus: Enter is that control's.
                KeyRoute.CONTROL -> false
                KeyRoute.INTO_CONTROLS -> {
                    if (event.type == KeyEventType.KeyDown) controls?.enter?.invoke()
                    true
                }
                // Moved here, and taken either way, so an arrow at the controls' edge never turns
                // the page under them or goes on to the system.
                KeyRoute.MOVE_FOCUS -> {
                    if (event.type == KeyEventType.KeyDown) arrowDirection(native.keyCode)?.let { focusManager.moveFocus(it) }
                    true
                }
                KeyRoute.READER -> when (event.type) {
                    KeyEventType.KeyDown -> {
                        val answer = rules.onKeyDown(native) ?: return@onPreviewKeyEvent false
                        answer.action?.let(onAction)
                        true
                    }
                    KeyEventType.KeyUp -> rules.takesKeyUp(event.key.nativeKeyCode)
                    else -> false
                }
            }
        }
        .focusable()
}
