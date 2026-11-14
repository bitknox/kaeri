// What reading's places share: a book's cover, a series' cover card with its silk track, the
// shelves' heading, the empty place with no picture, and the words for lists, actions and days.

package app.kaeri.ui.reading

import android.content.Context
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.kaeri.design.ArtShape
import app.kaeri.design.ShowTitle
import app.kaeri.design.tabular
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.ReadSearchQuery
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.ReadRowKind
import app.kaeri.engine.model.readingFailureMsg
import app.kaeri.engine.model.SeriesAction
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.AirTime
import app.kaeri.store.AppClock
import app.kaeri.store.ReadingModel
import app.kaeri.store.Wording
import app.kaeri.store.text
import app.kaeri.ui.ArtworkImage
import app.kaeri.ui.Destination
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Navigator
import app.kaeri.ui.Shared
import app.kaeri.ui.SilkMeter
import app.kaeri.ui.WaterDot
import app.kaeri.ui.pixels
import app.kaeri.ui.sharedWork
import java.time.Instant
import java.time.format.DateTimeFormatter

// Covers

/** A cover's corners: 12 on the board's covers of 100 dp and more, the shows' radius below. */
internal fun coverShape(width: Dp) = if (width >= 100.dp) RoundedCornerShape(12.dp) else ArtShape.cover(width)

/** A book's cover at 2:3 from its URL, on its plate until the art lands. */
@Composable
internal fun ReadCover(
    url: String?,
    width: Dp,
    modifier: Modifier = Modifier,
    shape: androidx.compose.ui.graphics.Shape = coverShape(width),
    /** Drawn on the plate with a book glyph when there is no art at all, so the cover still says what it is. */
    title: String? = null,
) {
    if (url == null && title != null) {
        NoArtPlate(title, width, modifier.size(width, width * 1.5f).clip(shape))
        return
    }
    ArtworkImage(
        url = url,
        plate = null,
        maxPixelSize = pixels(width * 1.5f),
        modifier = modifier.size(width, width * 1.5f).clip(shape),
    )
}

/**
 * A cover with no art: the plate with the title at the top in the show face and silk, and a book
 * glyph at the bottom, as the board draws it. Hidden from screen readers, as the caption under it
 * is read once.
 */
@Composable
private fun NoArtPlate(title: String, width: Dp, modifier: Modifier) {
    val colors = MaterialTheme.colorScheme
    Box(modifier.background(colors.surfaceContainerHighest).clearAndSetSemantics {}) {
        Column(Modifier.fillMaxSize().padding(if (width >= 100.dp) 12.dp else 8.dp)) {
            Text(
                title,
                style = ShowTitle.caption,
                color = colors.onSurface,
                maxLines = 4,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.weight(1f))
            Icon(Icons.Read, contentDescription = null, tint = colors.onSurfaceVariant, modifier = Modifier.size(20.dp))
        }
    }
}

/**
 * A series to open: its cover, the title and one line under it. [track] draws Continue's silk
 * track 6 under the cover, 3 high, while a place is being read; a waiting new chapter says so in
 * silk with the water dot instead. [cover] dresses the cover's art, for the reader's zoom.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun ReadTile(
    tile: WorkTile,
    width: Dp,
    from: String,
    modifier: Modifier = Modifier,
    track: Boolean = false,
    /** Lines the line under the title may take; a phone's grid cell wraps rather than cut. */
    lines: Int = 1,
    cover: Modifier = Modifier,
    onLongClick: (() -> Unit)? = null,
    /** Keeps [lines] lines under the title even when the line is shorter, so tiles in a row line up. */
    reserveLine: Boolean = false,
    /** Offers Add to Planning as a screen reader's custom action, as discovery's covers do. */
    planning: Boolean = false,
    /** A third line in silk under the facts: "In Home library" on a joined search result. */
    extra: String? = null,
    /** The line under the title in place of the tile's own: Search's "Manga · Japan", a hit's "2 volumes". */
    caption: String? = null,
    /** What a screen reader says for the line, the facts with commas: "Manga, Japan, releasing". */
    spoken: String? = null,
    /** A one-column list's row rather than a cover over its words: grids at the largest text. */
    list: Boolean = false,
    /** Lines the title may take, where it is not one (two from font scale 1.3): Search's two, as the shows' posters. */
    titleMax: Int? = null,
) {
    val navigator = LocalNavigator.current
    val store = app.kaeri.ui.LocalAppStore.current
    val planningWord = Strings.readTileAddPlanning(context = LocalContext.current)
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    val colors = MaterialTheme.colorScheme
    val line = caption ?: tile.line(context)
    val newWord = stringResource(R.string.poster_new)
    // Read once as the board says it: "Iron Kite Brigade. Manga, Japan, releasing. In Home library."
    val said = spokenTile(context, tile.title, (spoken ?: line).ifEmpty { null }, extra)
    val titleLines = titleLines()
    val actions = modifier
        .then(if (list) Modifier.fillMaxWidth() else Modifier.width(width))
        .sharedWork(Shared.Page, tile.work, from)
        .semantics(mergeDescendants = true) {
            contentDescription = said
            // The water dot is colour alone; the state says it too.
            if (tile.newChapter) stateDescription = newWord
            if (planning) {
                customActions = listOf(
                    CustomAccessibilityAction(planningWord) {
                        store.reading.setStatus(tile.work, LibrarySegmentRead.PLANNING)
                        true
                    },
                )
            }
        }
        .combinedClickable(
            onClickLabel = stringResource(R.string.title_open),
            onLongClickLabel = onLongClick?.let { stringResource(R.string.action_more) },
            onLongClick = onLongClick?.let { more -> { haptics.performHapticFeedback(HapticFeedbackType.LongPress); more() } },
            onClick = { navigator.openSeries(tile.work, from) },
        )
    val words: @Composable () -> Unit = {
        Text(
            tile.title,
            style = ShowTitle.caption,
            // Two lines from font scale 1.3, as Watch's shelves, so large text keeps more than a word;
            // a list row has the width for three.
            maxLines = if (list) 3 else titleMax?.coerceAtLeast(titleLines) ?: titleLines,
            minLines = if (reserveLine && !list) titleLines else 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = if (list) 0.dp else if (track) 6.dp else 8.dp).clearAndSetSemantics {},
        )
        // A grid's line keeps all its lines' room, so every caption is as tall and the rows stay even.
        val keepLines = (reserveLine || lines > 1) && !list
        if (line.isNotEmpty() || keepLines) {
            Row(Modifier.padding(top = 2.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (tile.newChapter) WaterDot()
                // A line that changes (a miss recorded while away) crossfades in place.
                androidx.compose.animation.Crossfade(line, animationSpec = app.kaeri.design.Motion.fadeIn(), label = "tile line") { words -> // i18n-skip
                    Text(
                        words,
                        style = MaterialTheme.typography.bodySmall.tabular(),
                        color = if (tile.newChapter) colors.onSurface else colors.onSurfaceVariant,
                        maxLines = if (list) 2 else lines,
                        minLines = if (keepLines) lines else 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.clearAndSetSemantics {},
                    )
                }
            }
        }
        if (extra != null) {
            // Where a source has it, in semibold silk; it wraps rather than cut.
            Text(
                extra,
                style = MaterialTheme.typography.bodySmall.copy(fontWeight = FontWeight.SemiBold),
                color = colors.onSurface,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 2.dp).clearAndSetSemantics {},
            )
        }
    }
    if (list) {
        // The largest text: a one-column row with a 60 dp cover, as the iPhone's grids become lists.
        Row(actions.heightIn(min = 48.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            ReadCover(tile.cover, LIST_COVER, cover.sharedWork(Shared.Cover, tile.work, from), title = tile.title)
            Column(Modifier.weight(1f).padding(top = 4.dp)) { words() }
        }
        return
    }
    Column(actions) {
        ReadCover(tile.cover, width, cover.sharedWork(Shared.Cover, tile.work, from), title = tile.title)
        if (track) {
            // The track holds its place on every Continue cover, so the titles line up.
            Box(Modifier.padding(top = 6.dp).height(3.dp).fillMaxWidth()) {
                val progress = tile.progress
                SilkTrack(progress ?: 0f)
            }
        }
        words()
    }
}

/** A list row's cover at the largest text: 60 wide, as the iPhone rule's. */
internal val LIST_COVER = 60.dp

/** A list row's place before its series arrives: the 60 dp plate and two bars beside it. */
@Composable
internal fun ListRowPlaceholder() {
    val density = androidx.compose.ui.platform.LocalDensity.current
    val titleLine = with(density) { ShowTitle.caption.lineHeight.toDp() }
    val line = with(density) { MaterialTheme.typography.bodySmall.lineHeight.toDp() }
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        app.kaeri.design.CoverPlate(null, Modifier.size(LIST_COVER, LIST_COVER * 1.5f).clip(coverShape(LIST_COVER)))
        Column(Modifier.weight(1f).padding(top = 4.dp)) {
            app.kaeri.ui.TextPlaceholder(0.8f, titleLine)
            app.kaeri.ui.TextPlaceholder(0.5f, line, Modifier.padding(top = 2.dp))
        }
    }
}

/** Continue's track: silk on the rule, 3 high, as the board draws it under a cover. */
@Composable
internal fun SilkTrack(fraction: Float, modifier: Modifier = Modifier) {
    androidx.compose.material3.LinearProgressIndicator(
        progress = { fraction.coerceIn(0f, 1f) },
        modifier = modifier.fillMaxWidth().height(3.dp),
        color = MaterialTheme.colorScheme.onSurface,
        trackColor = MaterialTheme.colorScheme.outlineVariant,
        gapSize = 0.dp,
        drawStopIndicator = {},
    )
}

// Headings and empty places

/** A shelf's heading, 48 high, with its link at the end in silk. */
@Composable
internal fun ReadShelfHeading(title: String, link: String? = null, onLink: () -> Unit = {}) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            title,
            style = MaterialTheme.typography.titleMedium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).semantics { heading() },
        )
        if (link != null) {
            TextButton(
                onClick = onLink,
                modifier = Modifier.semantics { contentDescription = "$link, $title" }, // i18n-skip
                colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
            ) {
                Text(link)
                Spacer(Modifier.width(2.dp))
                Icon(Icons.Chevron, contentDescription = null, modifier = Modifier.size(18.dp))
            }
        }
    }
}

/**
 * An empty place with no picture, where the content would be: a title, a line and at most one
 * outlined button (the reading board's decision).
 */
@Composable
internal fun PlainEmpty(
    title: String,
    line: String,
    modifier: Modifier = Modifier,
    action: String? = null,
    /** The side margin: 16 in a list, none where the place already has its margins, as a grid does. */
    inset: Dp = 16.dp,
    /** The title at 16 semibold, for a state inside a page of results rather than the whole page. */
    small: Boolean = false,
    onAction: () -> Unit = {},
) {
    val colors = MaterialTheme.colorScheme
    val titleStyle = if (small) MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold) else MaterialTheme.typography.titleLarge
    Column(modifier.fillMaxWidth().padding(horizontal = inset, vertical = 8.dp)) {
        Text(title, style = titleStyle, modifier = Modifier.semantics { heading() })
        Text(
            line,
            style = MaterialTheme.typography.bodyMedium,
            color = colors.onSurfaceVariant,
            modifier = Modifier.padding(top = 8.dp).widthIn(max = 480.dp),
        )
        if (action != null) {
            OutlinedButton(
                onClick = onAction,
                modifier = Modifier.padding(top = 16.dp),
                colors = ButtonDefaults.outlinedButtonColors(contentColor = colors.onSurface),
            ) { Text(action) }
        }
    }
}

// Words

/** Reading's lists in the board's order: AniList's six, then Your files. */
internal val READ_LISTS = listOf(
    LibrarySegmentRead.READING,
    LibrarySegmentRead.PLANNING,
    LibrarySegmentRead.COMPLETED,
    LibrarySegmentRead.PAUSED,
    LibrarySegmentRead.DROPPED,
    LibrarySegmentRead.REREADING,
    LibrarySegmentRead.YOUR_FILES,
)

/** The lists a series can be put on. */
internal val READ_STATUSES = READ_LISTS - LibrarySegmentRead.YOUR_FILES

internal fun listWords(context: Context, segment: LibrarySegmentRead): String = when (segment) {
    LibrarySegmentRead.READING -> Strings.readListReading(context)
    LibrarySegmentRead.PLANNING -> Strings.readListPlanning(context)
    LibrarySegmentRead.COMPLETED -> Strings.readListCompleted(context)
    LibrarySegmentRead.PAUSED -> Strings.readListPaused(context)
    LibrarySegmentRead.DROPPED -> Strings.readListDropped(context)
    LibrarySegmentRead.REREADING -> Strings.readListRereading(context)
    LibrarySegmentRead.YOUR_FILES -> Strings.readListYourFiles(context)
}

/** Title lines under a cover: two from font scale 1.3, as Watch's shelves, else one. */
@Composable
internal fun titleLines(): Int = if (androidx.compose.ui.platform.LocalDensity.current.fontScale >= 1.3f) 2 else 1

/** Whether grids give way to one-column lists: at the largest text, as the iPhone rule has it. */
@Composable
internal fun listInsteadOfGrid(): Boolean = androidx.compose.ui.platform.LocalDensity.current.fontScale >= 2f

/** A tile as a screen reader says it: "Iron Kite Brigade. Manga, Japan, releasing. In Home library." */
internal fun spokenTile(context: Context, title: String, line: String?, extra: String?): String {
    // A title that ends in a full stop takes no second one.
    val said = title.trimEnd().removeSuffix(".")
    return when {
        line == null -> Strings.readTileSpokenTitle(context, said)
        extra == null -> Strings.readTileSpoken(context, said, line)
        else -> Strings.readTileSpokenIn(context, said, line, extra)
    }
}

/** What a discovery row's caption is made of: the row picks two of them. */
private val Msg.reading: ReadingMsg? get() = (this as? Msg.Reading)?.v1

private val WorkTile.status: Msg?
    get() = facts.firstOrNull {
        when (it.reading) {
            ReadingMsg.ReadingStatusReleasing, ReadingMsg.ReadingStatusFinished, ReadingMsg.ReadingStatusHiatus,
            ReadingMsg.ReadingStatusCancelled, ReadingMsg.ReadingStatusNotYet -> true
            else -> false
        }
    }

private val WorkTile.count: Msg?
    get() = facts.firstOrNull { it.reading is ReadingMsg.ReadingChapters || it.reading is ReadingMsg.ReadingVolumes }

/** A row tile's caption, shown and spoken. */
internal data class RowCaption(val shown: String, val spoken: String)

/**
 * A discovery tile's caption, picked by its row, never more than two facts: a row of one format
 * already names it, so it says the origin and the status ("Japan · Releasing"), the origin and
 * the length on Finished ("Japan · 112 chapters") and the origin and the start on New ("Japan ·
 * since March"); a mixed row (Because, a genre) says the format and the status. The spoken form
 * adds what the row names: "Manga, Japan, releasing".
 */
internal fun WorkTile.rowCaption(context: Context, kind: ReadRowKind, oneFormat: Boolean): RowCaption {
    if (notInSources) return Strings.homeNotFound(context).let { RowCaption(it, it) }
    next?.text(context)?.let { return RowCaption(it, it) }
    val format = facts.firstOrNull()
    val status = status
    val second = when (kind) {
        ReadRowKind.FINISHED -> count ?: status
        // A new series by the month it started, else where it stands.
        ReadRowKind.NEW -> started ?: status
        else -> status
    }
    val shown = if (oneFormat) listOfNotNull(origin, second) else listOfNotNull(format, status)
    val spoken = if (oneFormat) listOfNotNull(format, origin, second) else listOfNotNull(format, origin, status)
    return RowCaption(
        shown.joinToString(" · ") { it.text(context) }, // i18n-skip
        spoken.joinToString(", ") { it.text(context) }, // i18n-skip
    )
}

/** The line under a cover: the engine's words for where the person is, else its facts joined. */
internal fun WorkTile.line(context: Context): String =
    if (notInSources) Strings.homeNotFound(context)
    else next?.text(context) ?: started?.let { since -> listOfNotNull(origin?.text(context), since.text(context)).joinToString(" · ") } // i18n-skip
        ?: facts.joinToString(" · ") { it.text(context) } // i18n-skip

/** An AniList result's caption: its format and where it comes from, "Light novel · Japan". */
internal fun WorkTile.anilistLine(context: Context): String =
    listOfNotNull(facts.firstOrNull()?.text(context), origin?.text(context)).joinToString(" · ") // i18n-skip

/** An AniList result's caption as a screen reader says it: "Light novel, Japan". */
internal fun WorkTile.anilistSpoken(context: Context): String =
    listOfNotNull(facts.firstOrNull()?.text(context), origin?.text(context)).joinToString(", ") // i18n-skip

/** A source hit's caption: how much the source holds, "2 volumes"; nothing when it does not say. */
internal fun WorkTile.hitLine(context: Context): String = amount?.text(context).orEmpty()

/** The facts of a series as one line: "Manga · Japan · Releasing". */
internal fun WorkTile.factsLine(context: Context): String = facts.joinToString(" · ") { it.text(context) } // i18n-skip

/**
 * The label is a chapter's number, "12", "10.5" or "12-13", which the words put "chapter" or "part"
 * before; anything else is the chapter's own title.
 */
internal fun numbered(label: String): Boolean =
    label.firstOrNull()?.let { it in '0'..'9' } == true && label.all { it in '0'..'9' || it == '.' || it == '-' }

/** A main action's words, in the unit the series reads in, or `null` where there is nothing to read. */
internal fun actionWords(context: Context, action: SeriesAction?): String? = when (action) {
    // "Read chapter 12", but a chapter with no number goes by its own title: "Read Oneshot".
    is SeriesAction.Read ->
        if (numbered(action.label)) Strings.actionReadChapter(context, action.label) else Strings.actionReadNamed(context, action.label)
    is SeriesAction.Continue ->
        if (numbered(action.label)) {
            Strings.actionContinueChapter(context, action.label, action.page.toInt())
        } else {
            Strings.actionContinueNamedPage(context, action.label, action.page.toInt())
        }
    // A part goes by the source's own name for it where it has one, as its row does.
    is SeriesAction.ContinuePart -> {
        val percent = action.percent?.takeIf { it > 0u }?.toInt()
        val named = action.named
        val unit = named?.let(::namedUnit)
        when {
            unit?.first == NamedUnit.CHAPTER && percent != null -> Strings.actionContinueChapterPercent(context, unit.second, percent)
            unit?.first == NamedUnit.CHAPTER -> Strings.actionContinueChapterPlain(context, unit.second)
            unit?.first == NamedUnit.PART && percent != null -> Strings.actionContinuePart(context, unit.second, percent)
            unit?.first == NamedUnit.PART -> Strings.actionContinuePartPlain(context, unit.second)
            named != null && percent != null -> Strings.actionContinueNamed(context, named, percent)
            named != null -> Strings.actionContinueNamedPlain(context, named)
            !numbered(action.label) && percent != null -> Strings.actionContinueNamed(context, action.label, percent)
            !numbered(action.label) -> Strings.actionContinueNamedPlain(context, action.label)
            // A bare number is a chapter, as its row and the reader call it.
            percent != null -> Strings.actionContinueChapterPercent(context, action.label, percent)
            else -> Strings.actionContinueChapterPlain(context, action.label)
        }
    }
    is SeriesAction.ReadPart -> {
        val named = action.named
        val unit = named?.let(::namedUnit)
        when {
            unit?.first == NamedUnit.CHAPTER -> Strings.actionReadChapter(context, unit.second)
            unit?.first == NamedUnit.PART -> Strings.actionReadPart(context, unit.second)
            named != null -> Strings.actionReadNamed(context, named)
            !numbered(action.label) -> Strings.actionReadNamed(context, action.label)
            else -> Strings.actionReadChapter(context, action.label)
        }
    }
    SeriesAction.ReadAgain -> Strings.actionReadAgain(context)
    SeriesAction.NoSource, null -> null
}

private enum class NamedUnit { CHAPTER, PART }

private val unitName = Regex("""^\s*(chapter|part)\s+(\S+)\s*$""", RegexOption.IGNORE_CASE)

/**
 * A source's own "Chapter 3" or "Part 3", so the action says it in the app's words and case, as
 * every other main action does; `null` for any other name, which goes as the source has it.
 */
private fun namedUnit(named: String): Pair<NamedUnit, String>? {
    val found = unitName.matchEntire(named) ?: return null
    val unit = if (found.groupValues[1].equals("chapter", ignoreCase = true)) NamedUnit.CHAPTER else NamedUnit.PART
    return unit to found.groupValues[2]
}

/** The chapter a main action opens the reader at, or `null` for one that opens no chapter. */
internal val SeriesAction.chapter: ChapterId?
    get() = when (this) {
        is SeriesAction.Read -> chapter
        is SeriesAction.Continue -> chapter
        is SeriesAction.ContinuePart -> chapter
        is SeriesAction.ReadPart -> chapter
        SeriesAction.ReadAgain, SeriesAction.NoSource -> null
    }

/** A day as Updates heads it: Today, Yesterday, else the weekday and date, "Monday 2 November". */
internal fun dayWords(context: Context, unix: Long): String {
    val zone = AppClock.zone
    val day = Instant.ofEpochSecond(unix).atZone(zone).toLocalDate()
    val today = AppClock.now().atZone(zone).toLocalDate()
    return when (day) {
        today -> Strings.chapterUpdatesToday(context)
        today.minusDays(1) -> Strings.chapterUpdatesYesterday(context)
        else -> DateTimeFormatter.ofPattern(AirTime.bestPattern("EEEEdMMMM", "EEEE d MMMM"), AirTime.locale).format(day)
    }
}

/** A time of day on the system's clock: "09:40". */
internal fun clockWords(unix: Long): String = AirTime.clock(Instant.ofEpochSecond(unix))

/** How long ago [unix] was: "12 min ago". */
internal fun agoWords(context: Context, unix: Long): String =
    Wording.ago(context, (AppClock.now().epochSecond - unix).coerceAtLeast(0))

/** A reading failure in the person's words: the engine's own message for it, as the reader says it. */
internal fun failureWords(context: Context, failure: ReadingFailure): String = when (failure) {
    is ReadingFailure.Offline -> ReadingMsg.ReadingFailedOffline
    is ReadingFailure.SourceDown -> ReadingMsg.ReadingFailedSourceDown(failure.source)
    is ReadingFailure.SourceOff -> ReadingMsg.ReadingFailedSourceOff(failure.source)
    is ReadingFailure.NotFound -> ReadingMsg.ReadingFailedNotFound
    is ReadingFailure.NoSource -> ReadingMsg.ReadingFailedNoSource
    is ReadingFailure.Blocked -> ReadingMsg.ReadingFailedBlocked(failure.source, failure.reason)
    is ReadingFailure.Parse -> ReadingMsg.ReadingFailedParse(failure.source)
    is ReadingFailure.PageFetch -> ReadingMsg.ReadingFailedPage(failure.status.toLong())
    is ReadingFailure.SignInNeeded -> ReadingMsg.ReadingFailedSignIn(failure.what)
    is ReadingFailure.FileMissing -> ReadingMsg.ReadingFailedFileMissing(failure.path)
    is ReadingFailure.Unreadable -> ReadingMsg.ReadingFailedUnreadable(failure.path, failure.format)
    is ReadingFailure.Unsupported -> ReadingMsg.ReadingFailedUnsupported(failure.extension)
    is ReadingFailure.Protected -> ReadingMsg.ReadingFailedProtected
    is ReadingFailure.DiskFull -> ReadingMsg.ReadingFailedDiskFull
    is ReadingFailure.WebViewTooOld -> ReadingMsg.ReadingFailedWebview(failure.version.toLong())
    is ReadingFailure.NeedsNewerKaeri -> ReadingMsg.ReadingFailedNeedsNewer(failure.world)
    is ReadingFailure.ReadingOff -> ReadingMsg.ReadingFailedOff
    is ReadingFailure.Other -> ReadingMsg.ReadingFailedOther(failure.reason)
    is ReadingFailure.SendTooLarge, is ReadingFailure.SendCheck, is ReadingFailure.CannotWrite ->
        readingFailureMsg(failure)
}.text(context)

// Between places

/**
 * Where Library and Search stand on reading's side, so a link from Home can open them at a list or
 * a search: the scope each shows with Both, and Library's list. Process-wide, as the bar's places are.
 */
internal object ReadPlaces {
    /** Library shows reading rather than shows (with Both). */
    var libraryReading by mutableStateOf(false)

    /** Search runs reading's search rather than shows' (with Both). */
    var searchReading by mutableStateOf(false)

    /** The reading list Library shows. */
    var list by mutableStateOf(LibrarySegmentRead.READING)

    /** Library at [segment] on reading's side. */
    fun showList(segment: LibrarySegmentRead) {
        list = segment
        libraryReading = true
    }

    /**
     * Switch to Library at [segment], at its start: a series page left open on Library last time
     * is closed, so the list shows.
     */
    fun openList(navigator: Navigator, segment: LibrarySegmentRead) {
        showList(segment)
        navigator.goToStart(Destination.Library)
    }

    /** Search on reading's side, running [query]. */
    fun search(reading: ReadingModel, query: ReadSearchQuery) {
        searchReading = true
        reading.runSearch(query)
    }
}

/**
 * Shows or Reading, with Both: a segmented control with words that sits first on Library and
 * Search, each remembering its own.
 */
@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
internal fun ScopeControl(reading: Boolean, modifier: Modifier = Modifier, onChange: (Boolean) -> Unit) {
    val context = LocalContext.current
    val label = Strings.libraryScopeLabel(context)
    androidx.compose.material3.SingleChoiceSegmentedButtonRow(
        modifier.fillMaxWidth().semantics { contentDescription = label },
    ) {
        val choices = listOf(false to Strings.libraryScopeShows(context), true to Strings.libraryScopeReading(context))
        choices.forEachIndexed { index, (value, words) ->
            SegmentedButton(
                selected = reading == value,
                onClick = { onChange(value) },
                shape = androidx.compose.material3.SegmentedButtonDefaults.itemShape(index, choices.size),
                label = { Text(words, maxLines = 1) },
            )
        }
    }
}

/**
 * A cover's long-press menu: Details, then the lists to put it on, [current] checked. Without
 * [lists] it is Details alone, for covers of the person's own (Up next, This week).
 */
@Composable
internal fun ReadTileMenu(tile: WorkTile, expanded: Boolean, current: LibrarySegmentRead?, lists: Boolean = true, onDismiss: () -> Unit) {
    val store = app.kaeri.ui.LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    app.kaeri.ui.Menu(expanded = expanded, onDismissRequest = onDismiss) {
        androidx.compose.material3.DropdownMenuItem(
            text = { Text(Strings.homeDetails(context)) },
            leadingIcon = { Icon(Icons.External, null) },
            onClick = { onDismiss(); navigator.openSeries(tile.work, "read-menu") },
        )
        if (!lists) return@Menu
        androidx.compose.material3.HorizontalDivider(Modifier.padding(vertical = 8.dp), color = MaterialTheme.colorScheme.outlineVariant)
        for (list in READ_STATUSES) {
            androidx.compose.material3.DropdownMenuItem(
                text = { Text(listWords(context, list)) },
                leadingIcon = {
                    Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                        if (list == current) Icon(Icons.Check, contentDescription = stringResource(R.string.library_sort_current))
                    }
                },
                onClick = {
                    onDismiss()
                    if (list != current) store.reading.setStatus(tile.work, list)
                },
            )
        }
    }
}
