// The strip reader: the chapter's images one under the other, full width on a phone and 620 dp
// wide on a large screen, with no side zones. A tap anywhere shows the controls and a scroll hides
// them. Where the reader settles goes to the engine at most every 500 ms, with whether the foot of
// the last image is in view, which is what the read rule waits for.

package app.kaeri.reader

import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.LeafPart
import app.kaeri.engine.model.StripPos
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter

/** Where a strip stands: the image at the top of the screen and how far into it, 0 to 1. */
fun LazyListState.stripPos(): StripPos? {
    val info = layoutInfo
    val first = info.visibleItemsInfo.firstOrNull() ?: return null
    val offset = if (first.size > 0) (-first.offset).toFloat() / first.size else 0f
    return StripPos(first.index.toUInt(), offset.coerceIn(0f, 1f))
}

/** Whether the foot of image [last] is on screen. */
fun LazyListState.footVisible(last: Int): Boolean {
    val info = layoutInfo
    val item = info.visibleItemsInfo.firstOrNull { it.index == last } ?: return false
    return item.offset + item.size <= info.viewportEndOffset
}

/**
 * The strip over [state]'s leaves, each a single image. [onAt] gets each settled place, at most
 * every 500 ms; [onScrolled] runs when a scroll starts, for the screen to hide its controls;
 * [onTap] for any tap. [footer] follows the last image: the end page in the flow.
 */
@Composable
fun StripReader(
    state: ReaderState,
    images: PageImages,
    onAt: (at: StripPos, footVisible: Boolean) -> Unit,
    onTap: () -> Unit,
    onScrolled: () -> Unit,
    onMissing: (LeafPart) -> Unit,
    onRetry: (page: Int) -> Unit,
    onJumped: (id: Long) -> Unit,
    footer: @Composable (Leaf) -> Unit,
    modifier: Modifier = Modifier,
    list: LazyListState = rememberLazyListState(),
    maxWidth: Dp = 620.dp,
    gap: Dp = 0.dp,
    waiting: WaitingSlot = { page, long -> PageWaiting(page, long) },
    failed: FailedSlot = { page, failure, retry -> PageFailed(page, failure, retry) },
) {
    val story = state.leaves.filter { !it.endPage }
    val end = state.leaves.lastOrNull()?.takeIf { it.endPage }
    val at by rememberUpdatedState(onAt)
    val scrolled by rememberUpdatedState(onScrolled)

    LaunchedEffect(state.jump, story.size) {
        val jump = state.jump ?: return@LaunchedEffect
        if (story.isEmpty()) return@LaunchedEffect
        val index = (jump.strip?.image?.toInt() ?: jump.leaf).coerceIn(0, story.size - 1)
        list.scrollToItem(index)
        val size = list.layoutInfo.visibleItemsInfo.firstOrNull { it.index == index }?.size ?: 0
        val into = ((jump.strip?.offset ?: 0f) * size).toInt()
        if (into > 0) list.scrollToItem(index, into)
        onJumped(jump.id)
    }
    // The place, once a scroll comes to rest, and no more often than every 500 ms.
    LaunchedEffect(list, story.size) {
        snapshotFlow { list.isScrollInProgress }
            .distinctUntilChanged()
            .collect { moving ->
                if (moving) {
                    scrolled()
                } else if (state.jump == null) {
                    list.stripPos()?.let { at(it, list.footVisible(story.size - 1)) }
                    delay(500)
                }
            }
    }
    LaunchedEffect(list, story.size) {
        // A long scroll still reports where it is, every 500 ms.
        snapshotFlow { list.firstVisibleItemIndex }
            .filter { list.isScrollInProgress }
            .collect {
                list.stripPos()?.let { pos -> at(pos, list.footVisible(story.size - 1)) }
                delay(500)
            }
    }

    BoxWithConstraints(modifier.fillMaxSize().pointerInput(Unit) { detectTapGestures { onTap() } }) {
        LazyColumn(
            state = list,
            modifier = Modifier.fillMaxSize(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(gap),
            contentPadding = PaddingValues(0.dp),
        ) {
            itemsIndexed(story, key = { _, leaf -> leaf.index.toInt() }) { _, leaf ->
                val part = leaf.parts.firstOrNull()
                val file = part?.let(state::file)
                // Its own height before it lands: the file's shape, or a tall page's.
                val ratio = file?.let { f ->
                    val w = f.crop?.width ?: f.width
                    val h = f.crop?.height ?: f.height
                    if (h > 0u) w.toFloat() / h.toFloat() else null
                } ?: 0.7f
                Box(Modifier.widthIn(max = maxWidth).fillMaxWidth().aspectRatio(ratio)) {
                    LeafView(
                        leaf = leaf,
                        state = state,
                        images = images,
                        onMissing = onMissing,
                        onRetry = onRetry,
                        waiting = waiting,
                        failed = failed,
                    )
                }
            }
            if (end != null) {
                item(key = "end") {
                    Box(Modifier.widthIn(max = maxWidth).fillMaxWidth().height(this@BoxWithConstraints.maxHeight)) { footer(end) }
                }
            }
        }
    }
}
