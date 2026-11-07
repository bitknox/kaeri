package app.kaeri.ui.reading

import android.content.Context
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.foundation.text.appendInlineContent
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.Placeholder
import androidx.compose.ui.text.PlaceholderVerticalAlign
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.engine.model.BookServerRow
import app.kaeri.i18n.Strings
import app.kaeri.store.RowLine
import app.kaeri.store.WhyNot
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.Place
import app.kaeri.ui.RowFrame
import app.kaeri.ui.RowText

/**
 * The server's row in Your sources with a second line for progress sent back to it: saved and
 * when, sending, waiting offline, trying again, not saved with the reason, or kept on this device.
 * The row's words read as one element; Try again follows them, then the row's own trailing.
 * Try again dims to 38% on the press, and once the answer lands it crossfades out in place.
 */
@Composable
internal fun ServerProgressRow(
    server: BookServerRow,
    place: Place,
    title: String,
    value: String?,
    note: String? = null,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
) {
    val store = LocalAppStore.current
    val lines by store.rowLines.lines.collectAsStateWithLifecycle()
    val pressing by store.rowLines.retrying.collectAsStateWithLifecycle()
    val line = lines[server.id]
    val pressed = server.id in pressing
    val retry = pressed || line?.canRetry == true
    val tryAgain: @Composable (Modifier, Dp) -> Unit = { modifier, top ->
        AnimatedContent(
            targetState = retry,
            transitionSpec = { fadeIn(Motion.reveal()) togetherWith fadeOut(Motion.reveal()) },
            contentAlignment = Alignment.CenterStart,
            label = "server progress try again",
            modifier = modifier,
        ) { show ->
            if (show) {
                FilledTonalButton(
                    onClick = { store.rowLines.retry(server.id) },
                    enabled = !pressed,
                    modifier = Modifier.padding(top = top).alpha(if (pressed) 0.38f else 1f),
                    colors = androidx.compose.material3.ButtonDefaults.filledTonalButtonColors(
                        disabledContainerColor = MaterialTheme.colorScheme.secondaryContainer,
                        disabledContentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                    ),
                ) { Text(Strings.readerTryAgain(LocalContext.current)) }
            }
        }
    }
    RowFrame(place, onClick = null, modifier = Modifier.heightIn(min = 72.dp)) { selected ->
        BoxWithConstraints {
            // A phone keeps the words their width: Try again goes under them, the row's menu stays.
            val beside = maxWidth >= WIDE_ROW
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                leading?.invoke()
                Column(Modifier.weight(1f)) {
                    Column(Modifier.semantics(mergeDescendants = true) {}) {
                        RowText(title, value, note, selected)
                        AnimatedContent(
                            targetState = line,
                            transitionSpec = { fadeIn(Motion.reveal()) togetherWith fadeOut(Motion.reveal()) },
                            contentAlignment = Alignment.TopStart,
                            label = "server progress line",
                            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
                        ) { shown -> if (shown != null) RowProgressText(shown) }
                    }
                    if (!beside) tryAgain(Modifier, 8.dp)
                }
                if (beside) tryAgain(Modifier, 0.dp)
                trailing?.invoke()
            }
        }
    }
}

/** Below this the row's words would be squeezed by Try again beside them. */
private val WIDE_ROW = 480.dp

/** The row's progress words in bodyMedium: a small spinner before Sending, the warn glyph before not saved. */
@Composable
private fun RowProgressText(line: RowLine) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val words = rowProgressWords(context, line)
    val style = MaterialTheme.typography.bodyMedium
    when (line) {
        is RowLine.Sending -> Row(verticalAlignment = Alignment.CenterVertically) {
            CircularProgressIndicator(Modifier.size(11.dp), color = colors.onSurfaceVariant, strokeWidth = 1.5.dp)
            Text(words, style = style, color = colors.onSurfaceVariant, modifier = Modifier.padding(start = 6.dp))
        }
        is RowLine.Failed -> Text(
            buildAnnotatedString {
                appendInlineContent(WARN, "!")
                append(words)
            },
            style = style,
            color = colors.onSurface,
            inlineContent = mapOf(
                WARN to InlineTextContent(Placeholder(17.sp, 12.sp, PlaceholderVerticalAlign.Center)) {
                    Icon(Icons.Problem, contentDescription = null, tint = colors.error, modifier = Modifier.padding(end = 5.dp).size(12.dp))
                },
            ),
        )
        else -> Text(words, style = style, color = colors.onSurfaceVariant)
    }
}

private const val WARN = "warn"

/** The row's second line in words. */
internal fun rowProgressWords(context: Context, line: RowLine): String = when (line) {
    RowLine.Off -> Strings.progressRowOff(context)
    is RowLine.Saved -> Strings.progressRowSaved(context, agoWords(context, line.at))
    is RowLine.Sending -> Strings.progressRowSending(context, line.count)
    is RowLine.Offline -> Strings.progressRowOffline(context, line.count)
    is RowLine.Retrying -> Strings.progressRowRetrying(context, line.count)
    is RowLine.Failed -> Strings.progressRowFailed(context, line.count, whyWords(context, line.why))
}

/** Why the server said no, as the line and the row both say it. */
internal fun whyWords(context: Context, why: WhyNot): String = when (why) {
    WhyNot.KEY -> Strings.progressWhyKey(context)
    WhyNot.PASSWORD -> Strings.progressWhyPassword(context)
    WhyNot.GONE -> Strings.progressWhyGone(context)
    WhyNot.GAVE_UP -> Strings.progressWhyGaveUp(context)
}
