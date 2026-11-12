// A novel on the phone: the engine's served chapter in a web view, with the paginator and the
// stylesheet the engine sends. The reader's own chrome, the end page and a failure are drawn over
// it from the place it reports.

package app.kaeri.reader.phone

import android.annotation.SuppressLint
import android.view.GestureDetector
import android.view.MotionEvent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView
import app.kaeri.engine.model.BookResource
import app.kaeri.engine.model.NovelAssets
import app.kaeri.engine.model.NovelPos
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.TapAction
import app.kaeri.engine.model.TurnHow
import app.kaeri.i18n.Strings
import app.kaeri.reader.actionAt
import app.kaeri.reader.rememberScreenReader
import app.kaeri.reader.tapReading
import app.kaeri.reader.tapZones
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
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
    // Which way a swipe turns forward: leftward, or rightward in a right-to-left book.
    val rtl by rememberUpdatedState(state.view?.direction == ReadDirection.RIGHT_TO_LEFT)
    val scrolled by rememberUpdatedState(state.choice(ReaderSettingKey.MODE) == "scrolled")
    // Taps go by the engine's zones for paged or scrolled pages; a screen reader has actions instead.
    val exploring = rememberScreenReader()
    val zones by rememberUpdatedState(state.tapZones())
    val reading by rememberUpdatedState(state.tapReading(screenReader = exploring))
    val controls by rememberUpdatedState(onControls)
    lateinit var turn: (forward: Boolean, how: TurnHow) -> Unit
    // A renderer that went away leaves its view dead: a new host takes over and opens the
    // chapter again where it was.
    var lives by remember(session) { mutableIntStateOf(0) }
    val last = remember(session) { LastPlace() }
    val host = remember(session, lives) {
        NovelWebHost(context, assets, resource) { message ->
            when (message) {
                // A tap off a link, in fractions of the view: the zone it falls in says what it does.
                // Next and Previous scroll a scrolled chapter by a step.
                is NovelMessage.Tap -> when (reading?.let { zones.actionAt(it, message.x, message.y) } ?: TapAction.CONTROLS) {
                    TapAction.NEXT -> turn(true, TurnHow.TURN)
                    TapAction.PREVIOUS -> turn(false, TurnHow.TURN)
                    TapAction.CONTROLS -> controls()
                }
                is NovelMessage.Page -> {
                    place.ready = true
                    place.page = message.page
                    place.pages = message.pages
                    place.ended = false
                    message.pos?.let {
                        last.pos = it
                        // A first layout says no how: the app's own move, never the person's jump.
                        session.novelAt(it, message.page.toUInt(), message.pages.toUInt(), message.how ?: TurnHow.PLACED)
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
                NovelMessage.Gone -> lives++
                else -> {}
            }
        }
    }
    turn = { forward, how -> if (forward) host.next(how) else host.prev(how) }
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
    LaunchedEffect(host, novel?.session, novel?.chapter) {
        place.failed = null
        place.ended = false
        place.ready = false
        if (novel != null) {
            // A new host for the same chapter, after its renderer went: back to the last place shown.
            val chapter = novel.session to novel.chapter
            val restore = if (last.chapter == chapter) last.pos ?: novel.restore else novel.restore
            if (last.chapter != chapter) last.pos = null
            last.chapter = chapter
            place.reload = { host.open(novel.spine, novel.entry, novel.restore) }
            host.open(novel.spine, novel.entry, restore)
        }
    }
    LaunchedEffect(state.novelGoTo?.id) {
        state.novelGoTo?.let { host.goToPos(it.pos, TurnHow.PLACED) }
    }
    LaunchedEffect(host, state.novelStyle?.style) {
        state.novelStyle?.let { host.configure(it.style) }
    }
    // With a screen reader every tap is the chrome's, so the page turns and the controls are actions.
    val next = if (scrolled) Strings.readerZoneScrollDown(context) else Strings.readerZoneNext(context)
    val previous = if (scrolled) Strings.readerZoneScrollUp(context) else Strings.readerZonePrevious(context)
    val showControls = Strings.readerActionControls(context)
    // Words a screen reader can rest on, so the moves come up as its actions: the page, or the chapter
    // while the paginator counts and when the text scrolls.
    val where = if (!scrolled && place.pages > 0) Strings.readerPageOf(context, place.page + 1, place.pages) else state.view?.let { chapterWords(context, it.chapter) }.orEmpty()
    Box(
        modifier.fillMaxSize().semantics {
            if (where.isNotEmpty()) contentDescription = where
            customActions = listOf(
                CustomAccessibilityAction(next) { turn(true, TurnHow.KEY); true },
                CustomAccessibilityAction(previous) { turn(false, TurnHow.KEY); true },
                CustomAccessibilityAction(showControls) { controls(); true },
            )
        },
    ) {
        // A new host brings a new view: the old one is not reused.
        key(host) { AndroidView(factory = { host.view }, modifier = Modifier.fillMaxSize()) }
    }
}

/** The chapter open and the last place its paginator reported, for a host that takes over. */
private class LastPlace {
    var chapter: Any? = null
    var pos: NovelPos? = null
}
