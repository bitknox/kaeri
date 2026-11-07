// What the series page opens over itself: a chapter's sheet (long press), Read from, Tracking, the
// status and Download menus, the Read again question and the bar that Choose chapters brings up.

package app.kaeri.ui.reading

import android.content.Context
import androidx.compose.animation.AnimatedContent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ObiSilk
import app.kaeri.design.tabular
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.BookEditionRow
import app.kaeri.engine.model.BookServerKind
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.DownloadPick
import app.kaeri.engine.model.EditionMiss
import app.kaeri.engine.model.ListRemovalOutcome
import app.kaeri.engine.model.SeriesView
import app.kaeri.engine.model.TrackerMapping
import app.kaeri.engine.model.TrackerReadRow
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.i18n.Strings
import app.kaeri.store.text
import app.kaeri.ui.AppActions
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppSheets
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Menu
import app.kaeri.ui.QuietButton
import app.kaeri.ui.SettingsPane
import app.kaeri.ui.SettingsSheet
import app.kaeri.ui.SheetButtons
import app.kaeri.ui.SheetProblem
import app.kaeri.ui.SheetText
import app.kaeri.ui.SheetTitle
import java.util.Locale

// A chapter's sheet

/**
 * What a long press on a chapter offers, named by the chapter (fig 45): Read, Mark read or
 * unread, Mark read up to here, its download, Send to e-reader, and Versions with their count
 * last, opening in place.
 */
@Composable
internal fun ChapterSheet(
    series: SeriesView,
    row: ChapterRow,
    actions: AppActions,
    onDismiss: () -> Unit,
    onSend: (List<ChapterId>) -> Unit,
) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val work = series.work
    val read = row.state is ChapterReadState.Read
    var versionsOpen by remember { mutableStateOf(false) }
    fun then(action: () -> Unit) {
        action()
        onDismiss()
    }
    SettingsSheet(onDismiss = onDismiss) {
        Column(Modifier.padding(horizontal = 0.dp)) {
            Text(
                SeriesWords.heading(context, row),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(bottom = 4.dp).semantics { heading() },
            )
            SheetAction(Strings.seriesReadChapterMenu(context)) {
                then { actions.openReader(work, row.id, series.cover, null) }
            }
            SheetAction(if (read) Strings.seriesMarkUnread(context) else Strings.seriesMarkRead(context)) {
                then { store.reading.markRead(work, listOf(row.id), !read) }
            }
            if (!read) {
                SheetAction(Strings.seriesMarkReadUpTo(context)) { then { store.reading.markReadUpTo(work, row.id) } }
            }
            when (row.download) {
                null -> SheetAction(Strings.seriesDownload(context)) {
                    then { store.reading.download(work, DownloadPick.Chapters(listOf(row.id))) }
                }
                is ChapterDownloadState.Failed -> SheetAction(Strings.chapterRetry(context)) {
                    then { store.reading.download(work, DownloadPick.Chapters(listOf(row.id))) }
                }
                ChapterDownloadState.Done -> SheetAction(Strings.chapterDownloadDelete(context)) {
                    then { store.reading.deleteDownloads(work, listOf(row.id)) }
                }
                else -> SheetAction(Strings.chapterDownloadStop(context)) { then { store.reading.cancelDownloads(work, listOf(row.id)) } }
            }
            SheetAction(Strings.sendEreaderTitle(context)) {
                onDismiss()
                onSend(listOf(row.id))
            }
            if (row.versions > 1u) {
                SheetAction(Strings.seriesVersions(context, row.versions.toInt()), trailing = if (versionsOpen) Icons.Down else Icons.Chevron) {
                    versionsOpen = !versionsOpen
                }
                if (versionsOpen) ChapterVersions(work, row, onDismiss)
            }
        }
    }
}

/** One action in a chapter's sheet, 56 high, in silk. */
@Composable
private fun SheetAction(text: String, trailing: androidx.compose.ui.graphics.vector.ImageVector? = null, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).clickable(onClick = onClick),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        if (trailing != null) {
            Icon(trailing, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
        }
    }
}

/** A chapter's versions: each group's, the one in use checked, another made preferred for the series. */
@Composable
private fun ChapterVersions(work: WorkId, row: ChapterRow, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val versions by store.reading.versions.collectAsStateWithLifecycle()
    LaunchedEffect(row.id) { store.reading.loadVersions(work, row.id) }
    val list = versions[row.id]
    Column(Modifier.padding(start = 16.dp)) {
        if (list == null) {
            Text(
                Strings.seriesVersionsLoading(context),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(vertical = 12.dp),
            )
        }
        list.orEmpty().forEach { version ->
            val name = version.edition.versionLabel?.takeIf { it.isNotBlank() } ?: version.edition.sourceName
            val line = version.published?.let { agoWords(context, it) }
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 56.dp)
                    .clickable(enabled = !version.edition.preferred) {
                        store.reading.chooseEdition(work, version.edition)
                        onDismiss()
                    }
                    .semantics { if (version.edition.preferred) selected = true },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(
                        if (version.edition.preferred) Strings.seriesVersionInUse(context, name) else Strings.seriesPreferVersion(context, name),
                        style = MaterialTheme.typography.bodyLarge,
                    )
                    if (line != null) Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                if (version.edition.preferred) Icon(Icons.Check, contentDescription = null, modifier = Modifier.size(20.dp))
            }
        }
    }
}

// Read from

/**
 * Read from (fig 39): every source that has the series, the one in use checked, then the sources
 * that searched without finding it, in haze, each with Search there.
 */
@Composable
internal fun ReadFromSheet(work: WorkId, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val forWork by store.bookSources.forWork.collectAsStateWithLifecycle()
    val series by store.reading.series.collectAsStateWithLifecycle()
    val editions = forWork[work]
    val title = series[work]?.titles?.display.orEmpty()
    var searching by rememberSaveable { mutableStateOf<String?>(null) }
    LaunchedEffect(work) { store.bookSources.loadForWork(work) }
    DisposableEffect(work) { onDispose { store.bookSources.clearSourceSearch(work) } }
    SettingsSheet(onDismiss = onDismiss, tall = true, holdsInput = searching != null) {
        SheetTitle(Strings.seriesReadFromTitle(context))
        if (title.isNotEmpty()) SheetText(title, Modifier.padding(bottom = 8.dp))
        when {
            editions != null && editions.editions.isNotEmpty() -> {
                editions.editions.forEach { edition ->
                    EditionRow(edition) {
                        if (!edition.inUse) store.reading.chooseEdition(work, edition)
                        onDismiss()
                    }
                }
                SheetText(Strings.seriesReadFromNote(context), Modifier.padding(top = 12.dp))
            }
            editions == null || editions.asking -> Row(
                Modifier.padding(vertical = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp, color = MaterialTheme.colorScheme.onSurface)
                SheetText(Strings.seriesReadFromAsking(context))
            }
            editions.missing.isEmpty() -> Column(Modifier.padding(vertical = 12.dp)) {
                Text(Strings.seriesReadFromNoneTitle(context), style = MaterialTheme.typography.titleMedium)
                SheetText(Strings.seriesReadFromNoneLine(context), Modifier.padding(top = 4.dp))
            }
        }
        val missing = editions?.missing.orEmpty()
        if (missing.isNotEmpty()) {
            Text(
                Strings.readFromNotFound(context),
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 20.dp, bottom = 4.dp).semantics { heading() },
            )
            missing.forEach { miss ->
                MissRow(work, miss, title, open = searching == miss.sourceId) { searching = miss.sourceId }
            }
        }
    }
}

/** One source that has the series: its name, how it has it, and a check on the one in use. */
@Composable
private fun EditionRow(edition: BookEditionRow, onClick: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val view by store.bookSources.view.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    val lead = mutableListOf<String>()
    var folder = false
    val server = view?.servers?.firstOrNull { it.id == edition.sourceId }
    val plugin = view?.plugins?.firstOrNull { it.id == edition.sourceId }
    when {
        server != null -> lead += serverKind(context, server.kind)
        plugin?.store != null -> lead += Strings.bookSourcesFromStore(context, plugin.store!!)
        view?.folders?.any { it.id == edition.sourceId } == true -> folder = true
    }
    edition.versionLabel?.takeIf { it.isNotBlank() }?.let { lead += it }
    edition.language?.takeIf { it.isNotBlank() }?.let { code ->
        Locale.forLanguageTag(code).getDisplayLanguage(app.kaeri.store.AirTime.locale).takeIf { it.isNotBlank() }?.let { lead += it }
    }
    val latest = edition.latest ?: edition.chapters.toString()
    val parts = lead.toMutableList()
    parts += if (lead.isEmpty()) Strings.seriesReadFromChapters(context, latest) else Strings.seriesReadFromChaptersLater(context, latest)
    if (folder) parts += Strings.seriesReadFromOnDevice(context, deviceName(context))
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .clickable(onClick = onClick)
            .semantics { if (edition.inUse) selected = true }
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                edition.sourceName,
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = if (edition.inUse) FontWeight.Medium else null,
            )
            Text(parts.joinToString(" · "), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant) // i18n-skip
        }
        if (edition.inUse) Icon(Icons.Check, contentDescription = null, tint = colors.onSurface, modifier = Modifier.padding(start = 12.dp).size(20.dp))
    }
}

/** A source searched without finding the series: its name and why in haze, with Search there. */
@Composable
private fun MissRow(work: WorkId, miss: EditionMiss, title: String, open: Boolean, onOpen: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val found by store.bookSources.sourceSeries.collectAsStateWithLifecycle()
    var query by rememberSaveable(miss.sourceId) { mutableStateOf(title) }
    fun search() {
        val words = query.trim()
        if (words.isNotEmpty()) store.bookSources.searchSource(work, miss.sourceId, words)
    }
    Column(Modifier.fillMaxWidth()) {
        Row(Modifier.fillMaxWidth().heightIn(min = 64.dp).padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(miss.sourceName, style = MaterialTheme.typography.bodyLarge, color = colors.onSurfaceVariant)
                Text(miss.reason.text(context), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
            }
            if (!open) {
                TextButton(
                    onClick = {
                        onOpen()
                        query = title
                        store.bookSources.searchSource(work, miss.sourceId, title)
                    },
                    colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
                ) { Text(Strings.readFromSearchThere(context)) }
            }
        }
        if (open) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    placeholder = { Text(Strings.readFromSearchPlaceholder(context, miss.sourceName)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { search() }),
                    modifier = Modifier.weight(1f),
                )
                FilledTonalButton(onClick = { search() }, enabled = query.isNotBlank()) { Text(Strings.readFromSearch(context)) }
            }
            val result = found[work]?.takeIf { it.sourceId == miss.sourceId }
            when {
                result == null -> {}
                result.searching -> CircularProgressIndicator(
                    Modifier.padding(vertical = 12.dp).size(18.dp),
                    strokeWidth = 2.dp,
                    color = colors.onSurface,
                )
                result.failure != null -> SheetProblem(failureWords(context, result.failure!!))
                result.choices.isEmpty() -> SheetText(Strings.readFromSearchNone(context), Modifier.padding(vertical = 12.dp))
                else -> Column(Modifier.padding(top = 4.dp)) {
                    result.choices.forEach { choice ->
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = 64.dp)
                                .clickable { store.bookSources.chooseSourceSeries(work, miss.sourceId, choice.seriesRef) }
                                .padding(vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            ReadCover(choice.cover, 34.dp)
                            Column(Modifier.weight(1f)) {
                                Text(choice.title, style = MaterialTheme.typography.bodyLarge)
                                val facts = listOfNotNull(
                                    choice.year?.toString(),
                                    choice.language?.takeIf { it.isNotBlank() }?.let {
                                        Locale.forLanguageTag(it).getDisplayLanguage(app.kaeri.store.AirTime.locale).takeIf { name -> name.isNotBlank() }
                                    },
                                ).joinToString(" · ") // i18n-skip
                                if (facts.isNotEmpty()) Text(facts, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
                            }
                        }
                    }
                }
            }
        }
    }
}

/** A server's kind as its line says it: the product's name, or OPDS catalogue. */
private fun serverKind(context: Context, kind: BookServerKind): String = when (kind) {
    BookServerKind.KOMGA -> "Komga" // i18n-skip: a product name
    BookServerKind.KAVITA -> "Kavita" // i18n-skip: a product name
    BookServerKind.OPDS -> Strings.bookSourcesOpds(context)
}

/** "Android phone" or "Android tablet", for a folder on this device. */
private fun deviceName(context: Context): String =
    if (context.resources.configuration.smallestScreenWidthDp >= 600) Strings.deviceNameTablet(context) else Strings.deviceNamePhone(context)

// Tracking

/** The trackers that count reading, in the order the sheet lists those not connected. */
private val READING_TRACKERS = listOf("anilist", "mangaupdates", "kitsu", "myanimelist") // i18n-skip

/**
 * Tracking (fig 58): each tracker with what it holds and its score; one that differs from Kaeri
 * asks Keep Kaeri's place or Use theirs, one not sure which series this is lists its guesses.
 * Trackers that don't follow it come last, under Not tracked here.
 */
@Composable
internal fun TrackingSheet(work: WorkId, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val tracking by store.reading.tracking.collectAsStateWithLifecycle()
    val seriesMap by store.reading.series.collectAsStateWithLifecycle()
    val series = seriesMap[work] ?: return
    LaunchedEffect(work) { store.reading.loadTracking(work) }
    val rows = tracking[work] ?: series.tracking
    val tracked = rows.filter { it.mapping != TrackerMapping.Unmapped }
    val unmapped = rows.filter { it.mapping == TrackerMapping.Unmapped }
    val away = READING_TRACKERS.filter { id -> rows.none { it.tracker == id || (id == "myanimelist" && it.tracker == "mal") } }
    val newest = tracked.filter { !it.divergent }.maxOfOrNull { it.chapters } ?: tracked.maxOfOrNull { it.chapters } ?: 0u
    SettingsSheet(onDismiss = onDismiss, tall = true) {
        SheetTitle(Strings.seriesTracking(context))
        SheetText(series.titles.display, Modifier.padding(bottom = 8.dp))
        tracked.forEach { row -> TrackerRow(series, row, newest) }
        tracked.firstNotNullOfOrNull { it.splitNote }?.let { SheetText(it.text(context), Modifier.padding(top = 12.dp)) }
        if (away.isNotEmpty() || unmapped.isNotEmpty()) {
            Text(
                Strings.seriesNotTracked(context),
                style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 20.dp, bottom = 4.dp).semantics { heading() },
            )
            unmapped.forEach { PlainTrackerRow(SeriesWords.tracker(it.tracker), Strings.seriesTrackerNotThere(context), null) }
            away.forEach { id ->
                PlainTrackerRow(SeriesWords.tracker(id), Strings.seriesTrackerConnect(context)) {
                    onDismiss()
                    navigator.openSettings(SettingsPane.Accounts)
                }
            }
        }
        SheetButtons { QuietButton(Strings.actionDone(context), onClick = onDismiss) }
    }
}

@Composable
private fun PlainTrackerRow(name: String, line: String, onClick: (() -> Unit)?) {
    val colors = MaterialTheme.colorScheme
    Column(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier)
            .padding(vertical = 8.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Text(name, style = MaterialTheme.typography.bodyLarge, color = colors.onSurfaceVariant)
        Text(line, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
    }
}

/** One tracker: its name, what it holds in words, its score, and the question when it differs. */
@Composable
private fun TrackerRow(series: SeriesView, row: TrackerReadRow, newest: UInt) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    val name = SeriesWords.tracker(row.tracker)
    val line = when {
        row.mapping == TrackerMapping.Looking -> Strings.seriesTrackerLooking(context)
        row.mapping is TrackerMapping.Ask -> Strings.seriesTrackerAsk(context)
        row.divergent && row.chapters < newest -> Strings.seriesTrackerBehind(context, row.chapters.toInt(), newest.toInt())
        row.divergent -> Strings.seriesTrackerAhead(context, row.chapters.toInt())
        else -> listOfNotNull(row.status?.let { listWords(context, it) }, SeriesWords.progress(context, row, series)).joinToString(" · ") // i18n-skip
    }
    Column(Modifier.fillMaxWidth().padding(vertical = 10.dp)) {
        Row(verticalAlignment = Alignment.Top) {
            Column(Modifier.weight(1f)) {
                Text(name, style = MaterialTheme.typography.bodyLarge)
                Text(line, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
            }
            val score = row.score
            if (score != null && score > 0f && !row.divergent) {
                Text(
                    Strings.seriesTrackerScore(context, scoreWords(score)),
                    style = MaterialTheme.typography.bodyMedium.tabular(),
                    color = colors.onSurfaceVariant,
                    modifier = Modifier.padding(start = 12.dp),
                )
            }
        }
        if (row.divergent && row.mapping == TrackerMapping.Mapped) {
            Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilledTonalButton(onClick = { store.reading.resolveConflict(series.work, row.tracker, true) }) {
                    Text(Strings.seriesTrackerKeepKaeri(context))
                }
                TextButton(
                    onClick = { store.reading.resolveConflict(series.work, row.tracker, false) },
                    colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
                ) { Text(Strings.seriesTrackerTakeTheirs(context, name)) }
            }
        }
        val ask = row.mapping as? TrackerMapping.Ask
        if (ask != null) {
            Column(Modifier.padding(top = 4.dp)) {
                ask.candidates.forEach { candidate ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 48.dp)
                            .clickable { store.reading.chooseMapping(series.work, row.tracker, candidate.trackerId) },
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            listOfNotNull(candidate.title, candidate.year?.toString()).joinToString(" · "), // i18n-skip
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.weight(1f),
                        )
                        Text(
                            Strings.libraryMatchPercent(context, candidate.confidence.toInt()),
                            style = MaterialTheme.typography.bodyMedium.tabular(),
                            color = colors.onSurfaceVariant,
                        )
                    }
                }
                TextButton(
                    onClick = { store.reading.chooseMapping(series.work, row.tracker, null) },
                    colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
                ) { Text(Strings.seriesTrackerNoneOfThese(context)) }
            }
        }
    }
}

/** A score as the trackers write it: "8", "7.5". */
internal fun scoreWords(score: Float): String =
    if (score % 1f == 0f) score.toInt().toString() else String.format(app.kaeri.store.AirTime.locale, "%.1f", score)

// The status menu

private enum class StatusPane { Lists, Score, Remove, Removed }

/**
 * The status menu (fig 40): AniList's lists with the one the series is on checked, the score, and
 * Remove from lists, whose question opens in the same menu, anchored to its button, and then
 * says what each tracker did.
 */
@Composable
internal fun StatusMenu(series: SeriesView, expanded: Boolean, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val removals by store.reading.removals.collectAsStateWithLifecycle()
    var pane by remember { mutableStateOf(StatusPane.Lists) }
    val result = removals[series.work]
    LaunchedEffect(result) { if (result != null && pane == StatusPane.Remove) pane = StatusPane.Removed }
    fun close() {
        if (pane == StatusPane.Removed) store.reading.removalSeen(series.work)
        pane = StatusPane.Lists
        onDismiss()
    }
    Menu(expanded = expanded, onDismissRequest = ::close) {
        AnimatedContent(pane, label = "status") { now ->
            Column(Modifier.widthIn(min = 200.dp, max = 320.dp)) {
                when (now) {
                    StatusPane.Lists -> {
                        READ_STATUSES.forEach { status ->
                            MenuChoice(listWords(context, status), checked = series.list == status) {
                                store.reading.setStatus(series.work, status)
                                close()
                            }
                        }
                        HorizontalDivider()
                        DropdownMenuItem(
                            text = { Text(Strings.seriesScore(context)) },
                            trailingIcon = {
                                series.score?.takeIf { it > 0f }?.let {
                                    Text(scoreWords(it), style = MaterialTheme.typography.bodyMedium.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            },
                            onClick = { pane = StatusPane.Score },
                        )
                        if (series.list != null) {
                            HorizontalDivider()
                            DropdownMenuItem(text = { Text(Strings.seriesRemoveFromLists(context)) }, onClick = { pane = StatusPane.Remove })
                        }
                    }
                    StatusPane.Score -> {
                        DropdownMenuItem(
                            text = { Text(Strings.seriesScore(context).trimEnd('…', '.')) },
                            leadingIcon = { Icon(Icons.Back, contentDescription = Strings.actionBack(context)) },
                            onClick = { pane = StatusPane.Lists },
                        )
                        HorizontalDivider()
                        (10 downTo 1).forEach { score ->
                            MenuChoice(score.toString(), checked = series.score?.let { kotlin.math.round(it).toInt() } == score) {
                                store.reading.setScore(series.work, score.toFloat())
                                close()
                            }
                        }
                    }
                    StatusPane.Remove -> Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                        Text(Strings.seriesRemoveFromListsTitle(context, series.titles.display), style = MaterialTheme.typography.titleSmall)
                        Text(
                            Strings.seriesRemoveFromListsLine(context),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 6.dp),
                        )
                        Row(Modifier.fillMaxWidth().padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
                            QuietButton(Strings.actionCancel(context)) { pane = StatusPane.Lists }
                            TextButton(
                                onClick = { store.reading.removeFromLists(series.work) },
                                colors = ButtonDefaults.textButtonColors(contentColor = ObiSilk.Vermilion),
                            ) { Text(Strings.actionRemove(context)) }
                        }
                    }
                    StatusPane.Removed -> Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                        Text(Strings.seriesRemovedTitle(context), style = MaterialTheme.typography.titleSmall)
                        result.orEmpty().forEach { removal ->
                            Column(Modifier.padding(top = 8.dp)) {
                                Text(SeriesWords.tracker(removal.tracker), style = MaterialTheme.typography.bodyLarge)
                                Text(
                                    removal.line.joinToString(" · ") { it.text(context) }, // i18n-skip
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = if (removal.outcome is ListRemovalOutcome.Failed) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                        Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.End) {
                            QuietButton(Strings.actionDone(context), onClick = ::close)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun MenuChoice(text: String, checked: Boolean, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(text, fontWeight = if (checked) FontWeight.Medium else null) },
        trailingIcon = if (checked) {
            { Icon(Icons.Check, contentDescription = null, modifier = Modifier.size(20.dp)) }
        } else {
            null
        },
        onClick = onClick,
        modifier = Modifier.semantics { if (checked) selected = true },
    )
}

// The Download menu

/**
 * Download: the next 5 or 10 unread, every unread chapter, the volumes still to fetch, Choose
 * chapters, Send to e-reader for what is downloaded, and deleting the series' downloads.
 */
@Composable
internal fun DownloadMenu(
    series: SeriesView,
    volumes: List<VolumeGroup>,
    expanded: Boolean,
    onDismiss: () -> Unit,
    onChoose: () -> Unit,
    onSend: (List<ChapterId>) -> Unit,
) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val work = series.work
    val all = volumes.chapterRows()
    val unread = all.filter { it.state !is ChapterReadState.Read && it.download == null }
    val downloaded = volumes.downloaded()
    val any = all.filter { it.download != null }
    val toFetch = volumes.filter { group -> group.chapters.any { it.state !is ChapterReadState.Read && it.download == null } }.take(3)
    fun pick(what: DownloadPick) {
        store.reading.download(work, what)
        onDismiss()
    }
    Menu(expanded = expanded, onDismissRequest = onDismiss) {
        DropdownMenuItem(text = { Text(Strings.downloadNext(context, 5)) }, enabled = unread.isNotEmpty(), onClick = { pick(DownloadPick.NextUnread(5u)) })
        DropdownMenuItem(text = { Text(Strings.downloadNext(context, 10)) }, enabled = unread.size > 5, onClick = { pick(DownloadPick.NextUnread(10u)) })
        DropdownMenuItem(text = { Text(Strings.downloadUnread(context)) }, enabled = unread.isNotEmpty(), onClick = { pick(DownloadPick.Unread) })
        if (toFetch.isNotEmpty()) HorizontalDivider()
        toFetch.forEach { group ->
            val missing = group.chapters.filter { it.state !is ChapterReadState.Read && it.download == null }
            val name = SeriesWords.volume(context, group, novel = false).first
            DropdownMenuItem(
                text = { Text(Strings.downloadVolumeCount(context, name, missing.size)) },
                onClick = {
                    val volume = group.volume
                    pick(if (volume != null) DownloadPick.Volume(volume) else DownloadPick.Chapters(missing.map { it.id }))
                },
            )
        }
        HorizontalDivider()
        DropdownMenuItem(
            text = { Text(Strings.downloadChoose(context)) },
            onClick = {
                onDismiss()
                onChoose()
            },
        )
        if (downloaded.isNotEmpty()) {
            DropdownMenuItem(
                text = { Text(Strings.sendEreaderTitle(context)) },
                onClick = {
                    onDismiss()
                    onSend(downloaded.map { it.id })
                },
            )
        }
        if (any.isNotEmpty()) {
            HorizontalDivider()
            DropdownMenuItem(
                text = { Text(Strings.downloadDeleteSeries(context), color = ObiSilk.Vermilion) },
                onClick = {
                    store.reading.deleteDownloads(work, any.map { it.id })
                    onDismiss()
                },
            )
        }
    }
}

// Read again

/** Read again from the start (fig 61): what happens to the place and the trackers, and the one action. */
@Composable
internal fun ReadAgainDialog(series: SeriesView, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(Strings.readAgainTitle(context, series.titles.display)) },
        text = { Text(Strings.readAgainLine(context)) },
        confirmButton = {
            Button(onClick = {
                store.reading.readAgain(series.work)
                onDismiss()
            }) { Text(Strings.readAgainConfirm(context)) }
        },
        dismissButton = { QuietButton(Strings.actionCancel(context), onClick = onDismiss) },
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
    )
}

// Choose chapters' bar

/**
 * Choose chapters' bar, inset where the navigation bar was: what the download will take as plain
 * text, Mark read, and the one amber action with the count.
 */
@Composable
internal fun ChooseBar(series: SeriesView, chosen: Set<ChapterId>, onMarkRead: () -> Unit, onDownload: () -> Unit, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val estimates by store.reading.estimates.collectAsStateWithLifecycle()
    val sheets = LocalAppSheets.current
    DisposableEffect(Unit) {
        sheets.barHidden = true
        onDispose {
            sheets.barHidden = false
            store.reading.clearEstimate(series.work)
        }
    }
    LaunchedEffect(chosen) {
        kotlinx.coroutines.delay(150)
        if (chosen.isEmpty()) store.reading.clearEstimate(series.work) else store.reading.estimateDownload(series.work, DownloadPick.Chapters(chosen.toList()))
    }
    val line = estimates[series.work]?.takeIf { chosen.isNotEmpty() }?.line?.joinToString(" · ") { it.text(context) }.orEmpty() // i18n-skip
    Surface(
        modifier = modifier
            .fillMaxWidth()
            .navigationBarsPadding()
            .padding(horizontal = 12.dp, vertical = 8.dp),
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        tonalElevation = 0.dp,
        shadowElevation = 3.dp,
    ) {
        Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
            if (line.isNotEmpty()) {
                Text(
                    line,
                    style = MaterialTheme.typography.bodyMedium.tabular(),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(bottom = 10.dp, start = 4.dp),
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                FilledTonalButton(onClick = onMarkRead, enabled = chosen.isNotEmpty(), modifier = Modifier.heightIn(min = 48.dp)) {
                    Text(Strings.seriesMarkRead(context))
                }
                Button(onClick = onDownload, enabled = chosen.isNotEmpty(), modifier = Modifier.weight(1f).heightIn(min = 48.dp)) {
                    Text(Strings.downloadChapters(context, chosen.size), maxLines = 1)
                }
            }
        }
    }
}

