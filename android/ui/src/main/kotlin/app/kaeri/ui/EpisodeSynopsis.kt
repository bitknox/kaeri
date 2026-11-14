// The Netflix-style episode row on a phone's title page (the synopsis board, section 3): a still,
// title and facts, with up to three lines of synopsis under them. A row's height comes from its
// kind and the text's outlook, never from whether this row's text has arrived, so late text fades
// into room that was already there. Spoiler protection shows facts and a blurred still in a short
// row; a reveal covers the still and the text together for the visit. Long press opens a bottom
// sheet with the whole synopsis above the actions. Shows of more than 100 episodes split into
// ranges of 100 (the engine's `episodeRanges`).

package app.kaeri.ui

import android.content.Context
import android.view.textclassifier.TextClassificationManager
import android.view.textclassifier.TextLanguage
import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.combinedClickable
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.intl.LocaleList
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.Reveals
import app.kaeri.design.rememberTagged
import app.kaeri.design.tabular
import app.kaeri.engine.ffi.episodeRanges
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.EpisodeRange
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.EpisodeText
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.engine.model.DownloadState
import app.kaeri.store.AppClock
import androidx.compose.foundation.layout.fillMaxSize
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** The measures of the phone row, in dp, from the board. */
internal object RowDims {
    val StillWidth = 144.dp
    val StillHeight = 81.dp
    val Top = 10.dp
    val Gap = 12.dp
    val BandGap = 8.dp
    val BottomTall = 13.dp
    val BottomShort = 9.dp
    val TitleLine = 22.sp
    val FactsLine = 16.sp
    val BodyLine = 20.sp
    const val SynopsisLines = 3

    /** From this font scale the row stacks: the still across the row, four lines of text room. */
    const val StackScale = 1.5f
    const val StackedLines = 4
}

/** The beside row a tablet uses at regular width: the text beside the still, 128 dp tall at scale 1. */
internal object BesideDims {
    val StillWidth = 192.dp
    val StillHeight = 108.dp
    val Pad = 10.dp
    val Gap = 16.dp
    val TitleLine = 22.sp
    val FactsLine = 18.sp
    val BandGap = 8.dp
    const val SynopsisLines = 2

    /** How wide a beside row grows on a wide tablet, so its text keeps a readable measure. */
    val MaxWidth = 760.dp
}

/** How tall a row is: room for the text where text can land, none elsewhere. */
internal enum class RowForm { TALL, SHORT }

/**
 * The form of a row, decided at first layout from the table's text outlook: short for a protected
 * row and for a show with no text coming, tall otherwise.
 */
internal fun rowForm(text: EpisodeText, protectedRow: Boolean, unaired: Boolean = false): RowForm =
    if (protectedRow || unaired || text == EpisodeText.NONE) RowForm.SHORT else RowForm.TALL

/** A row's fixed height from the header's and the text band's heights. 172 dp tall and 100 dp short at scale 1. */
internal fun rowHeight(form: RowForm, header: Dp, band: Dp): Dp = when (form) {
    RowForm.TALL -> RowDims.Top + header + RowDims.BandGap + band + RowDims.BottomTall
    RowForm.SHORT -> RowDims.Top + header + RowDims.BottomShort
}

/** Whether spoiler protection hides [row]: it is on, the engine says the row is ahead, and the row has aired. */
internal fun hiddenBySpoilers(row: EpisodeRow, protect: Boolean, unaired: Boolean): Boolean =
    protect && !unaired && row.spoilerAhead

/**
 * The text outlook the page was laid out with. It is read once for a table (and again for another
 * range of it), so text that turns up for a table laid out short waits for the next layout.
 */
@Composable
internal fun rememberTextOutlook(key: Any?, rows: List<EpisodeRow>?, outlook: (List<EpisodeRow>) -> EpisodeText): EpisodeText =
    remember(key, rows == null) { rows?.let(outlook) ?: EpisodeText.NONE }

/** A synopsis, in its own language. */
@Composable
internal fun SynopsisText(
    text: String,
    color: Color,
    modifier: Modifier = Modifier,
    maxLines: Int = Int.MAX_VALUE,
    minLines: Int = 1,
    style: TextStyle = MaterialTheme.typography.bodyMedium,
) {
    val annotated = rememberTagged(text)
    Text(annotated, style = style, color = color, maxLines = maxLines, minLines = minLines, overflow = TextOverflow.Ellipsis, modifier = modifier)
}

// Facts

/** The two lines under an episode's title: "E7 · 14 min left", then "Aired 21 May 2014" once it has. */
internal fun airedDate(context: Context, airsAt: Long): String =
    Strings.titleFactsAired(context, DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).format(Instant.ofEpochSecond(airsAt).atZone(ZoneId.systemDefault())))

/**
 * The facts under an episode's title, as its row and its sheet both say them: "E7 · 14 min left",
 * "E6 · Watched" or "E8 · 24 min", then "Aired 21 May 2014" once it has.
 */
internal fun rowFacts(context: Context, entry: Entry, row: EpisodeRow, current: Boolean, unaired: Boolean, withNumber: Boolean = true): Pair<String, String?> {
    val number = row.number.toDoubleOrNull()?.toInt() ?: 0
    val resume = entry.resume?.takeIf { current && !row.completed }
    val first = if (withNumber) {
        when {
            unaired -> Strings.episodeLineDetail(context, number, unairedDetail(context, row))
            row.unsourced -> Strings.episodeLineDetail(context, number, Strings.episodeUnsourced(context))
            row.completed -> Strings.episodeLineDetail(context, number, Strings.episodeWatched(context))
            else -> episodeDetail(context, row, resume = resume)
        }
    } else {
        // The title is the number already, so the facts leave "E4" out.
        when {
            unaired -> unairedDetail(context, row)
            row.unsourced -> Strings.episodeUnsourced(context)
            row.completed -> Strings.episodeWatched(context)
            else -> episodeDetailOnly(context, row, resume = resume) ?: ""
        }
    }
    val second = row.airsAt?.takeIf { !unaired && it <= AppClock.now().epochSecond }?.let { airedDate(context, it) }
    return first to second
}

private fun unairedDetail(context: Context, row: EpisodeRow): String {
    val airs = row.airsAt?.let { airsWords(context, Instant.ofEpochSecond(it)) }
    return airs?.let { Strings.episodeRowAirs(context, it) } ?: Strings.episodeRowNotOut(context)
}

// The row

/**
 * An episode on the title page: [form] says whether it keeps room for three lines of synopsis.
 * [hidden] is spoiler protection on this row: facts and a blurred still, no text. A tap plays; a
 * long press opens the sheet.
 */
@androidx.compose.foundation.ExperimentalFoundationApi
@Composable
internal fun EpisodeCard(
    entry: Entry,
    row: EpisodeRow,
    current: Boolean,
    unaired: Boolean,
    hidden: Boolean,
    form: RowForm,
    onPlay: () -> Unit,
    onMenu: () -> Unit,
    onShow: () -> Unit,
    opensService: Boolean = false,
    /** The tablet's beside row: 192 × 108 still, title, facts and three lines of text beside it. */
    beside: Boolean = false,
) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val density = LocalDensity.current
    val resume = entry.resume?.takeIf { current }
    val fraction = resume?.fraction?.toFloat()
    val marks = rememberRowMarks(row, fraction)
    val airs = row.airsAt?.takeIf { unaired }?.let { airsWords(context, Instant.ofEpochSecond(it)) }
    // A title that is only the number comes through empty; the row writes "Episode N" and leaves "E4" out of the facts.
    val titled = !row.title.isNullOrBlank()
    val (first, second) = rowFacts(context, entry, row, current, unaired, withNumber = titled || beside)
    val unairedLabel = if (unaired) {
        val number = row.number
        when {
            titled && airs != null -> Strings.episodeUnairedAirsLabel(context, number, row.title!!, airs)
            titled -> Strings.episodeUnairedLabel(context, number, row.title!!)
            airs != null -> Strings.episodeUnairedAirsLabelPlain(context, number, airs)
            else -> Strings.episodeUnairedLabelPlain(context, number)
        }
    } else {
        null
    }
    val optionsLabel = stringResource(R.string.episode_options)
    val playLabel = stringResource(R.string.action_play)
    val hiddenWord = stringResource(R.string.episode_synopsis_hidden)
    val showWord = stringResource(R.string.episode_synopsis_show)
    val stacked = density.fontScale >= RowDims.StackScale
    val header = with(density) { maxOf(RowDims.StillHeight, ((RowDims.TitleLine * 2).toDp() + (RowDims.FactsLine * 2).toDp() + 4.dp)) }
    val band = with(density) { (RowDims.BodyLine * RowDims.SynopsisLines).toDp() }
    val title = row.title?.takeIf { it.isNotBlank() } ?: app.kaeri.store.episodeName(LocalContext.current, row.number)
    // A watched row keeps a silk title; the check and the dimmed still say watched.
    val titleColor = if (unaired) colors.onSurfaceVariant else colors.onSurface
    // A row that grows on screen (a reveal) takes its height first; its text fades in after.
    val tall = form == RowForm.TALL
    var settled by remember { mutableStateOf(tall) }
    androidx.compose.runtime.LaunchedEffect(tall) {
        if (!tall) {
            settled = false
        } else if (!settled) {
            // In case the size never animates (it was already this tall), the text still comes.
            delay(600)
            settled = true
        }
    }
    val text = row.description?.takeIf { it.isNotBlank() && !hidden && settled }

    val clickable = if (unaired) {
        Modifier.semantics(mergeDescendants = true) { contentDescription = unairedLabel.orEmpty() }
    } else {
        Modifier
            .combinedClickable(
                onClickLabel = if (row.metadataOnly || row.unsourced) optionsLabel else playLabel,
                onLongClickLabel = optionsLabel,
                onClick = onPlay,
                onLongClick = onMenu,
            )
            // A keyboard opens the same sheet with its menu key, or Shift and F10.
            .onPreviewKeyEvent { event ->
                val opens = event.key == Key.Menu || (event.key == Key.F10 && event.isShiftPressed)
                if (opens && event.type == KeyEventType.KeyUp) onMenu()
                opens
            }
            .semantics {
                if (hidden) {
                    stateDescription = hiddenWord
                    customActions = listOf(CustomAccessibilityAction(showWord) { onShow(); true })
                }
            }
    }
    val grows = Modifier.animateContentSize(Motion.standard()) { _, _ -> if (tall) settled = true }
    if (beside) {
        // The text beside the still; the row keeps its height whether text can land or not.
        val besideBand = with(density) { (RowDims.BodyLine * BesideDims.SynopsisLines).toDp() }
        val besideText = with(density) { BesideDims.TitleLine.toDp() + 2.dp + BesideDims.FactsLine.toDp() + BesideDims.BandGap + besideBand }
        val height = BesideDims.Pad * 2 + maxOf(BesideDims.StillHeight, besideText)
        Row(
            Modifier
                .widthIn(max = BesideDims.MaxWidth)
                .fillMaxWidth()
                .height(height)
                .alpha(if (row.unsourced) 0.55f else 1f)
                .then(clickable)
                .padding(horizontal = 16.dp, vertical = BesideDims.Pad),
            horizontalArrangement = Arrangement.spacedBy(BesideDims.Gap),
        ) {
            Box(Modifier.size(BesideDims.StillWidth, BesideDims.StillHeight).alpha(1f - 0.3f * marks.dim.value)) {
                StillWithMark(entry, row, hidden, unaired, BesideDims.StillWidth, marks, fraction)
            }
            Column(Modifier.weight(1f)) {
                val facts = second?.let { Strings.episodeFactsJoin(context, first, it) } ?: first
                RowHeader(
                    title, titleColor, facts, null, marks, titleLines = 1, modifier = Modifier.fillMaxWidth(), opensService = opensService,
                    titleLine = BesideDims.TitleLine, factsLine = BesideDims.FactsLine,
                )
                if (tall) {
                    // Text that turns up later fades in; a revealed row's fades in once it shows.
                    val shown = row.description?.takeIf { it.isNotBlank() && !hidden }
                    SynopsisBand(shown, Modifier.padding(top = BesideDims.BandGap).fillMaxWidth().height(besideBand), maxLines = BesideDims.SynopsisLines)
                }
            }
        }
        return
    }
    val fixed = if (stacked) Modifier else Modifier.height(rowHeight(form, header, band))
    Column(
        Modifier
            .fillMaxWidth()
            .then(grows)
            .then(fixed)
            .alpha(if (row.unsourced) 0.55f else 1f)
            .then(clickable)
            .padding(start = 16.dp, end = 16.dp, top = RowDims.Top),
    ) {
        if (stacked) {
            Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f).alpha(1f - 0.3f * marks.dim.value)) {
                StillWithMark(entry, row, hidden, unaired, 360.dp, marks, fraction)
            }
            Spacer(Modifier.height(8.dp))
            RowHeader(title, titleColor, first, second, marks, titleLines = 2, modifier = Modifier.fillMaxWidth(), opensService = opensService)
            if (tall) {
                // Four lines of room whether the text is here yet or not, so late text grows nothing.
                val room = with(density) { (RowDims.BodyLine * RowDims.StackedLines).toDp() }
                SynopsisBand(text, Modifier.padding(top = RowDims.BandGap).fillMaxWidth().height(room), maxLines = RowDims.StackedLines)
            }
            Spacer(Modifier.height(8.dp))
        } else {
            Row(horizontalArrangement = Arrangement.spacedBy(RowDims.Gap)) {
                Box(Modifier.size(RowDims.StillWidth, RowDims.StillHeight).alpha(1f - 0.3f * marks.dim.value)) {
                    StillWithMark(entry, row, hidden, unaired, RowDims.StillWidth, marks, fraction)
                }
                RowHeader(title, titleColor, first, second, marks, titleLines = 2, modifier = Modifier.weight(1f).padding(top = 2.dp), opensService = opensService)
            }
            if (tall) {
                SynopsisBand(text, Modifier.padding(top = RowDims.BandGap).fillMaxWidth().height(band), maxLines = RowDims.SynopsisLines)
            }
        }
    }
}

@Composable
private fun StillWithMark(entry: Entry, row: EpisodeRow, hidden: Boolean, unaired: Boolean, width: Dp, marks: RowMarks, fraction: Float?) {
    val colors = MaterialTheme.colorScheme
    Box(Modifier.fillMaxSize()) {
        EpisodeStill(entry, row, hidden = hidden, unaired = unaired, width = width, pixels = 400)
        val showsBar = (fraction != null || marks.fill.isRunning) && !unaired && marks.dim.value < 1f
        if (showsBar) {
            Box(
                Modifier
                    .align(Alignment.BottomStart)
                    .padding(start = 6.dp, end = 6.dp, bottom = 6.dp)
                    .fillMaxWidth()
                    .height(3.dp)
                    .clip(RoundedCornerShape(2.dp))
                    .background(colors.background.copy(alpha = 0.7f)),
            ) {
                Box(Modifier.fillMaxWidth(marks.fill.value.coerceIn(0f, 1f)).height(3.dp).background(colors.onSurface))
            }
        }
    }
}

@Composable
private fun RowHeader(
    title: String,
    titleColor: Color,
    first: String,
    second: String?,
    marks: RowMarks,
    titleLines: Int,
    modifier: Modifier,
    opensService: Boolean,
    titleLine: androidx.compose.ui.unit.TextUnit = RowDims.TitleLine,
    factsLine: androidx.compose.ui.unit.TextUnit = RowDims.FactsLine,
) {
    val colors = MaterialTheme.colorScheme
    Column(modifier) {
        Row(verticalAlignment = Alignment.Top) {
            Text(
                title,
                style = MaterialTheme.typography.titleMedium.copy(lineHeight = titleLine),
                color = titleColor,
                maxLines = titleLines,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            if (opensService) {
                Box(Modifier.size(20.dp), contentAlignment = Alignment.Center) {
                    Icon(Icons.External, contentDescription = stringResource(R.string.licensed_opens_service), tint = colors.onSurfaceVariant, modifier = Modifier.size(16.dp))
                }
            }
        }
        // A watched row's check draws itself on at the head of its facts: "✓ E6 · Watched".
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 2.dp)) {
            if (marks.check.value > 0f) {
                DrawnCheck(marks.check.value, colors.onSurfaceVariant, 14.dp, described = false)
                Spacer(Modifier.width(4.dp))
            }
            if (first.isNotEmpty()) Text(first, style = MaterialTheme.typography.bodySmall.tabular().copy(lineHeight = factsLine), color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        if (second != null) {
            Text(second, style = MaterialTheme.typography.bodySmall.copy(lineHeight = factsLine), color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

/**
 * The room for text, kept whether [text] is here or not: the synopsis fades in on the fade-in
 * token, and replaced text crossfades, never through empty.
 */
@Composable
private fun SynopsisBand(text: String?, modifier: Modifier, maxLines: Int) {
    Box(modifier) {
        Crossfade(text, animationSpec = Motion.fadeIn(), label = "synopsis") { shown -> // i18n-skip
            if (shown != null) {
                SynopsisText(shown, MaterialTheme.colorScheme.onSurfaceVariant, maxLines = maxLines)
            }
        }
    }
}

// The line where protection starts

/**
 * "Hidden after episode 7" with Show, over the first protected row. Once Show has revealed the
 * rows after it, the line stays and says "Showing after episode 7" with Hide; words and button
 * crossfade on the reveal token, so nothing under the line moves.
 */
@Composable
internal fun ProtectionLine(episode: Int, showing: Boolean, onShow: () -> Unit, onHide: () -> Unit, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val showLabel = stringResource(R.string.episode_synopsis_show)
    val hideLabel = stringResource(R.string.episode_protection_hide)
    Crossfade(showing, modifier, animationSpec = Motion.reveal(), label = "protection-line") { shown -> // i18n-skip
        Row(
            Modifier.fillMaxWidth().height(44.dp).padding(start = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(if (shown) Icons.Shown else Icons.Hidden, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(16.dp))
            Text(
                if (shown) Strings.episodeProtectionShownShort(context, episode) else Strings.episodeProtectionLineShort(context, episode),
                style = MaterialTheme.typography.bodySmall,
                color = colors.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            Box(
                Modifier
                    .height(44.dp)
                    .clickable(role = Role.Button, onClickLabel = if (shown) hideLabel else showLabel, onClick = if (shown) onHide else onShow)
                    .padding(horizontal = 16.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    stringResource(if (shown) R.string.episode_protection_hide else R.string.episode_protection_show),
                    style = MaterialTheme.typography.labelLarge,
                    color = colors.onSurface,
                )
            }
        }
    }
}

/** The episode the protection line names: the row before the first one it hides, as the engine marks them. */
internal fun protectionStart(listed: List<EpisodeRow>, firstHidden: EpisodeRow): Int? {
    val at = listed.indexOfFirst { it.number == firstHidden.number }
    return listed.getOrNull(at - 1)?.number?.toDoubleOrNull()?.toInt()
}

// Ranges

/** A range of a long series as the menu names it: the first and last episode numbers in it. */
internal fun rangeBounds(rows: List<EpisodeRow>, range: EpisodeRange): Pair<Int, Int> {
    val first = rows.getOrNull(range.start.toInt())?.number?.toDoubleOrNull()?.toInt() ?: (range.start.toInt() + 1)
    val last = rows.getOrNull(range.end.toInt() - 1)?.number?.toDoubleOrNull()?.toInt() ?: range.end.toInt()
    return first to last
}

/** The rows a range shows. */
internal fun rangeRows(rows: List<EpisodeRow>, range: EpisodeRange): List<EpisodeRow> =
    rows.subList(range.start.toInt().coerceIn(0, rows.size), range.end.toInt().coerceIn(0, rows.size))

/** The ranges of [rows], none for a series of 100 or fewer. */
internal fun rangesOf(rows: List<EpisodeRow>, now: Long): List<EpisodeRange> = episodeRanges(rows, now)

/** The range menu beside the dub line, over the list; each item says Watched or Not out yet. */
@Composable
internal fun RangeRow(
    rows: List<EpisodeRow>,
    ranges: List<EpisodeRange>,
    selected: Int,
    onPick: (Int) -> Unit,
    dub: String?,
    modifier: Modifier = Modifier,
    /** Go to episode, the menu's last item: opens its number field. */
    onGoTo: (() -> Unit)? = null,
) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    var open by remember { mutableStateOf(false) }
    // The picker says the short range, "101–200"; the menu's items say it in words.
    val label = rangeBounds(rows, ranges[selected]).let { Strings.episodeRangeShort(context, it.first.toString(), it.second.toString()) }
    val longLabel = rangeBounds(rows, ranges[selected]).let { Strings.episodeRange(context, it.first, it.second) }
    val rangeWord = stringResource(R.string.episode_range_label)
    val shape = RoundedCornerShape(8.dp)
    Row(modifier.fillMaxWidth().height(56.dp).padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
        Box {
            Row(
                Modifier
                    .height(40.dp)
                    .clip(shape)
                    .border(1.dp, colors.outlineVariant, shape)
                    .clickable(role = Role.Button, onClickLabel = rangeWord) { open = true }
                    .semantics(mergeDescendants = true) {
                        contentDescription = rangeWord
                        stateDescription = longLabel
                    }
                    .padding(start = 12.dp, end = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Crossfade(label, animationSpec = Motion.standard(), label = "range") { // i18n-skip
                    Text(it, style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurface)
                }
                Icon(Icons.Down, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(18.dp))
            }
            Menu(expanded = open, onDismissRequest = { open = false }) {
                ranges.forEachIndexed { index, range ->
                    val (from, to) = rangeBounds(rows, range)
                    DropdownMenuItem(
                        text = {
                            Column {
                                Text(Strings.episodeRange(context, from, to), style = MaterialTheme.typography.bodyLarge.tabular())
                                val state = when {
                                    range.watched -> stringResource(R.string.episode_watched)
                                    range.notOut -> stringResource(R.string.episode_row_not_out)
                                    else -> null
                                }
                                if (state != null) Text(state, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
                            }
                        },
                        trailingIcon = { if (index == selected) Icon(Icons.Check, contentDescription = null) },
                        onClick = {
                            open = false
                            onPick(index)
                        },
                    )
                }
                if (onGoTo != null) {
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.episode_go_to)) },
                        onClick = {
                            open = false
                            onGoTo()
                        },
                    )
                }
            }
        }
        Spacer(Modifier.weight(1f))
        androidx.compose.animation.AnimatedVisibility(dub != null, enter = androidx.compose.animation.fadeIn(Motion.fadeIn()), exit = androidx.compose.animation.ExitTransition.None) {
            Text(dub.orEmpty(), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, maxLines = 1)
        }
    }
}

// The sheet

/** One action of the episode sheet. */
internal data class SheetAction(
    val label: String,
    val icon: ImageVector,
    val supporting: String? = null,
    val enabled: Boolean = true,
    val onClick: () -> Unit,
)

/**
 * The episode's long-press sheet: its still, title, facts and the whole synopsis above the
 * actions as list items. A protected episode keeps the still blurred and the text out until Show
 * synopsis, its first action, is chosen; the header then updates in place before the sheet closes.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun EpisodeSheet(
    entry: Entry,
    row: EpisodeRow,
    hidden: Boolean,
    onShow: () -> Unit,
    onDismiss: () -> Unit,
    actions: AppActions,
    /** Whether it is the next episode, whose facts say how much is left. */
    current: Boolean = false,
) {
    val colors = MaterialTheme.colorScheme
    val context = LocalContext.current
    val state = rememberModalBottomSheetState(skipPartiallyExpanded = false)
    val scope = rememberCoroutineScope()
    fun close() {
        scope.launch { state.hide() }.invokeOnCompletion { onDismiss() }
    }
    val items = episodeActions(entry, row, actions, ::close)
    val title = row.title?.takeIf { it.isNotBlank() } ?: app.kaeri.store.episodeName(LocalContext.current, row.number)
    // The row's own facts: "E7 · 14 min left", then when it aired.
    val (kicker, aired) = rowFacts(context, entry, row, current, unaired = false)
    val synopsis = row.description?.takeIf { it.isNotBlank() && !hidden }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = state,
        containerColor = colors.surfaceContainer,
        scrimColor = Color.Black.copy(alpha = 0.32f),
        dragHandle = { androidx.compose.material3.BottomSheetDefaults.DragHandle(color = colors.onSurfaceVariant.copy(alpha = 0.4f)) },
    ) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(start = 20.dp, end = 20.dp, bottom = 24.dp)) {
            Row(horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(RowDims.StillWidth, RowDims.StillHeight)) {
                    EpisodeStill(entry, row, hidden = hidden, unaired = false, width = RowDims.StillWidth, pixels = 400)
                }
                Column(Modifier.weight(1f)) {
                    Text(kicker, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
                    Text(title, style = MaterialTheme.typography.titleMedium, color = colors.onSurface)
                    if (aired != null) Text(aired, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant, modifier = Modifier.padding(top = 2.dp))
                }
            }
            Crossfade(synopsis, animationSpec = Motion.fadeIn(), label = "sheet-synopsis") { shown -> // i18n-skip
                if (shown != null) SynopsisText(shown, colors.onSurfaceVariant, modifier = Modifier.padding(top = 14.dp, bottom = 4.dp))
            }
            HorizontalDivider(Modifier.padding(top = 12.dp, bottom = 4.dp), color = colors.outlineVariant)
            if (hidden) {
                SheetItem(
                    SheetAction(stringResource(R.string.episode_synopsis_show), Icons.Shown) {
                        onShow()
                        scope.launch {
                            delay(450)
                            close()
                        }
                    },
                )
            }
            items.forEach { SheetItem(it) }
        }
    }
}

@Composable
private fun SheetItem(action: SheetAction) {
    val colors = MaterialTheme.colorScheme
    val tint = if (action.enabled) colors.onSurface else colors.onSurface.copy(alpha = 0.38f)
    Row(
        Modifier
            .fillMaxWidth()
            .height(52.dp)
            .clickable(enabled = action.enabled, role = Role.Button, onClick = action.onClick),
        horizontalArrangement = Arrangement.spacedBy(18.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(action.icon, contentDescription = null, tint = if (action.enabled) colors.onSurfaceVariant else tint, modifier = Modifier.size(20.dp))
        Column {
            Text(action.label, style = MaterialTheme.typography.bodyLarge, color = tint)
            if (action.supporting != null) Text(action.supporting, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
        }
    }
}

/**
 * The episode's actions: play it, mark it watched or not, open its discussion, download it, play it
 * on a TV or from the start. One no source has, AniList's alone, offers to add a source instead of
 * downloading and playing it.
 */
@Composable
internal fun episodeActions(entry: Entry, row: EpisodeRow, actions: AppActions, close: () -> Unit): List<SheetAction> {
    val store = LocalAppStore.current
    val marks = LocalUserMarks.current
    val snackbars = LocalSnackbars.current
    val navigator = LocalNavigator.current
    val markedWatched = app.kaeri.store.wholeEpisode(row.number)?.let { stringResource(R.string.episode_marked_watched, it) }
        ?: listOf(app.kaeri.store.episodeName(LocalContext.current, row.number), stringResource(R.string.episode_watched)).joinToString(" · ") // i18n-skip
    val undoWord = stringResource(R.string.action_undo)
    val watched = row.completed
    val list = mutableListOf<SheetAction>()
    val playable = !row.unsourced && !row.metadataOnly
    if (playable) {
        list += SheetAction(stringResource(R.string.action_play), Icons.PlayRow) {
            close()
            actions.play(entry.id, row.number)
        }
    }
    list += SheetAction(stringResource(if (watched) R.string.episode_mark_unwatched else R.string.episode_mark_watched), Icons.Check) {
        close()
        marks.mark(row.number)
        store.setWatched(entry.id, listOf(row.number), watched = !watched)
        if (!watched) {
            snackbars.show(markedWatched, undoWord) {
                marks.mark(row.number)
                store.setWatched(entry.id, listOf(row.number), watched = false)
            }
        }
    }
    val comments = discussionCountOf(entry, row)
    if (comments != null) {
        list += SheetAction(
            stringResource(R.string.discussion_open),
            Icons.Discussion,
            supporting = androidx.compose.ui.res.pluralStringResource(R.plurals.discussion_count, comments.toInt(), comments.toInt()),
        ) {
            close()
            row.number.toUIntOrNull()?.let { store.discussion.show(entry.id, it) }
        }
    }
    if (row.unsourced) {
        // Nothing to play or download: marking it watched is all that is left.
    } else if (row.metadataOnly) {
        // A source that listed titles but could not choose: the question, not another source.
        val asking = store.match.state.collectAsStateWithLifecycle().value.asks(entry.id)
        list += SheetAction(stringResource(if (asking) R.string.match_choose else R.string.title_button_add_source), if (asking) Icons.Search else Icons.Providers) {
            close()
            if (asking) store.match.open(entry.id, fromMenu = false) else navigator.open(PROVIDERS_ROUTE)
        }
    } else {
        val downloads by store.downloads.rows.collectAsStateWithLifecycle()
        val queued = downloads.firstOrNull { it.anilistId == entry.id && it.episode == row.number }
        list += SheetAction(
            when (queued?.state) {
                DownloadState.DONE -> stringResource(R.string.episode_download_done)
                DownloadState.ACTIVE, DownloadState.QUEUED -> stringResource(R.string.downloads_caption_running_plain)
                DownloadState.PAUSED -> stringResource(R.string.episode_menu_download_paused)
                else -> stringResource(R.string.episode_menu_download)
            },
            Icons.Downloads,
            enabled = queued == null || queued.state == DownloadState.FAILED,
        ) {
            close()
            store.downloadEpisode(entry.id, row.number)
        }
        if (store.tv.available) {
            list += SheetAction(stringResource(R.string.tv_play_on), Icons.Tv) {
                close()
                store.tv.offer(app.kaeri.store.TvPlayRequest(entry.id, entry.showTitle, row.number))
            }
        }
        list += SheetAction(stringResource(R.string.episode_play_from_beginning), Icons.Restart) {
            close()
            actions.playFromStart(entry.id, row.number)
        }
    }
    return list
}
