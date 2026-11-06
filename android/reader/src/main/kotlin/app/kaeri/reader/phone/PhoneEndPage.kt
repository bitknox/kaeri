// The page between chapters on the phone: where the chapter ended, what is next and how it reads,
// or why nothing is. It is a leaf of its own, turned to like any page.

package app.kaeri.reader.phone

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.kaeri.design.ObiSilk
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.NoNext
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReaderMode
import app.kaeri.i18n.Strings
import app.kaeri.reader.ReaderSession
import app.kaeri.reader.ReaderState
import app.kaeri.reader.rightToLeft

@Composable
fun PhoneEndPage(state: ReaderState, session: ReaderSession, actions: PhoneReaderActions, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val view = state.view ?: return
    val end = state.end
    val next = end?.next ?: view.next
    val number = chapterNumber(view.chapter)
    Column(
        modifier.fillMaxSize().background(ObiSilk.Ground).padding(horizontal = 28.dp, vertical = 72.dp),
    ) {
        Text(Strings.readerEndOfChapter(context, number), color = Chrome.faint, fontSize = 14.sp)
        Spacer(Modifier.height(6.dp))
        Text(view.chapter.title ?: view.title, color = Chrome.text, fontSize = 24.sp, fontWeight = FontWeight.Medium)
        if (view.chapter.id in state.read) {
            Spacer(Modifier.height(6.dp))
            Text(Strings.readerEndRead(context), color = Chrome.accent, fontSize = 13.sp)
        }
        Spacer(Modifier.height(24.dp))
        Box(Modifier.width(56.dp).height(2.dp).background(Chrome.accent))
        Spacer(Modifier.height(24.dp))
        if (next != null) {
            Text(Strings.readerEndNext(context), color = Chrome.faint, fontSize = 13.sp)
            Spacer(Modifier.height(4.dp))
            Text(next.title ?: Strings.readerChapterLine(context, chapterNumber(next)), color = Chrome.text, fontSize = 20.sp, fontWeight = FontWeight.Medium)
            Spacer(Modifier.height(6.dp))
            Text(
                Strings.readerEndFacts(context, chapterNumber(next), facts(context, next, view.mode, view.direction)),
                color = Chrome.faint,
                fontSize = 14.sp,
            )
            Spacer(Modifier.height(28.dp))
            Button(Strings.readerEndContinue(context), solid = true) { session.openChapter(next.id) }
            Spacer(Modifier.height(12.dp))
            Button(Strings.readerEndSeries(context), solid = false, onClick = actions.onSeries)
            Spacer(Modifier.weight(1f))
            Text(
                Strings.readerEndSwipe(context),
                color = Chrome.faint,
                fontSize = 13.sp,
                modifier = Modifier.align(Alignment.CenterHorizontally),
            )
        } else {
            val why = end?.whyNone ?: NoNext.CAUGHT_UP
            val following = view.chapter.number?.toDoubleOrNull()?.let { (it.toInt() + 1).toString() }
            // The tracker line the board drew under these is left out: the reader has no tracker
            // to name here.
            val (title, line) = when (why) {
                NoNext.NO_SOURCE_HAS_IT -> (following?.let { Strings.einkNextMissing(context, it) } ?: Strings.einkCaughtUp(context)) to Strings.einkChecksAgain(context)
                NoNext.NOT_OUT_YET -> Strings.einkNextNotOut(context) to Strings.einkChecksAgain(context)
                NoNext.CAUGHT_UP -> Strings.einkCaughtUp(context) to Strings.einkChecksAgain(context)
                NoNext.FINISHED -> Strings.einkFinished(context) to Strings.einkFinishedLine(context)
                NoNext.OFFLINE -> Strings.einkNextOffline(context) to Strings.einkNextOfflineLine(context)
            }
            Text(title, color = Chrome.text, fontSize = 22.sp, fontWeight = FontWeight.Medium)
            Spacer(Modifier.height(8.dp))
            Text(line, color = Chrome.faint, fontSize = 15.sp)
            Spacer(Modifier.height(28.dp))
            if (why == NoNext.OFFLINE) {
                Button(Strings.einkOpenDownloads(context), solid = true, onClick = actions.onDownloads)
                Spacer(Modifier.height(12.dp))
                Button(Strings.readerEndSeries(context), solid = false, onClick = actions.onSeries)
            } else {
                Button(Strings.readerEndSeries(context), solid = true, onClick = actions.onSeries)
            }
        }
    }
}

@Composable
private fun Button(label: String, solid: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(14.dp)
    Box(
        Modifier
            .fillMaxWidth()
            .clip(shape)
            .then(if (solid) Modifier.background(Chrome.accent) else Modifier.border(1.dp, Chrome.rule, shape))
            .clickable(onClick = onClick)
            .padding(vertical = 14.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, color = if (solid) ObiSilk.Ground else Chrome.text, fontSize = 16.sp, fontWeight = FontWeight.Medium)
    }
}

private fun facts(context: android.content.Context, next: ChapterRow, mode: ReaderMode, direction: ReadDirection): String = listOfNotNull(
    next.pages?.let { Strings.einkEndPages(context, it.toInt()) },
    when {
        mode == ReaderMode.STRIP -> Strings.einkReadsStrip(context)
        direction.rightToLeft -> Strings.einkReadsRtl(context)
        else -> Strings.einkReadsLtr(context)
    },
    Strings.einkOnDevice(context).takeIf { next.download is ChapterDownloadState.Done },
).joinToString(" · ")
