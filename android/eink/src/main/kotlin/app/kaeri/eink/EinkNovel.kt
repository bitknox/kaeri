// A novel on e-ink: the engine's served chapter in the same web view the phone uses, paged, still,
// in the e-ink type settings the engine sends, with one quiet value at the foot. Each turn is a
// text page by the refresh policy; the end page, a failure and a long wait are the e-ink reader's.

package app.kaeri.eink

import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.background
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.kaeri.engine.model.EinkPaint
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderValue
import app.kaeri.i18n.Strings
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.phone.NovelPlace
import kotlinx.coroutines.delay

/** How long a novel's chapter may take to show before the wait offers a way on. */
internal const val NOVEL_WAIT_LONG_MS = 15_000L

/**
 * The novel's page and what is drawn over it: the wait until its first page shows, then pages
 * left at the foot, or the end page, or why it could not be shown. [novel] draws the web view.
 */
@Composable
internal fun BoxScope.EinkNovel(
    state: ReaderState,
    session: ReaderSession,
    place: NovelPlace,
    actions: EinkReaderActions,
    chrome: Boolean,
    paint: (EinkPaint) -> Unit,
    onControls: () -> Unit,
    novel: @Composable (place: NovelPlace, onControls: () -> Unit) -> Unit,
) {
    val context = LocalContext.current
    val view = state.view ?: return
    novel(place, onControls)

    // Each page the web view shows is one text page to the panel, and the end page its own.
    LaunchedEffect(place.ready, place.page, place.pages, place.ended) {
        when {
            place.ended -> paint(EinkPaint.EndPage)
            place.ready -> paint(EinkPaint.TextPage)
        }
    }

    // A wait that goes on offers Try again, another source and Back, as a failed open does.
    // Counted from the ask, again from each Try again.
    var tries by remember(view.chapter.id) { mutableIntStateOf(0) }
    var long by remember(view.chapter.id, tries) { mutableStateOf(false) }
    LaunchedEffect(view.chapter.id, tries) {
        delay(NOVEL_WAIT_LONG_MS)
        long = true
    }
    fun retry() {
        tries++
        if (state.novel != null) place.retry() else session.retryOpen()
    }

    val failed = place.failed
    when {
        failed != null -> Box(Modifier.fillMaxSize().background(EinkColours.Paper)) {
            EinkChapterFailed(
                line = failed,
                title = Strings.readerFailedNovel(context),
                state = state,
                session = session,
                onRetry = ::retry,
                onBack = actions.onBack,
            )
        }
        !place.ready && long -> Box(Modifier.fillMaxSize().background(EinkColours.Paper)) {
            EinkChapterFailed(
                line = Strings.readerWaitLong(context),
                state = state,
                session = session,
                onRetry = ::retry,
                onBack = actions.onBack,
                glyph = false,
            )
        }
        // Paper over the web view until its first page shows: one line, painted once.
        !place.ready -> Box(Modifier.fillMaxSize().background(EinkColours.Paper)) { EinkWaiting(state) }
        place.ended -> {
            val rtl = view.direction == ReadDirection.RIGHT_TO_LEFT
            EinkEndPage(
                state,
                actions,
                onContinue = { next -> session.openChapter(next.id) },
                // A tap on the forward side goes on, on the back side returns to the last page.
                modifier = Modifier.pointerInput(rtl) {
                    detectTapGestures { at ->
                        val x = at.x / size.width
                        when {
                            if (rtl) x < 1f / 3f else x > 2f / 3f -> session.crossOn()
                            if (rtl) x > 2f / 3f else x < 1f / 3f -> place.ended = false
                        }
                    }
                },
                onFirstPage = {
                    place.ended = false
                    place.go(0)
                },
            )
        }
        !chrome -> EinkPagesLeft(state, place)
    }
}

/** "6 pages left in this chapter", centred in the bottom margin, unless the reader turned it off. */
@Composable
private fun BoxScope.EinkPagesLeft(state: ReaderState, place: NovelPlace) {
    val foot = (state.settings?.rows?.firstOrNull { it.key == ReaderSettingKey.FOOT_VALUE }?.value as? ReaderValue.Choice)?.name
    if (foot == "none" || place.pages <= 0) return
    val left = (place.pages - 1 - place.page).coerceAtLeast(0)
    if (left == 0) return
    Text(
        Strings.readerPagesLeft(LocalContext.current, left),
        style = EinkType.Folio,
        textAlign = TextAlign.Center,
        modifier = Modifier
            .align(Alignment.BottomCenter)
            .padding(bottom = 16.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
    )
}
