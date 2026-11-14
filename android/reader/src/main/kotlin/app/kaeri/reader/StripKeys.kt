// Keys in a strip, which scrolls where pages turn: Down and Up a short step, Space, Shift-Space,
// Page Down, Page Up and the page buttons a screen less a little overlap (the tap's step), Home
// and End the top of the chapter's first image and the foot of its last. Left and Right do
// nothing. A held arrow keeps scrolling; a held page key moves once, as it turns once on pages.

package app.kaeri.reader

import android.view.KeyEvent
import androidx.compose.animation.core.AnimationSpec
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.unit.dp
import app.kaeri.engine.model.tapScrollStep
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/** How far Down and Up scroll a strip: the step the Mac and Windows readers take. */
val StripKeyLine = 120.dp

/** What a key does in a strip. [NONE] is a key the strip takes without moving. */
enum class StripMove { LINE_DOWN, LINE_UP, SCREEN_DOWN, SCREEN_UP, TOP, BOTTOM, NONE }

/**
 * The strip's move for a key, or null for a key it leaves to the reader (Enter, Escape, G) or the
 * system. The page buttons follow [ReaderKeyRules] as they do on pages.
 */
fun ReaderKeyRules.stripMove(keyCode: Int, shift: Boolean = false, repeat: Int = 0, longPress: Boolean = false): StripMove? {
    val action = action(keyCode, shift)?.action ?: return null
    val held = repeat > 0 || longPress
    return when (keyCode) {
        KeyEvent.KEYCODE_DPAD_DOWN -> StripMove.LINE_DOWN
        KeyEvent.KEYCODE_DPAD_UP -> StripMove.LINE_UP
        KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_RIGHT -> StripMove.NONE
        else -> when (action) {
            ReaderAction.FORWARD -> StripMove.SCREEN_DOWN
            ReaderAction.BACK -> StripMove.SCREEN_UP
            ReaderAction.FIRST -> StripMove.TOP
            ReaderAction.LAST -> StripMove.BOTTOM
            else -> return null
        }.let { if (held) StripMove.NONE else it }
    }
}

/**
 * Takes the strip's keys ahead of [readerKeys] while [on], handing each move to [onMove]. Their
 * key ups are left to [readerKeys], which takes them.
 */
fun Modifier.stripKeys(rules: ReaderKeyRules, on: Boolean, onMove: (StripMove) -> Unit): Modifier = onPreviewKeyEvent { event ->
    if (!on || event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
    val native = event.nativeKeyEvent
    val move = rules.stripMove(native.keyCode, native.isShiftPressed, native.repeatCount, native.isLongPress) ?: return@onPreviewKeyEvent false
    onMove(move)
    true
}

/**
 * Where the open chapter's images sit in the strip's list, which [StripReader] keeps up to date,
 * and the moves the keys ask for.
 */
class StripKeys {
    /** The list index of the chapter's first image, after a chapter kept above and its divider. */
    var first = 0
    /** The list index of its last image; below [first] while it has none. */
    var last = -1

    /**
     * Moves [list] as [move] asks: the steps through [step], gliding by [spec] when [smooth], a
     * [line] of pixels for the arrows; Home and End at once, as a page jump is.
     */
    fun go(move: StripMove, scope: CoroutineScope, list: LazyListState, step: StripStep, line: Float, smooth: Boolean, spec: AnimationSpec<Float>) {
        val screen = list.layoutInfo.viewportSize.height * tapScrollStep().toFloat()
        when (move) {
            StripMove.LINE_DOWN -> step.go(scope, list, line, smooth, spec, touch = false)
            StripMove.LINE_UP -> step.go(scope, list, -line, smooth, spec, touch = false)
            StripMove.SCREEN_DOWN -> step.go(scope, list, screen, smooth, spec, touch = false)
            StripMove.SCREEN_UP -> step.go(scope, list, -screen, smooth, spec, touch = false)
            StripMove.TOP -> if (last >= first) {
                step.stop()
                val to = first
                scope.launch { list.scrollToItem(to) }
            }
            StripMove.BOTTOM -> if (last >= first) {
                step.stop()
                val to = last
                scope.launch {
                    list.scrollToItem(to)
                    val info = list.layoutInfo
                    val item = info.visibleItemsInfo.firstOrNull { it.index == to } ?: return@launch
                    // The foot of the last image at the foot of the screen: up from its top for a
                    // tall image, down onto the one above for a short one.
                    list.scrollBy((item.offset + item.size - info.viewportEndOffset).toFloat())
                }
            }
            StripMove.NONE -> {}
        }
    }
}
