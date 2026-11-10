// A novel on the phone: the engine's served chapter in a web view, with the paginator and the
// stylesheet the engine sends. The reader's own chrome, the end page and a failure are drawn over
// it from the place it reports.

package app.kaeri.reader.phone

import android.annotation.SuppressLint
import android.view.GestureDetector
import android.view.MotionEvent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
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
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.TurnHow
import app.kaeri.reader.NovelMessage
import app.kaeri.reader.NovelWebHost
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import kotlin.math.abs

/**
 * Where the novel's paginator stands, for the chrome around it: the page of the document shown and
 * how many it has, whether the last page was turned past, and why the chapter could not be shown.
 */
@Stable
class NovelPlace {
    var page by mutableIntStateOf(0)
        internal set
    var pages by mutableIntStateOf(0)
        internal set

    /** The web view showed this chapter's first page: the opening plate can go. */
    var ready by mutableStateOf(false)
        internal set

    /** Turned past the last page of the chapter: the end page shows. */
    var ended by mutableStateOf(false)

    /** Why the web view could not show the chapter, or null. */
    var failed by mutableStateOf<String?>(null)
        internal set
    internal var goTo: ((Int) -> Unit)? = null
    internal var reload: (() -> Unit)? = null
    internal var turnBy: ((forward: Boolean) -> Unit)? = null

    /**
     * The reader's keys while the web view has focus, which would otherwise take arrows and Space
     * for itself: true for a key the reader took.
     */
    var keys: ((android.view.KeyEvent) -> Boolean)? = null

    /** Turn a page forward or back, as a key does. */
    fun turn(forward: Boolean) {
        turnBy?.invoke(forward)
    }

    /** Turn to [page] of the document shown. */
    fun go(page: Int) {
        goTo?.invoke(page)
    }

    /** Show the chapter again after it failed. */
    fun retry() {
        failed = null
        reload?.invoke()
    }
}

@SuppressLint("ClickableViewAccessibility")
@Composable
fun NovelScreen(
    session: ReaderSession,
    state: ReaderState,
    place: NovelPlace,
    assets: NovelAssets,
    resource: (String) -> BookResource?,
    onControls: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    // Which side turns forward: the right edge, or the left in a right-to-left book.
    val rtl by rememberUpdatedState(state.view?.direction == ReadDirection.RIGHT_TO_LEFT)
    val scrolled by rememberUpdatedState(state.choice(ReaderSettingKey.MODE) == "scrolled")
    lateinit var turn: (forward: Boolean, how: TurnHow) -> Unit
    val host = remember(session) {
        NovelWebHost(context, assets, resource) { message ->
            when (message) {
                // A tap on a side third turns the page, which the page does itself once it knows
                // which side is forward; the middle third is the chrome's.
                is NovelMessage.Tap -> when {
                    message.x < 1f / 3f -> turn(rtl, TurnHow.TURN)
                    message.x > 2f / 3f -> turn(!rtl, TurnHow.TURN)
                }
                is NovelMessage.Page -> {
                    place.ready = true
                    place.page = message.page
                    place.pages = message.pages
                    place.ended = false
                    message.pos?.let {
                        session.novelAt(it, message.page.toUInt(), message.pages.toUInt(), message.how ?: TurnHow.JUMP)
                    }
                }
                is NovelMessage.Layout -> {
                    place.ready = true
                    place.page = message.page
                    place.pages = message.pages
                }
                // The last page of the last document turned past: the end page, as manga has.
                is NovelMessage.End -> place.ended = true
                is NovelMessage.Failed -> place.failed = message.why
                else -> {}
            }
        }
    }
    turn = { forward, how -> if (forward) host.next(how) else host.prev(how) }
    LaunchedEffect(host, rtl) { host.tapsTurn(1f / 3f, rtl) }
    // A sideways fling turns the page. The web view still has the touch, for selecting text.
    DisposableEffect(host) {
        val flings = GestureDetector(
            context,
            object : GestureDetector.SimpleOnGestureListener() {
                override fun onFling(e1: MotionEvent?, e2: MotionEvent, velocityX: Float, velocityY: Float): Boolean {
                    if (scrolled || abs(velocityX) < abs(velocityY) * 1.5f) return false
                    // The finger moving left turns forward in a left-to-right book.
                    turn((velocityX < 0) != rtl, TurnHow.SWIPE)
                    return true
                }
            },
        )
        host.view.setOnTouchListener { _, event ->
            flings.onTouchEvent(event)
            false
        }
        onDispose { host.view.setOnTouchListener(null) }
    }
    DisposableEffect(host) {
        place.goTo = { host.goTo(it) }
        place.turnBy = { forward -> turn(forward, TurnHow.KEY) }
        // Keys reach the web view first; the reader's own go to the reader.
        host.view.setOnKeyListener { _, _, event -> place.keys?.invoke(event) == true }
        onDispose {
            host.view.setOnKeyListener(null)
            place.turnBy = null
            place.goTo = null
            place.reload = null
            host.destroy()
        }
    }
    val novel = state.novel
    LaunchedEffect(novel?.session, novel?.chapter) {
        place.failed = null
        place.ended = false
        place.ready = false
        if (novel != null) {
            place.reload = { host.open(novel.spine, novel.entry, novel.restore) }
            host.open(novel.spine, novel.entry, novel.restore)
        }
    }
    LaunchedEffect(state.novelStyle?.style) {
        state.novelStyle?.let { host.configure(it.style) }
    }
    Box(modifier.fillMaxSize()) {
        AndroidView(factory = { host.view }, modifier = Modifier.fillMaxSize())
        // Taps in the middle third, top to bottom, open the chrome; the sides stay with the page.
        Box(
            Modifier
                .align(Alignment.Center)
                .fillMaxWidth(1f / 3f)
                .fillMaxHeight()
                .clickable(indication = null, interactionSource = remember { MutableInteractionSource() }, onClick = onControls),
        )
    }
}
