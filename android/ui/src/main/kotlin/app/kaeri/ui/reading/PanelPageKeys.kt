// An e-reader's page buttons on the app's lists: on paper a list moves a screen at a time, as the
// reader's pages do, where the buttons would otherwise do nothing or move the focus.

package app.kaeri.ui.reading

import android.view.KeyEvent
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.ScrollableState
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.nativeKeyCode
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.onSizeChanged
import app.kaeri.design.LocalStillPanel
import kotlinx.coroutines.launch

/** How much of a screen one press moves: a little of the last screen stays in view. */
internal const val PANEL_PAGE = 0.9f

/**
 * On an e-ink panel, Page down and Page up move [state] by most of its height. Off a panel the
 * list is left as it is.
 */
@Composable
internal fun Modifier.panelPageKeys(state: ScrollableState): Modifier {
    if (!LocalStillPanel.current) return this
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }
    var height by remember { mutableIntStateOf(0) }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    return this
        .onSizeChanged { height = it.height }
        .focusRequester(focus)
        .onPreviewKeyEvent { event ->
            val by = when (event.key.nativeKeyCode) {
                KeyEvent.KEYCODE_PAGE_DOWN -> 1
                KeyEvent.KEYCODE_PAGE_UP -> -1
                else -> return@onPreviewKeyEvent false
            }
            if (event.type == KeyEventType.KeyDown) scope.launch { state.scrollBy(by * height * PANEL_PAGE) }
            true
        }
        .focusable()
}
