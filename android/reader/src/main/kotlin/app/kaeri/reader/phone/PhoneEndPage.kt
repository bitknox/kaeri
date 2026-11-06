// The page between chapters on the phone: where the chapter ended and which trackers heard of it,
// what is next and how it reads, or why nothing is. It is a leaf of its own, turned to like any
// page. Its one amber is Continue, or Back to the series when nothing is next.

package app.kaeri.reader.phone

import android.content.Context
import android.net.ConnectivityManager
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.NoNext
import app.kaeri.i18n.Strings
import app.kaeri.reader.PageFileView
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.design.robotoFlex

@Composable
fun PhoneEndPage(
    state: ReaderState,
    session: ReaderSession,
    actions: PhoneReaderActions,
    images: PageImages,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val view = state.view ?: return
    val end = state.end
    val next = end?.next ?: view.next
    val number = chapterNumber(view.chapter)
    Box(modifier.fillMaxSize().background(Chrome.ground).windowInsetsPadding(WindowInsets.safeDrawing)) {
        // Larger text scrolls the page; the buttons stay pinned at the bottom.
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(start = 28.dp, end = 28.dp, top = 100.dp, bottom = 190.dp),
        ) {
            Text(Strings.readerEndOfChapter(context, number), color = Chrome.faint, fontSize = 15.sp)
            if (view.chapter.id in state.read) {
                Spacer(Modifier.height(4.dp))
                Text(trackerLine(context, state, view.chapter.id), color = Chrome.faint, fontSize = 13.sp)
            }
            if (next != null) {
                Spacer(Modifier.height(34.dp))
                Text(Strings.readerEndNext(context), color = Chrome.faint, fontSize = 13.sp)
                Spacer(Modifier.height(4.dp))
                Text(
                    next.title ?: Strings.readerChapterLine(context, chapterNumber(next)),
                    color = Chrome.text,
                    fontFamily = robotoFlex(FontWeight.Bold, 30.sp),
                    fontSize = 30.sp,
                    lineHeight = 38.sp,
                )
                Spacer(Modifier.height(4.dp))
                val facts = endFacts(context, next, end?.source?.takeIf { end.next?.id == next.id })
                if (facts.isNotEmpty()) Text(facts, color = Chrome.faint, fontSize = 14.sp)
                // The next chapter's first page, once the engine made it; its plate until then. A
                // novel's chapter has no page to show.
                if (view.medium != Medium.NOVEL) {
                    Spacer(Modifier.height(22.dp))
                    PageFileView(
                        file = state.preview?.takeIf { it.chapter == next.id }?.file,
                        images = images,
                        modifier = Modifier.width(170.dp).height(242.dp),
                    ) { Box(Modifier.fillMaxSize().background(Chrome.plate)) }
                }
            } else {
                val why = end?.whyNone ?: NoNext.CAUGHT_UP
                val following = view.chapter.number?.toDoubleOrNull()?.let { (it.toInt() + 1).toString() } ?: ""
                val (title, line) = when (why) {
                    NoNext.NO_SOURCE_HAS_IT, NoNext.NOT_OUT_YET -> Strings.readerEndNone(context, following) to Strings.readerEndNoneLine(context)
                    NoNext.CAUGHT_UP -> Strings.readerEndCaughtUp(context) to Strings.readerEndNoneLine(context)
                    NoNext.FINISHED -> Strings.readerEndFinished(context) to Strings.readerEndFinishedLine(context)
                    NoNext.OFFLINE -> Strings.readerEndOffline(context, following) to Strings.readerEndOfflineLine(context)
                }
                Spacer(Modifier.height(44.dp))
                Text(title, color = Chrome.text, fontFamily = robotoFlex(FontWeight.SemiBold, 22.sp), fontSize = 22.sp, lineHeight = 28.sp)
                Spacer(Modifier.height(6.dp))
                Text(line, color = Chrome.faint, fontSize = 15.sp, lineHeight = 21.sp)
            }
        }
        // Swipe at 150 dp from the foot, the amber button at 88, the text button at 40.
        val finished = next == null && end?.whyNone == NoNext.FINISHED
        if (next != null) {
            Text(
                Strings.readerEndSwipe(context),
                color = Chrome.faint,
                fontSize = 13.sp,
                textAlign = TextAlign.Center,
                modifier = Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(start = 28.dp, end = 28.dp, bottom = 150.dp),
            )
        }
        Button(
            onClick = { if (next != null) session.openChapter(next.id) else actions.onSeries() },
            shape = RoundedCornerShape(25.dp),
            colors = ButtonDefaults.buttonColors(containerColor = Chrome.accent, contentColor = Chrome.onAccent),
            modifier = Modifier.align(Alignment.BottomCenter).padding(start = 28.dp, end = 28.dp, bottom = 88.dp).fillMaxWidth().height(50.dp),
        ) {
            Text(
                if (next != null) Strings.readerEndContinue(context) else Strings.readerEndSeries(context),
                fontSize = 16.sp,
                fontWeight = FontWeight.SemiBold,
            )
        }
        when {
            next != null -> TextButton(
                onClick = actions.onSeries,
                modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 40.dp).heightIn(min = 38.dp),
            ) { Text(Strings.readerEndSeries(context), color = Chrome.text, fontSize = 15.sp) }
            finished -> TextButton(
                onClick = session::readAgain,
                modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 40.dp).heightIn(min = 38.dp),
            ) { Text(Strings.readerEndAgain(context), color = Chrome.text, fontSize = 15.sp) }
        }
    }
}

/** "Chapter 13 · 34 pages", with "· from" its source when it comes from one. */
internal fun endFacts(context: Context, next: ChapterRow, source: String?): String {
    // The heading already names an untitled chapter, and a novel's chapter has no page count.
    val chapter = next.title?.let { chapterNumber(next) }
    val pages = next.pages?.takeIf { it > 0u }?.let { Strings.readerEndPages(context, it.toInt()) }
    val facts = when {
        chapter != null && pages != null -> Strings.readerEndFacts(context, chapter, pages)
        chapter != null -> Strings.readerChapterLine(context, chapter)
        else -> pages
    }
    return when {
        source == null -> facts.orEmpty()
        facts == null -> Strings.readerEndFrom(context, source)
        else -> Strings.readerDividerFactsSource(context, facts, source)
    }
}

/** "Read · AniList and another updated", or the offline form when the phone has no network. */
private fun trackerLine(context: Context, state: ReaderState, chapter: app.kaeri.engine.core.ChapterId): String {
    val names = state.readTrackers[chapter].orEmpty()
    if (names.isEmpty()) return Strings.readerEndRead(context)
    val manager = context.getSystemService(ConnectivityManager::class.java)
    val online = manager?.activeNetwork != null
    return if (online) {
        Strings.readerEndTracker(context, android.icu.text.ListFormatter.getInstance().format(names))
    } else {
        Strings.readerEndTrackerOffline(context, android.icu.text.ListFormatter.getInstance().format(names))
    }
}
