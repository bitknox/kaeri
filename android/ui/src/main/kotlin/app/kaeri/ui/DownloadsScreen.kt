// Downloads (AndroidDownloads): the queue by title, each title with its cover, how many episodes
// and how much they take, and each episode with where it stands and the one thing to do with it;
// the watched episodes still seeding; and, in the bar, what the downloads take and what is free.
// With Read or Both, reading's chapters by series too (fig 46): with Both one section per medium
// with its total, Reading first when the person came from Read.

package app.kaeri.ui

import app.kaeri.store.plainReason
import android.os.StatFs
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.TextButton
import androidx.compose.foundation.background
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import java.util.Locale
import app.kaeri.design.ShowTitle
import app.kaeri.design.tabular
import app.kaeri.engine.model.DownloadRow
import app.kaeri.engine.model.DownloadState
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.SeedRow
import app.kaeri.store.DownloadGroup
import app.kaeri.store.DownloadsModel
import app.kaeri.store.Wording
import app.kaeri.store.bytesOnDisk
import app.kaeri.store.reads
import app.kaeri.store.text
import app.kaeri.store.watches
import app.kaeri.i18n.R
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.res.pluralStringResource
import app.kaeri.i18n.Strings
import android.content.Context

internal const val DOWNLOADS_ROUTE = "downloads"

/** The Downloads screen's route. */
internal fun NavGraphBuilder.downloadPages(actions: AppActions) {
    composable(DOWNLOADS_ROUTE) { DownloadsScreen(actions) }
}

/** The bytes free where downloads are kept: Kaeri's own space on the device. */
@Composable
internal fun freeBytes(): Long? {
    val context = LocalContext.current
    return remember(context) {
        runCatching { StatFs((context.getExternalFilesDir(null) ?: context.filesDir).path).availableBytes }.getOrNull()
    }
}

/** "3.4 GB used · 38 GB free". */
internal fun storageWords(context: Context, used: Long, free: Long?): String =
    if (free == null) Strings.downloadsStorageUsed(context, Wording.bytes(context, used))
    else Strings.downloadsStorageUsedFree(context, Wording.bytes(context, used), Wording.bytes(context, free))

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun DownloadsScreen(actions: AppActions = AppActions()) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val isFor = kaeriIsFor()
    val reads = isFor.reads
    val watches = isFor.watches
    val reading by store.reading.downloads.collectAsStateWithLifecycle()
    LaunchedEffect(reads) { if (reads) store.reading.loadDownloads() }
    // The series Send to e-reader is open for, from a series' long press, with its downloaded chapters.
    var send by remember { mutableStateOf<Pair<app.kaeri.engine.core.WorkId, List<app.kaeri.engine.core.ChapterId>>?>(null) }
    val allRows by store.downloads.rows.collectAsStateWithLifecycle()
    val allSeeding by store.downloads.seeding.collectAsStateWithLifecycle()
    // With Read alone, the shows' downloads stay out of sight.
    val rows = if (watches) allRows else emptyList()
    val seeding = if (watches) allSeeding else emptyList()
    val readSeries = if (reads) reading?.series.orEmpty() else emptyList()
    val titles by store.titles.entries.collectAsStateWithLifecycle()
    val episodes by store.titles.episodes.collectAsStateWithLifecycle()
    val loaded by store.downloads.loaded.collectAsStateWithLifecycle()
    val free = freeBytes()
    var menu by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { store.loadDownloads(force = true) }
    // The titles' episode names, for the rows that have none of their own.
    LaunchedEffect(rows.map { it.anilistId }.toSet()) {
        for (id in rows.map { it.anilistId }.distinct()) store.loadEpisodes(id)
    }
    val running = rows.any { it.state == DownloadState.ACTIVE || it.state == DownloadState.QUEUED }
    val paused = rows.any { it.state == DownloadState.PAUSED }
    val colors = MaterialTheme.colorScheme
    Scaffold(
        containerColor = colors.surface,
        contentWindowInsets = WindowInsets(0),
        topBar = {
            TopAppBar(
                title = {
                    Column(Modifier.padding(start = 4.dp)) {
                        Text(stringResource(R.string.downloads_title), style = MaterialTheme.typography.titleLarge)
                        Text(
                            storageWords(LocalContext.current, store.downloads.bytes(rows) + (if (reads) reading?.meter?.used?.toLong() ?: 0L else 0L), free),
                            style = MaterialTheme.typography.labelMedium.tabular(),
                            color = colors.onSurfaceVariant,
                        )
                    }
                },
                navigationIcon = { IconButton(onClick = navigator::back) { Icon(Icons.Back, contentDescription = stringResource(R.string.action_back)) } },
                actions = {
                    if (running) {
                        IconButton(onClick = store::pauseAllDownloads) { Icon(Icons.Pause, contentDescription = stringResource(R.string.downloads_pause_all_label)) }
                    } else if (paused) {
                        IconButton(onClick = store::resumeAllDownloads) { Icon(Icons.Resume, contentDescription = stringResource(R.string.downloads_resume_all_label)) }
                    }
                    Box {
                        IconButton(onClick = { menu = true }) { Icon(Icons.More, contentDescription = stringResource(R.string.action_more)) }
                        DropdownMenu(
                            expanded = menu,
                            onDismissRequest = { menu = false },
                            shape = MaterialTheme.shapes.extraSmall,
                            containerColor = colors.surfaceContainerHigh,
                        ) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.downloads_clear_finished)) },
                                enabled = rows.any { it.state == DownloadState.DONE },
                                onClick = {
                                    menu = false
                                    store.clearCompletedDownloads()
                                },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.action_open_settings)) },
                                onClick = {
                                    menu = false
                                    navigator.openSettings(SettingsPane.Downloads)
                                },
                            )
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = colors.surface, scrolledContainerColor = colors.surfaceContainer),
            )
        },
    ) { padding ->
        // Before the first answer, nothing for 600 ms and then the rows' places; never the empty
        // state ahead of the answer (A6).
        val watchWaits = watches && !loaded && rows.isEmpty()
        val readWaits = reads && reading == null
        if ((watchWaits || !watches) && (readWaits || !reads) && readSeries.isEmpty()) {
            Column(Modifier.padding(top = padding.calculateTopPadding())) { AfterWait { DownloadPlaceholders() } }
            return@Scaffold
        }
        if (rows.isEmpty() && seeding.isEmpty() && readSeries.isEmpty()) {
            Column(
                Modifier.fillMaxSize().padding(padding).padding(horizontal = 32.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                app.kaeri.design.Nagare(cell = 5)
                Text(stringResource(R.string.downloads_empty_title), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 16.dp))
                val detail = when {
                    !reads -> stringResource(R.string.downloads_empty_detail)
                    !watches -> stringResource(R.string.read_downloads_empty_detail)
                    else -> stringResource(R.string.read_downloads_empty_both)
                }
                SheetText(detail, Modifier.padding(top = 4.dp))
            }
            return@Scaffold
        }
        val groups = DownloadsModel.groups(rows)
        Refreshable({ store.loadDownloads(force = true) }, top = padding.calculateTopPadding()) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(top = padding.calculateTopPadding(), bottom = 48.dp),
        ) {
            // With Both each medium has its own section and total; Reading comes first from Read.
            val both = reads && watches
            val readFirst = !watches || navigator.openedFrom == Destination.Read
            val readingSection: () -> Unit = {
                reading?.takeIf { readSeries.isNotEmpty() }?.let { view ->
                    readingDownloads(view, divided = !readFirst && (groups.isNotEmpty() || seeding.isNotEmpty()), actions = actions, showSends = send == null) { work, chapters ->
                        send = work to chapters
                    }
                }
            }
            if (readFirst) readingSection()
            if (both && (groups.isNotEmpty() || seeding.isNotEmpty())) {
                item(key = "shows-heading", contentType = "section") {
                    Box(itemMotion()) {
                        SectionHeading(
                            stringResource(R.string.settings_group_shows),
                            pluralStringResource(R.plurals.downloads_group_summary, rows.size, rows.size, Wording.bytes(LocalContext.current, store.downloads.bytes(rows))),
                            divided = readFirst && readSeries.isNotEmpty(),
                        )
                    }
                }
            }
            // Rows come and go on standard, every item with the rest (motion-android.md's list
            // insert and remove), so a cancelled or cleared download never leaves a gap that snaps.
            groups.forEachIndexed { index, group ->
                if (index > 0) item(key = "rule-${group.id}", contentType = "rule") { HorizontalDivider(itemMotion().padding(vertical = 8.dp), color = colors.outlineVariant) }
                item(key = "title-${group.id}", contentType = "group") { Box(itemMotion()) { GroupHeader(group, titles[group.id]) } }
                // TODO(engine): AndroidDownloads puts the show's own "Keep next 3 episodes ·
                // Higher quality" row here, with a switch, under its title. Keep next and the
                // download quality are Settings-wide; the row waits on per-show settings for both
                // (`ShowSettingKey` has neither yet).
                items(group.rows, key = { "row-${it.id}" }, contentType = { "download" }) { row ->
                    Box(itemMotion()) { DownloadItem(row, episodes[row.anilistId]?.firstOrNull { it.number == row.episode }) }
                }
            }
            if (seeding.isNotEmpty()) {
                item(key = "seeding", contentType = "seeding") {
                    Column(itemMotion()) {
                        if (groups.isNotEmpty()) HorizontalDivider(Modifier.padding(vertical = 8.dp), color = colors.outlineVariant)
                        SettingsHeader(stringResource(R.string.downloads_seeding_after_watching), Modifier.padding(start = 0.dp))
                    }
                }
                items(seeding, key = { "seed-${it.name}" }, contentType = { "seed" }) { Box(itemMotion()) { SeedItem(it) } }
                item(key = "seeding-note", contentType = "note") {
                    Box(itemMotion()) { SettingsFootnote(stringResource(R.string.downloads_seeding_note)) }
                }
            }
            if (!readFirst) readingSection()
        }
        }
    }
    send?.let { (work, chapters) ->
        app.kaeri.ui.reading.SendToEreaderSheet(work, chapters, actions) { send = null }
    }
}

/** A medium's heading in Downloads with Both: "Reading" and its total beside it, as the rows add up. */
@Composable
private fun SectionHeading(title: String, total: String, divided: Boolean) {
    Column {
        if (divided) HorizontalDivider(Modifier.padding(vertical = 8.dp), color = MaterialTheme.colorScheme.outlineVariant)
        Row(
            Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 16.dp).semantics(mergeDescendants = true) { heading() },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(title, style = MaterialTheme.typography.titleLarge)
            Text(total, style = MaterialTheme.typography.bodyMedium.tabular(), color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

/**
 * Reading's section: its heading with the total, the sends going on when [showSends], then one row
 * per series with its line and, while bytes move, the ring.
 */
private fun androidx.compose.foundation.lazy.LazyListScope.readingDownloads(
    view: app.kaeri.engine.model.BookDownloadsView,
    divided: Boolean,
    actions: AppActions,
    showSends: Boolean,
    onSend: (app.kaeri.engine.core.WorkId, List<app.kaeri.engine.core.ChapterId>) -> Unit,
) {
    item(key = "reading-heading", contentType = "section") {
        val context = LocalContext.current
        // The engine's total, which counts the rows below; the meter's while it sends none.
        val total = view.total.takeIf { it.isNotEmpty() }?.joinToString(" · ") { it.text(context) } // i18n-skip
            ?: Strings.readDownloadsTotal(context, view.meter.chapters.toInt(), Wording.bytes(context, view.meter.used.toLong()))
        Box(itemMotion()) { SectionHeading(stringResource(R.string.settings_group_reading), total, divided) }
    }
    // Every send under way, held for Wi-Fi, ready or failed, while no Send sheet covers them.
    if (showSends) {
        item(key = "reading-sends", contentType = "sends") {
            Box(itemMotion()) {
                app.kaeri.ui.reading.SendStatusRows(null, actions, Modifier.padding(horizontal = 16.dp).padding(bottom = 8.dp))
            }
        }
    }
    items(view.series, key = { "read-${it.work}" }, contentType = { "read-series" }) { series ->
        Box(itemMotion()) {
            ReadSeriesDownload(series, view.rows.filter { it.work == series.work }, onSend)
        }
    }
}

/**
 * A series in Downloads: its cover, its title, its line ("6 chapters · 214 MB · 9 read"), a
 * failure with its cause and Retry, Delete read once chapters are read, the ring while a chapter
 * downloads; a long press offers Send to e-reader and deleting its downloads.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ReadSeriesDownload(
    series: app.kaeri.engine.model.BookDownloadSeries,
    rows: List<app.kaeri.engine.model.BookDownloadRow>,
    onSend: (app.kaeri.engine.core.WorkId, List<app.kaeri.engine.core.ChapterId>) -> Unit,
) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val haptics = androidx.compose.ui.platform.LocalHapticFeedback.current
    var menu by remember { mutableStateOf(false) }
    val done = rows.filter { it.state == app.kaeri.engine.model.ChapterDownloadState.Done }.map { it.chapter.id }
    val moving = rows.filter { it.state !is app.kaeri.engine.model.ChapterDownloadState.Done && it.state !is app.kaeri.engine.model.ChapterDownloadState.Failed }.map { it.chapter.id }
    val failed = series.failed.isNotEmpty()
    val line = series.line.joinToString(" · ") { it.text(context) } // i18n-skip
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 72.dp)
                .combinedClickable(
                    onClick = { navigator.openSeries(series.work, "downloads") },
                    onLongClickLabel = stringResource(R.string.action_more),
                    onLongClick = {
                        haptics.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
                        menu = true
                    },
                )
                .padding(start = 16.dp, end = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            app.kaeri.ui.reading.ReadCover(series.cover, 40.dp, Modifier.sharedWork(Shared.Cover, series.work, from = "downloads"))
            Column(Modifier.weight(1f).padding(vertical = 8.dp)) {
                Text(series.title, style = ShowTitle.row)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.Top) {
                    if (failed) Icon(Icons.Problem, contentDescription = null, tint = app.kaeri.design.ObiSilk.Vermilion, modifier = Modifier.padding(top = 2.dp).size(16.dp))
                    Text(line, style = MaterialTheme.typography.bodyMedium.tabular(), color = if (failed) colors.onSurface else colors.onSurfaceVariant)
                }
            }
            val progress = series.progress
            when {
                progress != null && moving.isNotEmpty() -> app.kaeri.ui.reading.ChapterRing(progress, stringResource(R.string.chapter_download_stop)) {
                    store.reading.cancelDownloads(series.work, moving)
                }
                failed -> QuietButton(stringResource(R.string.chapter_retry)) {
                    store.reading.download(series.work, app.kaeri.engine.model.DownloadPick.Chapters(series.failed))
                }
                series.read > 0u -> QuietButton(stringResource(R.string.read_downloads_delete_read)) { store.reading.deleteRead(series.work) }
            }
        }
        Menu(expanded = menu, onDismissRequest = { menu = false }) {
            if (done.isNotEmpty()) {
                DropdownMenuItem(text = { Text(Strings.sendTitle(context)) }, onClick = {
                    menu = false
                    onSend(series.work, done)
                })
            }
            if (series.read > 0u) {
                DropdownMenuItem(text = { Text(stringResource(R.string.read_downloads_delete_read)) }, onClick = {
                    menu = false
                    store.reading.deleteRead(series.work)
                })
            }
            DropdownMenuItem(text = { Text(stringResource(R.string.download_delete_series)) }, onClick = {
                menu = false
                store.reading.deleteDownloads(series.work, rows.map { it.chapter.id })
            })
        }
    }
}

/** A title's head: its cover, its title, and how many episodes and how much. */
@Composable
private fun GroupHeader(group: DownloadGroup, entry: Entry?) {
    Row(
        Modifier.fillMaxWidth().height(72.dp).padding(horizontal = 16.dp).semantics(mergeDescendants = true) { heading() },
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (entry != null) {
            CoverArt(entry, 40.dp, Modifier.size(40.dp, 60.dp))
        } else {
            app.kaeri.design.CoverPlate(null, Modifier.size(40.dp, 60.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(entry?.showTitle ?: group.title, style = ShowTitle.row, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                pluralStringResource(R.plurals.downloads_group_summary, group.rows.size, group.rows.size, Wording.bytes(LocalContext.current, group.bytesOnDisk)),
                style = MaterialTheme.typography.bodyMedium.tabular(),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/**
 * An episode: "Episode 8 · A Town Without Night", where it stands, a bar while it runs or waits
 * paused, and its one action: play, pause, resume, remove or retry. A long press offers deleting
 * it, or taking it off the list.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun DownloadItem(row: DownloadRow, episode: EpisodeRow?) {
    val store = LocalAppStore.current
    val rates by store.downloads.rates.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    var menu by remember { mutableStateOf(false) }
    // Deleting a download and its file asks first, from the menu or a swipe.
    var deleting by remember { mutableStateOf(false) }
    val name = listOfNotNull(stringResource(R.string.downloads_episode_name, row.episode.toInt()), episode?.title?.takeIf { it.isNotBlank() }).joinToString(" · ")
    val progress = row.state == DownloadState.ACTIVE || row.state == DownloadState.PAUSED
    val hasFile = row.state == DownloadState.DONE || row.path != null
    // A row with bytes behind it is deleted, which asks first, however it is taken off: its file
    // goes with it. One with none is only taken off the list.
    val hasBytes = hasFile || row.downloaded > 0uL
    val remove = { store.cancelDownload(row.id) }
    val haptics = androidx.compose.ui.platform.LocalHapticFeedback.current
    // The menu's items, for TalkBack too.
    val talkback = listOfNotNull(
        if (row.state == DownloadState.QUEUED) androidx.compose.ui.semantics.CustomAccessibilityAction(stringResource(R.string.downloads_start_now)) { store.startDownloadNow(row.id); true } else null,
        if (hasBytes) androidx.compose.ui.semantics.CustomAccessibilityAction(stringResource(R.string.downloads_delete_one_title)) { deleting = true; true } else null,
        if (!hasBytes) androidx.compose.ui.semantics.CustomAccessibilityAction(stringResource(R.string.downloads_remove_from_list)) { remove(); true } else null,
    )
    Swipeable(
        start = null,
        // From the end: Delete for a row with bytes, which asks first, or Cancel for one with none.
        end = if (hasBytes) SwipeAction(stringResource(R.string.action_delete), Icons.Clear) { deleting = true } else SwipeAction(stringResource(R.string.action_cancel), Icons.Clear, remove),
    ) {
    Box(Modifier.background(colors.surface)) {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = if (progress) 88.dp else 72.dp)
                .semantics { customActions = talkback }
                .combinedClickable(
                    onClick = { if (row.state == DownloadState.DONE) store.playDownload(row) },
                    onLongClickLabel = stringResource(R.string.action_more),
                    onLongClick = {
                        haptics.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.LongPress)
                        menu = true
                    },
                )
                .padding(start = 72.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Column(Modifier.weight(1f).padding(vertical = 8.dp)) {
                Text(name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                // Queued by keep next rather than asked for, and let go of a day after it is watched.
                if (row.ahead) {
                    Text(stringResource(R.string.downloads_caption_ahead), style = MaterialTheme.typography.labelMedium, color = colors.onSurfaceVariant, maxLines = 2)
                }
                StateLine(row, store.downloads.secondsLeft(row, rates), rates[row.id])
                if (progress) {
                    LinearProgressIndicator(
                        progress = { row.fraction.toFloat().coerceIn(0f, 1f) },
                        modifier = Modifier.padding(top = 8.dp).fillMaxWidth().height(4.dp),
                        color = if (row.state == DownloadState.ACTIVE) colors.secondary else colors.onSurfaceVariant,
                        trackColor = colors.outline,
                        gapSize = 4.dp,
                        drawStopIndicator = {},
                    )
                }
            }
            Box(Modifier.heightIn(min = 48.dp).widthIn(min = 48.dp), contentAlignment = Alignment.Center) {
                when (row.state) {
                    DownloadState.DONE -> IconButton(onClick = { store.playDownload(row) }) { Icon(Icons.PlayRow, contentDescription = stringResource(R.string.downloads_play_episode, row.episode.toInt())) }
                    DownloadState.ACTIVE -> IconButton(onClick = { store.toggleDownload(row.id) }) { Icon(Icons.Pause, contentDescription = stringResource(R.string.downloads_pause_one)) }
                    DownloadState.PAUSED -> IconButton(onClick = { store.toggleDownload(row.id) }) { Icon(Icons.Resume, contentDescription = stringResource(R.string.downloads_resume_one)) }
                    DownloadState.QUEUED -> Row(verticalAlignment = Alignment.CenterVertically) {
                        QuietButton(stringResource(R.string.downloads_start_now)) { store.startDownloadNow(row.id) }
                        // Where Pause sat a moment ago: a started download is not dropped by a repeated tap, it asks.
                        IconButton(onClick = { if (hasBytes) deleting = true else remove() }) {
                            Icon(Icons.Clear, contentDescription = stringResource(R.string.downloads_remove_from_queue), tint = colors.onSurfaceVariant)
                        }
                    }
                    DownloadState.FAILED -> QuietButton(stringResource(R.string.action_try_again)) { store.downloadEpisode(row.anilistId, row.episode) }
                }
            }
        }
        DropdownMenu(
            expanded = menu,
            onDismissRequest = { menu = false },
            shape = MaterialTheme.shapes.extraSmall,
            containerColor = colors.surfaceContainerHigh,
        ) {
            if (row.state == DownloadState.QUEUED) {
                DropdownMenuItem(text = { Text(stringResource(R.string.downloads_start_now)) }, onClick = {
                    menu = false
                    store.startDownloadNow(row.id)
                })
            }
            if (hasBytes) {
                DropdownMenuItem(text = { Text(stringResource(R.string.downloads_delete_one_title)) }, onClick = {
                    menu = false
                    deleting = true
                })
            }
            if (!hasBytes) {
                DropdownMenuItem(text = { Text(stringResource(R.string.downloads_remove_from_list)) }, onClick = {
                    menu = false
                    remove()
                })
            }
        }
    }
    }
    if (deleting) {
        AlertDialog(
            onDismissRequest = { deleting = false },
            properties = AskingDialog,
            title = { Text(stringResource(R.string.downloads_delete_title, stringResource(R.string.downloads_episode_name, row.episode.toInt()))) },
            text = { Text(stringResource(R.string.downloads_delete_detail)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        deleting = false
                        store.deleteDownload(row.id)
                    },
                    colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
                ) { Text(stringResource(R.string.action_delete)) }
            },
            dismissButton = { QuietButton(stringResource(R.string.action_cancel)) { deleting = false } },
            containerColor = colors.surfaceContainerHigh,
        )
    }
}

/** The rows' places before the downloads first answer: a show's head and two episodes. */
@Composable
private fun DownloadPlaceholders() {
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(Modifier.heightIn(min = 72.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            app.kaeri.design.CoverPlate(null, Modifier.size(40.dp, 60.dp))
            Column(Modifier.weight(1f)) {
                TextPlaceholder(0.6f, 24.dp)
                TextPlaceholder(0.35f, 20.dp)
            }
        }
        repeat(2) {
            Column(Modifier.padding(start = 56.dp).heightIn(min = 72.dp), verticalArrangement = Arrangement.Center) {
                TextPlaceholder(0.7f, 24.dp)
                TextPlaceholder(0.45f, 20.dp)
            }
        }
    }
}

/** Where a download stands, in the words the board gives each state. */
@Composable
private fun StateLine(row: DownloadRow, secondsLeft: Long?, rate: Double?) {
    val colors = MaterialTheme.colorScheme
    val style = MaterialTheme.typography.bodyMedium.tabular()
    val context = LocalContext.current
    val sizes = Strings.downloadsLiveProgress(context, Wording.bytes(context, row.downloaded.toLong()), Wording.bytes(context, row.total.toLong()))
    when (row.state) {
        DownloadState.DONE -> Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Check, contentDescription = null, tint = colors.secondary, modifier = Modifier.size(18.dp))
            Text(stringResource(R.string.downloads_caption_done_size, Wording.bytes(context, row.bytesOnDisk.toLong())), style = style, color = colors.onSurfaceVariant)
        }
        DownloadState.ACTIVE -> Text(
            listOfNotNull(
                if (row.total > 0u) sizes else Wording.bytes(context, row.downloaded.toLong()),
                rate?.takeIf { it >= 1 }?.let { Strings.downloadsRate(context, Wording.bytes(context, it.toLong())) },
                secondsLeft?.let { Strings.downloadsTimeLeft(context, Wording.countdown(context, it)) },
            ).joinToString(" · "), // i18n-skip
            style = style,
            color = colors.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        DownloadState.PAUSED -> Text(stringResource(R.string.downloads_caption_paused, sizes), style = style, color = colors.onSurfaceVariant)
        DownloadState.QUEUED -> Text(stringResource(R.string.downloads_caption_queued_slot), style = style, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        // The dot is the error's; the words are haze, the first clause in plain words. The whole
        // error goes to the bug report from the notice the engine posted.
        DownloadState.FAILED -> Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
            StateDot(colors.error)
            Text(
                row.error?.let { stringResource(R.string.downloads_caption_failed_why, plainReason(LocalContext.current, it)) } ?: stringResource(R.string.downloads_caption_failed),
                style = style,
                color = colors.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

/**
 * A watched episode still seeding, as the board says it: "450 MB · 0.2 of 1.0 · 18h left · 90 MB
 * up". The size is what was downloaded, which the ratio is of.
 */
@Composable
private fun SeedItem(row: SeedRow) {
    Column(Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.Center) {
        Text(row.name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
        val context = LocalContext.current
        Text(
            listOfNotNull(
                row.ratio.takeIf { it >= 0.01 }?.let { Wording.bytes(context, (row.uploadedBytes.toDouble() / it).toLong()) },
                Strings.downloadsRatioOf(context, String.format(Locale.getDefault(), "%.1f", row.ratio), String.format(Locale.getDefault(), "%.1f", row.goal)),
                Strings.downloadsTimeLeft(context, Wording.countdown(context, row.leftSecs.toLong())),
                Strings.downloadsUploaded(context, Wording.bytes(context, row.uploadedBytes.toLong())),
            ).joinToString(" · "), // i18n-skip
            style = MaterialTheme.typography.bodyMedium.tabular(),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/**
 * Download, in an episode's long-press menu on its title page: queued at once; Downloaded or
 * Downloading once the queue has it.
 */
@Composable
internal fun DownloadMenuItem(entry: Entry, episode: String, onDismiss: () -> Unit) {
    val store = LocalAppStore.current
    val rows by store.downloads.rows.collectAsStateWithLifecycle()
    val queued = rows.firstOrNull { it.anilistId == entry.id && it.episode == episode }
    DropdownMenuItem(
        text = {
            Text(
                when (queued?.state) {
                    DownloadState.DONE -> stringResource(R.string.episode_download_done)
                    DownloadState.ACTIVE, DownloadState.QUEUED -> stringResource(R.string.downloads_caption_running_plain)
                    DownloadState.PAUSED -> stringResource(R.string.episode_menu_download_paused)
                    else -> stringResource(R.string.episode_menu_download)
                },
            )
        },
        leadingIcon = { Icon(Icons.Downloads, contentDescription = null) },
        enabled = queued == null || queued.state == DownloadState.FAILED,
        onClick = {
            onDismiss()
            store.downloadEpisode(entry.id, episode)
        },
    )
}
