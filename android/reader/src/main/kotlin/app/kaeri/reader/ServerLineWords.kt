// The server line on the page between chapters, as the phone and e-ink pages share it: where the
// lines come from, whether the page holds their space, and the words.

package app.kaeri.reader

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.Dp
import kotlin.math.ceil
import androidx.compose.runtime.getValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.collectAsState
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.WorkId
import app.kaeri.i18n.Strings
import app.kaeri.store.ServerLine
import app.kaeri.store.ServerLines
import app.kaeri.store.WhyNot

/** The app's server lines, provided around the reader; none in a reader without a store. */
val LocalServerLines = staticCompositionLocalOf<ServerLines?> { null }

/** What the page shows for one chapter: whether it holds the line's space, and the line. */
class HeldLine(val held: Boolean, val line: ServerLine?, private val onRetry: () -> Unit) {
    fun retry() = onRetry()
}

/** [chapter]'s server line, read from [LocalServerLines]. */
@Composable
fun rememberServerLine(work: WorkId, chapter: ChapterId): HeldLine {
    val lines = LocalServerLines.current ?: return HeldLine(false, null) {}
    val all by lines.lines.collectAsState()
    val line = all[chapter]
    return HeldLine(line != null || lines.holds(work, chapter), line) { lines.retry(chapter) }
}

/** The line's words, without Try again. */
fun serverLineWords(context: Context, line: ServerLine): String = when (line) {
    is ServerLine.Saved -> Strings.progressServerSaved(context, line.host)
    is ServerLine.Sending -> Strings.progressServerSending(context, line.host)
    is ServerLine.Offline -> Strings.progressServerOffline(context, line.host)
    is ServerLine.Retrying -> Strings.progressServerRetrying(context, line.host)
    is ServerLine.Failed -> Strings.progressServerFailed(context, line.host, whyWords(context, line.why))
}

/** Why the server said no, as the line says it. */
fun whyWords(context: Context, why: WhyNot): String = when (why) {
    WhyNot.KEY -> Strings.progressWhyKey(context)
    WhyNot.PASSWORD -> Strings.progressWhyPassword(context)
    WhyNot.GONE -> Strings.progressWhyGone(context)
    WhyNot.GAVE_UP -> Strings.progressWhyGaveUp(context)
}

/**
 * A line's words with a link after them: on the words' last line when it fits there, as the
 * board sets Try again, else on a line of its own. [text] passes its layout to the callback.
 */
@Composable
fun TextThenLink(
    text: @Composable (onLayout: (TextLayoutResult) -> Unit) -> Unit,
    link: @Composable () -> Unit,
    gap: Dp,
    modifier: Modifier = Modifier,
) {
    val laid = remember { arrayOfNulls<TextLayoutResult>(1) }
    Layout({ text { laid[0] = it }; link() }, modifier) { measurables, constraints ->
        val loose = constraints.copy(minWidth = 0, minHeight = 0)
        val words = measurables[0].measure(loose)
        val after = measurables.getOrNull(1)?.measure(loose)
        val result = laid[0]
        if (after == null || result == null) {
            return@Layout layout(words.width, words.height) { words.place(0, 0) }
        }
        val last = result.lineCount - 1
        val x = ceil(result.getLineRight(last)).toInt() + gap.roundToPx()
        val y = result.getLineTop(last).toInt()
        val fits = !constraints.hasBoundedWidth || x + after.width <= constraints.maxWidth
        if (fits) {
            layout(maxOf(words.width, x + after.width), maxOf(words.height, y + after.height)) {
                words.place(0, 0)
                after.place(x, y)
            }
        } else {
            layout(maxOf(words.width, after.width), words.height + after.height) {
                words.place(0, 0)
                after.place(0, words.height)
            }
        }
    }
}
