// A novel on the phone: the engine's served chapter in a web view, with the paginator and the
// stylesheet the engine sends, and the reader's own chrome over it.

package app.kaeri.reader.phone

import android.annotation.SuppressLint
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import app.kaeri.engine.model.BookResource
import app.kaeri.engine.model.NovelAssets
import app.kaeri.engine.model.TurnHow
import app.kaeri.reader.NovelMessage
import app.kaeri.reader.NovelWebHost
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState

@SuppressLint("ClickableViewAccessibility")
@Composable
fun NovelScreen(
    session: ReaderSession,
    state: ReaderState,
    assets: NovelAssets,
    resource: (String) -> BookResource?,
    onControls: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val current = rememberUpdatedState(state)
    val host = remember(session) {
        NovelWebHost(context, assets, resource) { message ->
            val now = current.value
            when (message) {
                is NovelMessage.Page -> message.pos?.let {
                    session.novelAt(it, message.page.toUInt(), message.pages.toUInt(), message.how ?: TurnHow.JUMP)
                }
                // The last page of the last document: on to the next chapter, when there is one.
                is NovelMessage.End -> now.view?.next?.let { session.openChapter(it.id) }
                else -> {}
            }
        }
    }
    DisposableEffect(host) { onDispose { host.destroy() } }
    val novel = state.novel
    LaunchedEffect(novel?.session, novel?.chapter) {
        if (novel != null) host.open(novel.spine, novel.entry, novel.restore)
    }
    LaunchedEffect(state.novelStyle?.style) {
        state.novelStyle?.let { host.configure(it.style) }
    }
    Box(modifier.fillMaxSize()) {
        AndroidView(factory = { host.view }, modifier = Modifier.fillMaxSize())
        // Taps in the middle third open the chrome; the sides stay with the page.
        Box(
            Modifier
                .align(Alignment.Center)
                .fillMaxWidth(0.34f)
                .fillMaxHeight(0.7f)
                .clickable(indication = null, interactionSource = remember { MutableInteractionSource() }, onClick = onControls),
        )
    }
}
