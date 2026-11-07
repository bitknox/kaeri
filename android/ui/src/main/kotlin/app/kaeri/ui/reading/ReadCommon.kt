// What reading's places share: a book's cover, a series' cover card with its silk track, the
// shelves' heading, the empty place with no picture, and the words for lists, actions and days.

package app.kaeri.ui.reading

import android.content.Context
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
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
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalNavigator
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
internal fun ReadCover(url: String?, width: Dp, modifier: Modifier = Modifier, shape: androidx.compose.ui.graphics.Shape = coverShape(width)) {
    ArtworkImage(
        url = url,
        plate = null,
        maxPixelSize = pixels(width * 1.5f),
        modifier = modifier.size(width, width * 1.5f).clip(shape),
    )
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
) {
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    val colors = MaterialTheme.colorScheme
    val line = tile.line(context)
    val newWord = stringResource(R.string.poster_new)
    Column(
        modifier
            .width(width)
            .sharedWork(Shared.Page, tile.work, from)
            .semantics(mergeDescendants = true) {
                // The water dot is colour alone; the state says it too.
                if (tile.newChapter) stateDescription = newWord
            }
            .combinedClickable(
                onClickLabel = stringResource(R.string.title_open),
                onLongClickLabel = onLongClick?.let { stringResource(R.string.action_more) },
                onLongClick = onLongClick?.let { more -> { haptics.performHapticFeedback(HapticFeedbackType.LongPress); more() } },
                onClick = { navigator.openSeries(tile.work, from) },
            ),
    ) {
        ReadCover(tile.cover, width, cover.sharedWork(Shared.Cover, tile.work, from))
        if (track) {
            // The track holds its place on every Continue cover, so the titles line up.
            Box(Modifier.padding(top = 6.dp).height(3.dp).fillMaxWidth()) {
                val progress = tile.progress
                SilkTrack(progress ?: 0f)
            }
        }
        Text(
            tile.title,
            style = ShowTitle.caption,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(top = if (track) 6.dp else 8.dp),
        )
        if (line.isNotEmpty()) {
            Row(Modifier.padding(top = 2.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                if (tile.newChapter) WaterDot()
                Text(
                    line,
                    style = MaterialTheme.typography.bodySmall.tabular(),
                    color = if (tile.newChapter) colors.onSurface else colors.onSurfaceVariant,
                    maxLines = lines,
                    overflow = TextOverflow.Ellipsis,
                )
            }
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
    onAction: () -> Unit = {},
) {
    val colors = MaterialTheme.colorScheme
    Column(modifier.fillMaxWidth().padding(horizontal = inset, vertical = 8.dp)) {
        Text(title, style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
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

/** The line under a cover: the engine's words for where the person is, else its facts joined. */
internal fun WorkTile.line(context: Context): String =
    next?.text(context) ?: facts.joinToString(" · ") { it.text(context) } // i18n-skip

/** The facts of a series as one line: "Manga · Japan · Releasing". */
internal fun WorkTile.factsLine(context: Context): String = facts.joinToString(" · ") { it.text(context) } // i18n-skip

/** A main action's words, in the unit the series reads in, or `null` where there is nothing to read. */
internal fun actionWords(context: Context, action: SeriesAction?): String? = when (action) {
    is SeriesAction.Read -> Strings.actionReadChapter(context, action.label)
    is SeriesAction.Continue -> Strings.actionContinueChapter(context, action.label, action.page.toInt())
    // A part goes by the source's own name for it where it has one, as its row does.
    is SeriesAction.ContinuePart -> {
        val percent = action.percent?.takeIf { it > 0u }?.toInt()
        val named = action.named
        when {
            named != null && percent != null -> Strings.actionContinueNamed(context, named, percent)
            named != null -> Strings.actionContinueNamedPlain(context, named)
            percent != null -> Strings.actionContinuePart(context, action.label, percent)
            else -> Strings.actionContinuePartPlain(context, action.label)
        }
    }
    is SeriesAction.ReadPart -> action.named?.let { Strings.actionReadNamed(context, it) } ?: Strings.actionReadPart(context, action.label)
    SeriesAction.ReadAgain -> Strings.actionReadAgain(context)
    SeriesAction.NoSource, null -> null
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
    is ReadingFailure.NotFound -> ReadingMsg.ReadingFailedNotFound
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
    // Sizes at human scale, as Send to e-reader says them: the engine words these two.
    is ReadingFailure.SendTooLarge, is ReadingFailure.SendCheck -> app.kaeri.engine.model.readingFailureMsg(failure)
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

/** A cover's long-press menu: Details, then the lists to put it on, [current] checked. */
@Composable
internal fun ReadTileMenu(tile: WorkTile, expanded: Boolean, current: LibrarySegmentRead?, onDismiss: () -> Unit) {
    val store = app.kaeri.ui.LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    app.kaeri.ui.Menu(expanded = expanded, onDismissRequest = onDismiss) {
        androidx.compose.material3.DropdownMenuItem(
            text = { Text(Strings.homeDetails(context)) },
            leadingIcon = { Icon(Icons.External, null) },
            onClick = { onDismiss(); navigator.openSeries(tile.work, "read-menu") },
        )
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
