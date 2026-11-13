// The strip reader: the chapter's images one under the other, full width on a phone and 620 dp
// wide on a large screen. A tap goes to the screen, which asks the engine's Scrolling tap zones
// what it does (Off, the default, shows the controls); a touch while the strip still moves only
// stops it, and a scroll hides the controls. There is no double tap here, so a tap acts at once. Where the reader settles goes to the engine at most every 500 ms, with whether the foot of
// the last image is in view, which is what the read rule waits for. With a next chapter the strip
// does not stop: a divider names it, and its images follow in the same scroll, with the chapter
// left kept above them.

package app.kaeri.reader

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.ui.input.pointer.AwaitPointerEventScope
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerInputChange
import androidx.compose.ui.input.pointer.changedToUpIgnoreConsumed
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalDensity
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.model.CreditPage
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.LeafPart
import app.kaeri.engine.model.StripPos
import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.animate
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.runtime.withFrameNanos
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter

/**
 * Where a strip stands: the image at the top of the screen and how far into it, 0 to 1. Only the
 * [count] items from [first] count, so a chapter kept above does not move the place.
 */
fun LazyListState.stripPos(first: Int = 0, count: Int = Int.MAX_VALUE): StripPos? {
    val info = layoutInfo
    val top = info.visibleItemsInfo.firstOrNull { it.index >= first && it.index - first < count } ?: return null
    val offset = if (top.size > 0) (-top.offset).toFloat() / top.size else 0f
    return StripPos((top.index - first).toUInt(), offset.coerceIn(0f, 1f))
}

/**
 * The strip's step for Next and Previous. A tap that lands while a step still moves the strip adds
 * its step to what is left of that one, so two quick taps go two steps: the touch's landing has
 * stopped the first by then, so what was left is kept from that moment.
 */
class StripStep {
    private var job: Job? = null
    private var left = 0f
    private var caught = 0f

    /** At a touch's landing: true when a step moves the strip, keeping what is left of it for the touch. */
    fun catch(): Boolean {
        val moving = job?.isActive == true
        caught = if (moving) left else 0f
        return moving
    }

    /**
     * Scroll [list] by [by] pixels and what is left of a step, gliding by [spec] when [smooth]. A
     * [touch] carries on the step its landing stopped; a screen reader's action has no landing.
     */
    fun go(scope: CoroutineScope, list: LazyListState, by: Float, smooth: Boolean, spec: AnimationSpec<Float>, touch: Boolean) {
        val total = (if (job?.isActive == true) left else if (touch) caught else 0f) + by
        caught = 0f
        job?.cancel()
        left = total
        job = scope.launch {
            // The touch that stopped a step lets go of the list as it lifts: the step waits for that.
            withFrameNanos { }
            if (smooth) {
                var done = 0f
                list.scroll {
                    animate(0f, total, animationSpec = spec) { value, _ ->
                        done += scrollBy(value - done)
                        left = total - done
                    }
                }
            } else {
                list.scrollBy(total)
            }
            left = 0f
        }
    }
}

/** The lift of [down]'s touch if it stays within [slop] of where it landed, whatever else takes its events; null once it moves. */
private suspend fun AwaitPointerEventScope.awaitLift(down: PointerInputChange, slop: Float): PointerInputChange? {
    while (true) {
        val change = awaitPointerEvent(PointerEventPass.Initial).changes.firstOrNull { it.id == down.id } ?: return null
        if ((change.position - down.position).getDistance() > slop) return null
        if (change.changedToUpIgnoreConsumed()) return change
    }
}

/** Whether the foot of the item at [last] is on screen. */
fun LazyListState.footVisible(last: Int): Boolean {
    val info = layoutInfo
    val item = info.visibleItemsInfo.firstOrNull { it.index == last } ?: return false
    return item.offset + item.size <= info.viewportEndOffset
}

/** The width over the height of [part] as drawn: its file's once it landed, else the page's own. */
fun ReaderState.ratioOf(part: LeafPart?): Float {
    part ?: return GUESSED_RATIO
    val landed = file(part)?.let { f ->
        val w = f.crop?.width ?: f.width
        val h = f.crop?.height ?: f.height
        if (h > 0u && w > 0u) w.toFloat() / h.toFloat() else null
    }
    return landed ?: part.ratio?.takeIf { it > 0f } ?: GUESSED_RATIO
}

/** A strip cut the engine has no shape for yet: a tall page's. */
private const val GUESSED_RATIO = 0.7f

/** The divider between two chapters of a strip. */
val StripDividerHeight = 168.dp

/**
 * The strip over [state]'s leaves, each a single image. [onAt] gets each settled place, at most
 * every 500 ms; [onScrolled] runs when a scroll starts, for the screen to hide its controls;
 * [onTap] for a tap that did not stop the strip, at fractions of the strip's width and height. [footer] follows the last image when there is no next chapter: the end
 * page. With one, [divider] names it, and [onFlow] runs once the divider comes into view, for the
 * session to open that chapter under this one. [onShowing] says whether the chapter across the
 * middle of the screen is the one kept above.
 */
@Composable
fun StripReader(
    state: ReaderState,
    images: PageImages,
    onAt: (at: StripPos, footVisible: Boolean) -> Unit,
    onTap: (x: Float, y: Float) -> Unit,
    onScrolled: () -> Unit,
    onMissing: (LeafPart) -> Unit,
    onRetry: (page: Int) -> Unit,
    onJumped: (id: Long) -> Unit,
    footer: @Composable (Leaf) -> Unit,
    modifier: Modifier = Modifier,
    /** The divider into a chapter; with it the strip flows on instead of stopping on [footer]. */
    divider: (@Composable (ChapterRow) -> Unit)? = null,
    onFlow: (next: ChapterRow) -> Unit = {},
    /** [onMissing] and [onRetry] for the chapter kept above after a flow, asked by its own id. */
    onMissingAbove: (LeafPart) -> Unit = {},
    onRetryAbove: (page: Int) -> Unit = {},
    onShowing: (previous: Boolean) -> Unit = {},
    /** The strip came to rest scrolled back into the chapter kept above, for the session to open it again. */
    onBack: () -> Unit = {},
    list: LazyListState = rememberLazyListState(),
    /** At a touch's landing: true when the strip moves by a tap's own step, which a tap carries on. */
    stepping: () -> Boolean = { false },
    maxWidth: Dp = 620.dp,
    gap: Dp = 0.dp,
    waiting: WaitingSlot = { page, long -> ImageWaiting(page, long) },
    failed: FailedSlot = { page, failure, retry -> PageFailed(page, failure, retry) },
    /**
     * Credit pages the engine took out of [chapter], drawn where they were: before the image they
     * sat before, and after the last image once those at the end are shown.
     */
    credits: (@Composable (chapter: ChapterId, pages: List<CreditPage>) -> Unit)? = null,
    /** In the room under a divider while that chapter is on its way: the wait it is on, as a first open says it. */
    room: @Composable () -> Unit = {},
) {
    val story = state.leaves.filter { !it.endPage }
    val end = state.leaves.lastOrNull()?.takeIf { it.endPage }
    // The end page's next chapter, which offline is the next one on the device or none; a chapter
    // the strip failed to flow into ends it on the end page instead, with Try again.
    val next = state.endNext?.takeIf { divider != null && it.id != state.flowFailed?.chapter }
    val previous = state.previous?.takeIf { divider != null }
    val above = previous?.leaves?.filter { !it.endPage }.orEmpty()
    // The chapter kept above and the divider into this one come first.
    val first = if (previous != null) above.size + 1 else 0
    val last = first + story.size - 1
    // A chapter counts as passed only with its images in: one still loading is short, so its
    // divider or foot could come into view with nothing read. Its foot is the end only once the
    // last image is there, and it flows on only while every image of it on screen landed or failed.
    val now by rememberUpdatedState(state)
    val placed by rememberUpdatedState(story to first)
    fun inView(): Boolean = list.layoutInfo.visibleItemsInfo.all { item ->
        val (images, from) = placed
        val leaf = images.getOrNull(item.index - from) ?: return@all true
        leaf.parts.all { now.file(it) != null || now.failures[it.page] != null }
    }
    val lastLanded by rememberUpdatedState(story.lastOrNull()?.parts?.all { state.file(it) != null } == true)
    val at by rememberUpdatedState(onAt)
    val scrolled by rememberUpdatedState(onScrolled)
    val flow by rememberUpdatedState(onFlow)
    val showing by rememberUpdatedState(onShowing)
    val back by rememberUpdatedState(onBack)

    LaunchedEffect(state.jump, story.size) {
        val jump = state.jump ?: return@LaunchedEffect
        if (story.isEmpty()) return@LaunchedEffect
        val index = first + (jump.strip?.image?.toInt() ?: jump.leaf).coerceIn(0, story.size - 1)
        list.scrollToItem(index)
        val size = list.layoutInfo.visibleItemsInfo.firstOrNull { it.index == index }?.size ?: 0
        val into = ((jump.strip?.offset ?: 0f) * size).toInt()
        if (into > 0) list.scrollToItem(index, into)
        onJumped(jump.id)
    }
    // The place, each time a scroll comes to rest once the jump to the start place is done, short
    // scrolls included; again with each layout, which a chapter opened again from above it gets
    // after the place was said. At rest with only the chapter kept above on screen, the divider
    // into this one gone below, that chapter is the one read again.
    LaunchedEffect(list, story.size, first, state.layout) {
        snapshotFlow { list.isScrollInProgress }
            .distinctUntilChanged()
            .collect { moving ->
                if (moving) {
                    scrolled()
                } else if (first > 0 && list.layoutInfo.visibleItemsInfo.let { items -> items.isNotEmpty() && items.all { it.index < first - 1 } }) {
                    back()
                } else if (now.jump == null && story.isNotEmpty()) {
                    list.stripPos(first, story.size)?.let { at(it, lastLanded && list.footVisible(last)) }
                }
            }
    }
    LaunchedEffect(list, story.size, first) {
        // A long scroll still reports where it is, every 500 ms.
        snapshotFlow { list.firstVisibleItemIndex }
            .filter { list.isScrollInProgress && story.isNotEmpty() }
            .collect {
                list.stripPos(first, story.size)?.let { pos -> at(pos, lastLanded && list.footVisible(last)) }
                delay(500)
            }
    }
    // The next chapter opens once its divider comes on screen in a scroll, so its images are there
    // to scroll on to. A chapter that just opened under it, or is still short of its images, never
    // flows on by itself.
    LaunchedEffect(list, next?.id, story.isNotEmpty()) {
        val upcoming = next ?: return@LaunchedEffect
        if (story.isEmpty()) return@LaunchedEffect
        val key = dividerKey(upcoming)
        snapshotFlow { list.isScrollInProgress && inView() && list.layoutInfo.visibleItemsInfo.any { it.key == key } }
            .filter { it }
            .collect { flow(upcoming) }
    }
    // Which chapter is across the middle of the screen, for the group's chapter line.
    LaunchedEffect(list, first) {
        snapshotFlow {
            val info = list.layoutInfo
            val middle = (info.viewportStartOffset + info.viewportEndOffset) / 2
            val across = info.visibleItemsInfo.firstOrNull { it.offset <= middle && it.offset + it.size > middle }
            across != null && across.index < first - 1
        }.distinctUntilChanged().collect { showing(it) }
    }

    // A screen reader rests on the strip as on a paged page, by its page: the moves are its actions.
    // The image at the top of the screen, in the chapter kept above or the open one; on the divider
    // or the room under it, none, and the divider's own words are read.
    val context = LocalContext.current
    val shown by remember(list, first, story.size, above.size) {
        derivedStateOf { list.layoutInfo.visibleItemsInfo.firstNotNullOfOrNull { stripImageAt(it.index, above.size, first, story.size) } }
    }
    val words = shown?.let { (inAbove, image) ->
        if (inAbove) previous?.let { above.getOrNull(image)?.let { leaf -> describe(context, leaf, it) } } else story.getOrNull(image)?.let { describe(context, it, state) }
    }.orEmpty()
    val tapped by rememberUpdatedState(onTap)
    val stepped by rememberUpdatedState(stepping)
    BoxWithConstraints(
        modifier.fillMaxSize().pointerInput(list) {
            awaitEachGesture {
                // Read before the list sees the touch, which stops a moving strip. A strip moving by
                // itself only stops; one moving by a tap's own step takes the next tap at once.
                val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                val step = stepped()
                val moving = list.isScrollInProgress && !step
                // The list takes a touch on a moving strip as a drag, and keeps its lift: a tap
                // during a step is the touch's own lift, as long as it stayed put.
                val up = (if (step) awaitLift(down, viewConfiguration.touchSlop) else waitForUpOrCancellation()) ?: return@awaitEachGesture
                if (!moving) tapped(up.position.x / size.width, up.position.y / size.height)
            }
        }.semantics { if (words.isNotEmpty()) contentDescription = words },
    ) {
        val screen = maxHeight
        // Show on a divider or the end page puts the pages skipped at the end back above it: with
        // the divider or the end at the top, the scroll moves up onto them.
        val column = minOf(this.maxWidth, maxWidth)
        val density = LocalDensity.current
        val endShown = state.revealed.filter { key -> state.credits[key.chapter].orEmpty().any { it.page == key.page && it.atEnd } }.toSet()
        var endBefore by remember { mutableStateOf(endShown) }
        LaunchedEffect(endShown) {
            val added = endShown - endBefore
            endBefore = endShown
            if (added.isEmpty() || credits == null) return@LaunchedEffect
            val width = with(density) { column.toPx() }
            val height = added.sumOf { key ->
                val page = state.credits[key.chapter].orEmpty().first { it.page == key.page }
                val w = (page.file.crop?.width ?: page.file.width).toFloat()
                val h = (page.file.crop?.height ?: page.file.height).toFloat()
                (if (w > 0f) width * h / w else width * 1.5f).toDouble() + with(density) { 44.dp.toPx() }
            }.toFloat()
            val top = list.layoutInfo.visibleItemsInfo.firstOrNull()?.key
            if (top is String && (top.startsWith("divider ") || top == "end")) list.animateScrollBy(-height)
        }
        LazyColumn(
            state = list,
            modifier = Modifier.fillMaxSize(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(gap),
            contentPadding = PaddingValues(0.dp),
        ) {
            val into = state.view?.chapter
            if (previous != null && into != null && divider != null) {
                images(previous, above, images, onMissingAbove, onRetryAbove, maxWidth, waiting, failed, state, credits)
                item(key = dividerKey(into)) {
                    Box(Modifier.widthIn(max = maxWidth).fillMaxWidth().height(StripDividerHeight)) { divider(into) }
                }
                // Room under the divider while this chapter's images come, so the scroll stays put.
                // It is keyed by the chapter it waits for, as the room under that chapter's divider
                // was before the flow, so once the images come they take its place: under one key
                // with the room after them, a scroll resting in it would follow it to the end.
                if (story.isEmpty()) item(key = roomKey(into)) { Box(Modifier.fillMaxWidth().height(screen)) { room() } }
            }
            images(state, story, images, onMissing, onRetry, maxWidth, waiting, failed, state, credits)
            when {
                story.isEmpty() -> {}
                next != null && divider != null -> {
                    item(key = dividerKey(next)) {
                        Box(Modifier.widthIn(max = maxWidth).fillMaxWidth().height(StripDividerHeight)) { divider(next) }
                    }
                    item(key = roomKey(next)) {
                        Box(Modifier.fillMaxWidth().height(screen)) { if (state.asking?.chapter == next.id) room() }
                    }
                }
                end != null -> item(key = "end") {
                    Box(Modifier.widthIn(max = maxWidth).fillMaxWidth().height(screen)) { footer(end) }
                }
            }
        }
    }
}

/**
 * The strip item at [index] as an image: whether it is in the chapter kept above ([aboveCount]
 * images from the top) and its image there, or in the open chapter's [count] from [first]. Null
 * for a divider or the room under one.
 */
internal fun stripImageAt(index: Int, aboveCount: Int, first: Int, count: Int): Pair<Boolean, Int>? = when {
    first > 0 && index < aboveCount -> true to index
    index >= first && index - first < count -> false to index - first
    else -> null
}

/** The room under the divider into [chapter]. */
private fun roomKey(chapter: ChapterRow) = "room ${chapter.id}"

private fun dividerKey(chapter: ChapterRow) = "divider ${chapter.id}"

/** One chapter's images, each its own height before it lands, keyed by chapter so a flow keeps them. */
private fun LazyListScope.images(
    state: ReaderState,
    story: List<Leaf>,
    images: PageImages,
    onMissing: (LeafPart) -> Unit,
    onRetry: (page: Int) -> Unit,
    maxWidth: Dp,
    waiting: WaitingSlot,
    failed: FailedSlot,
    /** The open state, which holds the credit pages of this chapter and the one above. */
    open: ReaderState,
    credits: (@Composable (chapter: ChapterId, pages: List<CreditPage>) -> Unit)?,
) {
    val chapter = state.chapter
    // The credit pages by the image they sat before; those at the end after the last image, and
    // only once shown, since the divider or the end page names them while skipped.
    val groups = if (credits == null || chapter == null) emptyMap() else open.chapterCredits(chapter).groupBy { credit ->
        val anchor = open.anchor(credit, chapter)
        if (credit.atEnd) maxOf(anchor, story.size) else anchor
    }
    val lastIndex = story.lastIndex
    itemsIndexed(story, key = { _, leaf -> "$chapter ${leaf.index}" }) { at, leaf ->
        val part = leaf.parts.firstOrNull()
        val ratio = state.ratioOf(part)
        // A row on screen with no file asks for its image; the session asks once a layout.
        val missing = part != null && state.landed(part) == null && state.failures[part.page] == null
        LaunchedEffect(part?.page, missing) { if (missing && part != null) onMissing(part) }
        val before = groups[at]
        val after = if (at == lastIndex) groups.filterKeys { it >= story.size }.values.flatten().filter { open.isRevealed(it, chapter ?: "") } else emptyList()
        Column(Modifier.widthIn(max = maxWidth).fillMaxWidth()) {
            if (before != null && chapter != null) credits?.invoke(chapter, before)
            Box(Modifier.fillMaxWidth().aspectRatio(ratio)) {
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
            if (after.isNotEmpty() && chapter != null) credits?.invoke(chapter, after)
        }
    }
}
