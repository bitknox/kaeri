// Keys for a reader drawn in Compose: the reader takes focus so page buttons, the volume keys and
// arrows reach it before the system, and it consumes both halves of every key it acts on.

package app.kaeri.reader

import androidx.compose.foundation.focusable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.nativeKeyCode
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type

/** Turns [rules]' keys into [onAction] while this reader is on screen. */
@Composable
fun Modifier.readerKeys(rules: ReaderKeyRules, onAction: (ReaderAction) -> Unit): Modifier {
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    return this
        .focusRequester(focus)
        .onPreviewKeyEvent { event ->
            val native = event.nativeKeyEvent
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
