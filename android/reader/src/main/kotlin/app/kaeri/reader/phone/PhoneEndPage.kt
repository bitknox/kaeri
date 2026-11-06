// The page between chapters on the phone: where the chapter ended and which trackers heard of it,
// what is next and how it reads, or why nothing is. It is a leaf of its own, turned to like any
// page. Continue is the one amber on it.

package app.kaeri.reader.phone

import android.content.Context
import android.net.ConnectivityManager
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.engine.model.NoNext
import app.kaeri.i18n.Strings
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.design.robotoFlex

@Composable
fun PhoneEndPage(state: ReaderState, session: ReaderSession, actions: PhoneReaderActions, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val view = state.view ?: return
    val end = state.end
    val next = end?.next ?: view.next
    val number = chapterNumber(view.chapter)
    Box(modifier.fillMaxSize().background(Chrome.ground).windowInsetsPadding(WindowInsets.safeDrawing)) {
        // Larger text scrolls the page; Continue stays pinned at the bottom.
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(start = 28.dp, end = 28.dp, top = 100.dp, bottom = 190.dp),
        ) {
            Text(Strings.readerEndOfChapter(context, number), color = Chrome.faint, fontSize = 15.sp)
            if (view.chapter.id in state.read) {
                Spacer(Modifier.height(4.dp))
                Text(trackerLine(context, state, view.chapter.id), color = Chrome.faint, fontSize = 13.sp)
            }
            Spacer(Modifier.height(32.dp))
            if (next != null) {
                Text(Strings.readerEndNext(context), color = Chrome.faint, fontSize = 13.sp)
                Spacer(Modifier.height(4.dp))
                Text(
                    next.title ?: Strings.readerChapterLine(context, chapterNumber(next)),
                    color = Chrome.text,
                    fontFamily = robotoFlex(FontWeight.Bold, 30.sp),
                    fontSize = 30.sp,
                    lineHeight = 38.sp,
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    Strings.readerEndFacts(context, chapterNumber(next), next.pages?.let { Strings.readerEndPages(context, it.toInt()) }.orEmpty()),
                    color = Chrome.faint,
                    fontSize = 14.sp,
                )
            } else {
                val why = end?.whyNone ?: NoNext.CAUGHT_UP
                val following = view.chapter.number?.toDoubleOrNull()?.let { (it.toInt() + 1).toString() } ?: ""
                val (title, line) = when (why) {
                    NoNext.NO_SOURCE_HAS_IT, NoNext.NOT_OUT_YET -> Strings.readerEndNone(context, following) to Strings.readerEndNoneLine(context)
                    NoNext.CAUGHT_UP -> Strings.readerEndCaughtUp(context) to Strings.readerEndNoneLine(context)
                    NoNext.FINISHED -> Strings.readerEndFinished(context) to Strings.readerEndFinishedLine(context)
                    NoNext.OFFLINE -> Strings.readerEndOffline(context, following) to Strings.readerEndOfflineLine(context)
                }
                Text(title, color = Chrome.text, fontFamily = robotoFlex(FontWeight.SemiBold, 22.sp), fontSize = 22.sp)
                Spacer(Modifier.height(8.dp))
                Text(line, color = Chrome.faint, fontSize = 15.sp)
            }
        }
        Column(
            Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(horizontal = 28.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Bottom,
        ) {
            if (next != null) {
                Text(Strings.readerEndSwipe(context), color = Chrome.faint, fontSize = 13.sp)
                Spacer(Modifier.height(16.dp))
                Button(
                    onClick = { session.openChapter(next.id) },
                    shape = RoundedCornerShape(25.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = Chrome.accent, contentColor = Chrome.onAccent),
                    modifier = Modifier.fillMaxWidth().height(50.dp),
                ) { Text(Strings.readerEndContinue(context), fontSize = 16.sp, fontWeight = FontWeight.SemiBold) }
                Spacer(Modifier.height(8.dp))
            }
            TextButton(onClick = actions.onSeries, modifier = Modifier.padding(bottom = 32.dp)) {
                Text(Strings.readerEndSeries(context), color = Chrome.text, fontSize = 15.sp)
            }
        }
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
