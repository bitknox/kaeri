// Updates: new chapters by day, newest first, from series the person follows. Unread rows are silk
// and bold with the water dot, read rows haze. A row opens the reader at its chapter; its cover
// opens the series; a long press offers Mark read, Download and Open series. Download new is the
// one tool, a text button in the top bar: nothing here is amber.

package app.kaeri.ui.reading

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ShowTitle
import app.kaeri.design.tabular
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.DownloadPick
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.UpdateDay
import app.kaeri.engine.model.UpdateFailure
import app.kaeri.engine.model.UpdateRow
import app.kaeri.engine.model.UpdatesView
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.ui.AfterWait
import app.kaeri.ui.AppActions
import app.kaeri.ui.Destination
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Menu
import app.kaeri.ui.Refreshable
import app.kaeri.ui.ScreenFrame
import app.kaeri.ui.Shared
import app.kaeri.ui.TextPlaceholder
import app.kaeri.ui.WaterDot
import app.kaeri.ui.itemMotion
import app.kaeri.ui.kaeriIsFor
import app.kaeri.ui.profileAccount
import app.kaeri.ui.sharedWork
import app.kaeri.ui.wideWindow

@Composable
internal fun UpdatesScreen(actions: AppActions) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val updates by store.reading.updates.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val user = trackers.values.firstOrNull { it.connected }?.user
    LaunchedEffect(Unit) { store.reading.loadUpdates() }
    val view = updates
    ScreenFrame(
        title = stringResource(R.string.nav_updates),
        subtitle = checkedWords(view),
        account = if (wideWindow()) null else profileAccount(user, actions.openSettings),
        // Beside Watch, Updates opens from reading's Home and goes back to it.
        onBack = if (kaeriIsFor() == KaeriIsFor.BOTH) ({ navigator.go(Destination.Read) }) else null,
        actions = {
            TextButton(
                onClick = { store.reading.downloadNew() },
                enabled = (view?.unread ?: 0u) > 0u,
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
            ) { Text(Strings.updatesDownloadNew(context)) }
        },
    ) { padding ->
        val top = padding.calculateTopPadding()
        Refreshable({ store.reading.checkUpdates() }, top = top) {
            LazyColumn(
                Modifier.fillMaxSize(),
                contentPadding = PaddingValues(top = top, bottom = 24.dp),
                userScrollEnabled = view != null,
            ) {
                if (view == null) {
                    item(key = "loading") { AfterWait { UpdatesPlaceholder() } }
                } else {
                    updateItems(view, actions)
                }
            }
        }
    }
}

/** "Checked 12 min ago", or that it is checking now. */
@Composable
private fun checkedWords(view: UpdatesView?): String? {
    val context = LocalContext.current
    view ?: return null
    if (view.checking) return Strings.chapterUpdatesChecking(context)
    return view.checkedAt?.let { Strings.updatesChecked(context, agoWords(context, it)) }
}

private fun androidx.compose.foundation.lazy.LazyListScope.updateItems(view: UpdatesView, actions: AppActions) {
    items(view.failures, key = { "f-${it.work}" }, contentType = { "failure" }) { failure ->
        Box(itemMotion().readingColumn()) { UpdateFailureRow(failure) }
    }
    if (view.days.isEmpty()) {
        item(key = "empty", contentType = "empty") {
            val context = LocalContext.current
            Box(itemMotion().readingColumn()) {
                PlainEmpty(Strings.updatesEmptyTitle(context), Strings.chapterUpdatesEmptyLine(context), Modifier.padding(top = 24.dp))
            }
        }
    }
    for (day in view.days) {
        item(key = "d-${day.day}", contentType = "day") {
            Box(itemMotion().readingColumn()) { DayHeading(day) }
        }
        items(day.rows, key = { "r-${day.day}-${it.chapter.id}" }, contentType = { "row" }) { row ->
            Column(itemMotion().readingColumn()) {
                HorizontalDivider(Modifier.padding(horizontal = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
                UpdateRowView(row, actions)
            }
        }
    }
}

/** Updates keeps to a reading width on a wide window, from the leading edge. */
private fun Modifier.readingColumn(): Modifier = fillMaxWidth().wrapContentWidth(Alignment.Start).widthIn(max = 820.dp).fillMaxWidth()

@Composable
private fun DayHeading(day: UpdateDay) {
    val context = LocalContext.current
    Text(
        dayWords(context, day.day),
        style = MaterialTheme.typography.titleSmall.copy(fontWeight = FontWeight.Bold),
        color = MaterialTheme.colorScheme.onSurface,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 20.dp, bottom = 8.dp).semantics { heading() },
    )
}

/** "Chapter 88 · Bluefinch Group", "Chapter 43 · 2 versions": the label, then the versions. */
private fun chapterLine(context: android.content.Context, row: UpdateRow, unread: Boolean): String {
    val chapter = row.chapter
    // The water dot stands only beside words that say the chapter is new.
    val parts = if (unread) mutableListOf(context.getString(R.string.chapter_state_new), chapter.label) else mutableListOf(chapter.label)
    val label = chapter.versionLabel
    if (chapter.versions > 1u) parts += Strings.chapterUpdatesVersions(context, chapter.versions.toInt())
    else if (!label.isNullOrBlank()) parts += label
    return parts.joinToString(" · ") // i18n-skip
}

/** A download's state in words, for a row that has one. */
private fun downloadWords(context: android.content.Context, state: ChapterDownloadState): String = when (state) {
    ChapterDownloadState.Done -> context.getString(R.string.chapter_state_downloaded)
    ChapterDownloadState.Queued -> context.getString(R.string.chapter_state_queued)
    ChapterDownloadState.WaitsForWifi -> context.getString(R.string.chapter_state_wifi)
    is ChapterDownloadState.Downloading -> Strings.chapterStateDownloading(context, state.done.toInt(), state.total.toInt())
    is ChapterDownloadState.Failed -> Strings.chapterStateFailed(context, failureWords(context, state.failure))
}

/** A new chapter: the cover, the title, the chapter with its source's words, and the time it came out. */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun UpdateRowView(row: UpdateRow, actions: AppActions) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val haptics = LocalHapticFeedback.current
    val colors = MaterialTheme.colorScheme
    val tile = row.tile
    val chapter = row.chapter
    val unread = chapter.state !is ChapterReadState.Read
    val key = readerCoverKey("updates-${chapter.id}", tile.work)
    var menu by remember { mutableStateOf(false) }
    Box {
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 72.dp)
                .padding(start = 16.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ReadCover(
                tile.cover,
                40.dp,
                Modifier
                    .sharedWork(Shared.Cover, tile.work, "updates")
                    .readerZoomSource(key)
                    .clickable(onClickLabel = Strings.chapterUpdatesOpenSeries(context, tile.title), role = Role.Button) {
                        navigator.openSeries(tile.work, "updates")
                    },
            )
            Row(
                Modifier
                    .weight(1f)
                    .heightIn(min = 60.dp)
                    .semantics(mergeDescendants = true) {}
                    .combinedClickable(
                        onClickLabel = stringResource(R.string.chapter_updates_open_chapter_hint),
                        onLongClickLabel = stringResource(R.string.action_more),
                        onLongClick = {
                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            menu = true
                        },
                        onClick = { actions.openReader(tile.work, chapter.id, tile.cover, key) },
                    )
                    .padding(start = 12.dp, end = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        tile.title,
                        style = ShowTitle.row.copy(fontWeight = if (unread) FontWeight.SemiBold else FontWeight.Normal),
                        color = if (unread) colors.onSurface else colors.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        if (unread) WaterDot()
                        Text(
                            chapterLine(context, row, unread),
                            style = MaterialTheme.typography.bodyMedium.tabular(),
                            color = if (unread) colors.onSurface else colors.onSurfaceVariant,
                        )
                    }
                    chapter.download?.let { state ->
                        Text(downloadWords(context, state), style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
                    }
                }
                chapter.published?.let { at ->
                    Text(clockWords(at), style = MaterialTheme.typography.bodyMedium.tabular(), color = colors.onSurfaceVariant)
                }
            }
        }
        Box(Modifier.align(Alignment.TopEnd).padding(end = 16.dp)) {
            Menu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(
                    text = { Text(if (unread) Strings.seriesMarkRead(context) else Strings.seriesMarkUnread(context)) },
                    leadingIcon = { Icon(Icons.Check, null) },
                    onClick = {
                        menu = false
                        store.reading.markRead(tile.work, listOf(chapter.id), unread)
                    },
                )
                if (chapter.download == null) {
                    DropdownMenuItem(
                        text = { Text(Strings.seriesDownload(context)) },
                        leadingIcon = { Icon(Icons.Downloads, null) },
                        onClick = {
                            menu = false
                            store.reading.download(tile.work, DownloadPick.Chapters(listOf(chapter.id)))
                        },
                    )
                }
                DropdownMenuItem(
                    text = { Text(Strings.chapterUpdatesOpenSeriesMenu(context)) },
                    leadingIcon = { Icon(Icons.External, null) },
                    onClick = {
                        menu = false
                        navigator.openSeries(tile.work, "updates")
                    },
                )
            }
        }
    }
}

/** A series the check could not ask: its title, the reason, and Try again. */
@Composable
private fun UpdateFailureRow(failure: UpdateFailure) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(failure.title, style = ShowTitle.row.copy(fontWeight = FontWeight.SemiBold), color = colors.onSurface)
            Text(failureWords(context, failure.failure), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
        }
        OutlinedButton(
            onClick = { store.reading.checkUpdates() },
            colors = ButtonDefaults.outlinedButtonColors(contentColor = colors.onSurface),
        ) { Text(Strings.readerTryAgain(context)) }
    }
}

/** Updates before it arrives: a day's heading and four rows' plates. */
@Composable
private fun UpdatesPlaceholder() {
    Column(Modifier.padding(horizontal = 16.dp)) {
        TextPlaceholder(0.2f, 44.dp)
        repeat(4) {
            Row(Modifier.padding(vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.widthIn(min = 40.dp, max = 40.dp)) { TextPlaceholder(1f, 60.dp) }
                Column(Modifier.weight(1f)) {
                    TextPlaceholder(0.6f, 22.dp)
                    TextPlaceholder(0.4f, 20.dp)
                }
            }
        }
    }
}
