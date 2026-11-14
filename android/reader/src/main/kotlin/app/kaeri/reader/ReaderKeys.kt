// Keys for a reader drawn in Compose: the reader takes focus so page buttons, the volume keys and
// arrows reach it before the system, and it consumes both halves of every key it acts on. While a
// control inside the reader holds the focus, such as a button in the bars reached with Tab, Enter
// and the D-pad centre act on that control; the arrows, Space and the page keys still turn. A
// focused control that goes away, such as a bar button as the bars hide, hands the focus back to
// the reader rather than to nothing, so the next key still reaches it.

package app.kaeri.reader

import android.view.KeyEvent
import androidx.compose.foundation.focusable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.nativeKeyCode
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import kotlinx.coroutines.launch

/** The keys a focused control answers itself, where the reader would show or hide its controls. */
private val controlKeys = setOf(KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER, KeyEvent.KEYCODE_DPAD_CENTER)

/** Where the reader itself holds the focus, as against a control inside it, and whether either does. */
private class OwnFocus {
    var held = false
    var within = false
}

/**
 * Turns [rules]' keys into [onAction] while this reader is on screen. [focus] puts the focus back
 * on the reader, for a reader whose focused control went away with the bars.
 */
@Composable
fun Modifier.readerKeys(rules: ReaderKeyRules, onAction: (ReaderAction) -> Unit, focus: FocusRequester = remember { FocusRequester() }): Modifier {
    val own = remember { OwnFocus() }
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    return this
        .onFocusChanged {
            val lost = own.within && !it.hasFocus
            own.held = it.isFocused
            own.within = it.hasFocus
            // The focused control left with its bars or page and the focus went nowhere: back to the
            // reader, a frame on, unless something took it meanwhile. Leaving the reader cancels this.
            if (lost) {
                scope.launch {
                    withFrameNanos {}
                    if (!own.within) runCatching { focus.requestFocus() }
                }
            }
        }
        .focusRequester(focus)
        .onPreviewKeyEvent { event ->
            val native = event.nativeKeyEvent
            // A control inside the reader has the focus: Enter is that control's.
            if (!own.held && native.keyCode in controlKeys) return@onPreviewKeyEvent false
            when (event.type) {
                KeyEventType.KeyDown -> {
                    val answer = rules.onKeyDown(native) ?: return@onPreviewKeyEvent false
                    answer.action?.let(onAction)
                    true
                }
                KeyEventType.KeyUp -> rules.takesKeyUp(event.key.nativeKeyCode)
                else -> false
            }
        }
        .focusable()
}
