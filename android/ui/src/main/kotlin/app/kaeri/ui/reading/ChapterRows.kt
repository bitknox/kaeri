// A series' chapters as rows: "Chapter 43" in silk with its title in haze on a 16/24 first line,
// its state always in words on the second, the 22 dp ring only while bytes move, and the volume
// rows that fold the chapters under them. With the words the series page and its sheets share.

package app.kaeri.ui.reading

import android.content.Context
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
import app.kaeri.design.SwapFade
import app.kaeri.design.tabular
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.BookEditionRow
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.SeriesAction
import app.kaeri.engine.model.SeriesView
import app.kaeri.store.Wording
import app.kaeri.store.AirTime
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadingMsg
import java.util.Locale
import app.kaeri.engine.model.TrackerReadRow
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.AccountsModel
import app.kaeri.store.text
import app.kaeri.ui.Icons
import app.kaeri.ui.WaterDot

/** The words the series page and its sheets say, from what the engine sends. */
internal object SeriesWords {
    /** The main action: "Read chapter 88", "Continue chapter 41 · page 8", "Continue part 5 · 34%"; null with no source. */
    fun action(context: Context, action: SeriesAction?): String? = actionWords(context, action)

    /** A tracker's name as a person writes it, reading's trackers among them. */
    fun tracker(id: String): String = when (id) {
        "mangaupdates" -> "MangaUpdates" // i18n-skip: a product name
        else -> AccountsModel.name(id)
    }

    /** "AniList and MangaUpdates": the trackers that follow the series. */
    fun trackers(rows: List<TrackerReadRow>): String = Wording.list(rows.map { tracker(it.tracker) })

    /** "87 of 88 chapters · volume 9", "Volume 5 of 5", or "Not started" while nothing is read. */
    fun progress(context: Context, row: TrackerReadRow, series: SeriesView): String {
        val volumes = row.volumes
        if (row.chapters == 0u && (volumes ?: 0u) == 0u) return Strings.seriesTrackerNotStarted(context)
        if (series.medium == Medium.NOVEL && volumes != null) {
            return Strings.seriesTrackerVolumes(context, volumes.toInt(), (series.volumesTotal ?: volumes).toInt())
        }
        val chapters = series.chaptersTotal?.let { Strings.seriesTrackerChaptersOf(context, row.chapters.toInt(), it.toInt()) }
            ?: Strings.seriesTrackerChapters(context, row.chapters.toInt())
        return if (volumes != null && volumes > 0u) Strings.seriesTrackerWithVolume(context, chapters, volumes.toInt()) else chapters
    }

    /** "books.home.example · 1 more": the source and how many others have it. The group has a row of its own. */
    fun source(context: Context, edition: BookEditionRow, others: Int): String =
        listOfNotNull(
            edition.sourceName,
            if (others > 0) Strings.seriesSourcesMore(context, others) else null,
        ).joinToString(" · ") // i18n-skip

    /** A language tag's first part, lower case: "es" of "es-419". */
    fun primary(tag: String): String = tag.split('-', '_').first().lowercase()

    /**
     * A language by its tag, in the app's language: "es" and "es-419" read "Spanish" in English
     * and "español" in Spanish. The engine's name is English, so the app names it here.
     */
    fun language(tag: String): String =
        Locale.forLanguageTag(primary(tag)).getDisplayLanguage(AirTime.locale).takeIf { it.isNotBlank() && it != primary(tag) } ?: tag

    /** One word of a chapter row's line, with a group's language named in the app's language. */
    fun text(context: Context, msg: Msg, row: ChapterRow): String {
        val reading = (msg as? Msg.Reading)?.v1
        return when (reading) {
            is ReadingMsg.ReadingRowFromGroupIn ->
                Strings.readingRowFromGroupIn(context, reading.group, row.groupLanguage?.let(::language) ?: reading.language)
            is ReadingMsg.ReadingRowPartFromGroupIn -> {
                val tag = row.partGroups.firstOrNull { it.part.toLong() == reading.part.toLong() }?.language
                Strings.readingRowPartFromGroupIn(context, reading.part.toInt(), reading.group, tag?.let(::language) ?: reading.language)
            }
            else -> msg.text(context)
        }
    }

    /** "Volume 9" and "chapters 77 to 84 · read", or "Not in a volume yet". */
    fun volume(context: Context, group: VolumeGroup, novel: Boolean): Pair<String, String> {
        val name = group.volume?.let { Strings.seriesVolume(context, it) } ?: context.getString(R.string.series_no_volume)
        val rest = mutableListOf<String>()
        group.title?.takeIf { it.isNotBlank() }?.let { rest += it }
        val numbers = group.chapters.mapNotNull { it.number?.toDoubleOrNull() }
        val low = numbers.minOrNull()
        val high = numbers.maxOrNull()
        if (!novel && low != null && high != null && low != high) rest += Strings.seriesChapterRange(context, number(low), number(high))
        if (group.allRead) rest += context.getString(R.string.series_volume_read)
        return name to rest.joinToString(" · ") // i18n-skip
    }

    private fun number(value: Double): String = if (value % 1.0 == 0.0) value.toLong().toString() else value.toString()

    /** The source's own label for a chapter: "Chapter 12", "Part 5". */
    fun name(context: Context, row: ChapterRow): String = row.label.ifBlank { row.number?.let { Strings.seriesChapterNumber(context, it) } ?: row.label }

    /** The chapter's own title, when it says more than its label. */
    fun title(row: ChapterRow, name: String): String? = row.title?.takeIf { it.isNotBlank() && it != name && it != row.label }

    /** Words of a row's line that say its parts and where it comes from, not its state. */
    private fun origin(msg: Msg): Boolean = when ((msg as? Msg.Reading)?.v1) {
        is ReadingMsg.ReadingRowParts, is ReadingMsg.ReadingRowPartsOf, is ReadingMsg.ReadingRowPartMissing,
        is ReadingMsg.ReadingRowPartFromGroup, is ReadingMsg.ReadingRowPartFromGroupIn,
        is ReadingMsg.ReadingRowFromGroup, is ReadingMsg.ReadingRowFromGroupIn, is ReadingMsg.ReadingRowFromSource,
        -> true
        else -> false
    }

    /**
     * The engine's line as its state words and its parts and origin words apart, for a new row
     * that draws the first in silk and the rest in haze. Null without both.
     */
    fun lineParts(context: Context, row: ChapterRow): Pair<String, String>? {
        val (from, state) = row.line.partition(::origin)
        if (from.isEmpty() || state.isEmpty()) return null
        return state.joinToString(" · ") { text(context, it, row) } to from.joinToString(" · ") { text(context, it, row) } // i18n-skip
    }

    /**
     * The second line in words: the engine's ("Read · 4 days ago · Downloaded"), else its state.
     * A from-line is lower case, so one that leads the line takes a capital.
     */
    fun line(context: Context, row: ChapterRow): String {
        if (row.line.isNotEmpty()) {
            return row.line.joinToString(" · ") { text(context, it, row) }.replaceFirstChar { it.titlecase(AirTime.locale) } // i18n-skip
        }
        val read = when (val state = row.state) {
            is ChapterReadState.Read -> context.getString(R.string.chapter_state_read)
            is ChapterReadState.InProgress -> state.of?.let { Strings.chapterStatePageOf(context, state.page.toInt(), it.toInt()) }
                ?: Strings.chapterStatePage(context, state.page.toInt())
            is ChapterReadState.Unread -> if (row.newChapter) context.getString(R.string.chapter_state_new) else null
        }
        val download = when (val state = row.download) {
            ChapterDownloadState.Done -> context.getString(R.string.chapter_state_downloaded)
            ChapterDownloadState.Queued -> context.getString(R.string.chapter_state_queued)
            ChapterDownloadState.WaitsForWifi -> context.getString(R.string.chapter_state_wifi)
            is ChapterDownloadState.Downloading -> Strings.chapterStateDownloading(context, state.done.toInt(), state.total.toInt())
            is ChapterDownloadState.Failed -> Strings.chapterStateFailed(context, failureWords(context, state.failure))
            null -> null
        }
        return listOfNotNull(read, download).joinToString(" · ") // i18n-skip
    }

    /** "Chapter 43 · Rust Bloom": a chapter named in a sheet's heading. */
    fun heading(context: Context, row: ChapterRow): String {
        val name = name(context, row)
        return listOfNotNull(name, title(row, name)).joinToString(" · ") // i18n-skip
    }

    /** Whether a chapter's bytes are moving now. */
    fun moving(row: ChapterRow): Boolean = row.download is ChapterDownloadState.Downloading
}

/**
 * The ring while bytes move: a silk arc on the rule with a stop square inside, 22 dp with a 2 dp
 * stroke, in a 48 dp target that stops the download.
 */
@Composable
internal fun ChapterRing(fraction: Float, label: String, state: String? = null, onStop: () -> Unit) {
    val silk = MaterialTheme.colorScheme.onSurface
    val rule = MaterialTheme.colorScheme.outlineVariant
    IconButton(
        onClick = onStop,
        modifier = Modifier.semantics {
            contentDescription = label
            if (state != null) stateDescription = state
        },
    ) {
        Canvas(Modifier.size(22.dp)) {
            val stroke = 2.dp.toPx()
            val inset = stroke / 2
            val arc = Size(size.width - stroke, size.height - stroke)
            drawArc(rule, 0f, 360f, false, Offset(inset, inset), arc, style = Stroke(stroke))
            drawArc(silk, -90f, 360f * fraction.coerceIn(0.02f, 1f), false, Offset(inset, inset), arc, style = Stroke(stroke, cap = StrokeCap.Round))
            val square = size.width * 0.3f
            drawRoundRect(
                silk,
                topLeft = Offset((size.width - square) / 2, (size.height - square) / 2),
                size = Size(square, square),
                cornerRadius = CornerRadius(1.dp.toPx()),
            )
        }
    }
}

/**
 * One chapter: its label in silk (haze once read) with its title in haze, and its state in words
 * on the second line, silk beside the water dot when it is new. [chosen] is null outside choosing;
 * while choosing, a tap ticks the row.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun ChapterRowItem(
    row: ChapterRow,
    chosen: Boolean?,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
    onStop: () -> Unit,
    onRetry: () -> Unit,
    actions: List<CustomAccessibilityAction> = emptyList(),
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val haptics = LocalHapticFeedback.current
    val read = row.state is ChapterReadState.Read
    val fresh = row.newChapter && !read
    val name = SeriesWords.name(context, row)
    val title = SeriesWords.title(row, name)
    val failed = row.download is ChapterDownloadState.Failed
    val line = SeriesWords.line(context, row)
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 72.dp)
            .combinedClickable(
                onClick = onClick,
                onLongClickLabel = stringResource(R.string.action_more),
                onLongClick = if (chosen == null) {
                    {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        onLongClick()
                    }
                } else {
                    null
                },
            )
            .semantics {
                customActions = actions
                if (chosen != null) selected = chosen
            }
            .padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f).padding(vertical = 8.dp)) {
            Text(
                buildAnnotatedString {
                    withStyle(SpanStyle(color = if (read) colors.onSurfaceVariant else colors.onSurface)) { append(name) }
                    if (title != null) {
                        append(" ")
                        withStyle(SpanStyle(color = colors.onSurfaceVariant)) { append(title) }
                    }
                },
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 2,
            )
            if (line.isNotEmpty()) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    if (fresh) WaterDot()
                    if (failed) Icon(Icons.Problem, contentDescription = null, tint = ObiSilk.Vermilion, modifier = Modifier.size(16.dp))
                    // A new row's state is silk; its parts and the group it comes from stay haze.
                    val split = if (fresh) SeriesWords.lineParts(context, row) else null
                    val words = if (split == null) {
                        AnnotatedString(line)
                    } else {
                        buildAnnotatedString {
                            append(split.first)
                            withStyle(SpanStyle(color = colors.onSurfaceVariant)) { append(" · " + split.second) } // i18n-skip: a separator
                        }
                    }
                    val colour = if (fresh || failed) colors.onSurface else colors.onSurfaceVariant
                    // A group picked changes the line: it crossfades, except on e-ink and while a
                    // download counts up.
                    if (LocalEinkLayout.current != null || row.download is ChapterDownloadState.Downloading) {
                        Text(words, style = MaterialTheme.typography.bodyMedium.tabular(), color = colour)
                    } else {
                        SwapFade(words to colour, animationSpec = Motion.fadeIn()) { (shown, tint) ->
                            Text(shown, style = MaterialTheme.typography.bodyMedium.tabular(), color = tint)
                        }
                    }
                }
            }
        }
        when {
            chosen != null -> Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) { ChooseMark(chosen) }
            row.download is ChapterDownloadState.Downloading -> {
                val moving = row.download as ChapterDownloadState.Downloading
                val fraction = if (moving.total > 0u) moving.done.toFloat() / moving.total.toFloat() else 0f
                ChapterRing(fraction, stringResource(R.string.chapter_download_stop), onStop = onStop)
            }
            failed -> FilledTonalButton(onClick = onRetry, modifier = Modifier.padding(end = 12.dp)) { Text(stringResource(R.string.chapter_retry)) }
        }
    }
}

/** The mark a chapter carries while choosing: a silk disc with a check, or an empty ring in haze. */
@Composable
internal fun ChooseMark(chosen: Boolean) {
    val colors = MaterialTheme.colorScheme
    if (chosen) {
        Box(Modifier.size(24.dp).background(colors.onSurface, CircleShape), contentAlignment = Alignment.Center) {
            Icon(Icons.Check, contentDescription = null, tint = colors.surface, modifier = Modifier.size(18.dp))
        }
    } else {
        Box(Modifier.size(24.dp).border(2.dp, colors.onSurfaceVariant, CircleShape))
    }
}

/** A volume's row: its name in medium weight, the rest in haze, and the chevron that folds it. */
@Composable
internal fun VolumeRowItem(name: String, rest: String, open: Boolean, onToggle: () -> Unit, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val state = stringResource(if (open) R.string.series_expanded else R.string.series_collapsed)
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clickable(onClick = onToggle)
            .semantics(mergeDescendants = true) {
                heading()
                stateDescription = state
            }
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            buildAnnotatedString {
                withStyle(SpanStyle(fontWeight = FontWeight.SemiBold, color = colors.onSurface)) { append(name) }
                if (rest.isNotEmpty()) {
                    withStyle(SpanStyle(color = colors.onSurfaceVariant)) { append(" · $rest") } // i18n-skip
                }
            },
            style = MaterialTheme.typography.bodyLarge,
            modifier = Modifier.weight(1f),
        )
        Icon(
            if (open) Icons.Down else Icons.Chevron,
            contentDescription = null,
            tint = colors.onSurfaceVariant,
            modifier = Modifier.size(20.dp),
        )
    }
}

/** The faint rule between rows. */
@Composable
internal fun SeriesRule(modifier: Modifier = Modifier) {
    HorizontalDivider(modifier.padding(horizontal = 16.dp), color = ObiSilk.FaintRule)
}

/** Every chapter of [groups] in reading order. */
internal fun List<VolumeGroup>.chapterRows(): List<ChapterRow> = flatMap { it.chapters }

/** How many chapters Choose chapters starts with ticked. */
internal const val CHOOSE_START_COUNT = 10

/**
 * Where Choose chapters starts: the next [CHOOSE_START_COUNT] unread chapters not downloaded yet,
 * from the reader's place on. The place is the chapter in progress, or the one after the last read
 * one. With none left after it, the first unread ones are ticked instead.
 */
internal fun List<VolumeGroup>.chooseStart(): Set<String> {
    val rows = chapterRows()
    val reading = rows.indexOfLast { it.state is ChapterReadState.InProgress }
    val from = if (reading >= 0) reading else rows.indexOfLast { it.state is ChapterReadState.Read } + 1
    val wanted = { row: ChapterRow -> row.state !is ChapterReadState.Read && row.download == null }
    val next = rows.drop(from).filter(wanted).ifEmpty { rows.filter(wanted) }
    return next.take(CHOOSE_START_COUNT).map { it.id }.toSet()
}

/** All unread, from the bar. */
internal fun List<VolumeGroup>.allUnread(): Set<String> =
    chapterRows().filter { it.state !is ChapterReadState.Read }.map { it.id }.toSet()

/** The chapters downloaded here. */
internal fun List<VolumeGroup>.downloaded(): List<ChapterRow> = chapterRows().filter { it.download is ChapterDownloadState.Done }
