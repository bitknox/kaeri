// A reading series' page (figs 35 to 37, 82): the cover beside the title and facts, the one main
// action with Status and Download beside it, the source in use and the trackers, then the volumes
// and chapters. A phone scrolls it as one list; a tablet in landscape puts the chapters in a pane
// of their own (fig 38). Choose chapters turns the list into a selection with its bar inset where
// the navigation bar was.

package app.kaeri.ui.reading

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateMap
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ObiSilk
import app.kaeri.design.ShowTitle
import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.Medium
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.DownloadPick
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.SeriesAction
import app.kaeri.engine.model.SeriesView
import app.kaeri.engine.model.TrackerMapping
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.i18n.Strings
import app.kaeri.store.ReadChapters
import app.kaeri.store.text
import app.kaeri.ui.AppActions
import app.kaeri.ui.FailedState
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Menu
import app.kaeri.ui.Shared
import app.kaeri.ui.TextPlaceholder
import app.kaeri.ui.kaeriIsFor
import app.kaeri.ui.sharedWork
import app.kaeri.ui.wideWindow

/**
 * What the screenshots open on the series page once it has loaded: a sheet or menu by name
 * (chapter, read-from, tracking, read-again, send, where-stop, choose, status, download), set by
 * the JVM shots and the Fixtures app's `series` extra.
 */
object SeriesShots {
    var open by mutableStateOf<String?>(null)
}

/** The sheets the page holds open, one at a time. */
private sealed interface SeriesSheet {
    data class Chapter(val id: ChapterId) : SeriesSheet
    data object ReadFrom : SeriesSheet
    data object Tracking : SeriesSheet
    data object ReadAgain : SeriesSheet
    data class Send(val chapters: List<ChapterId>) : SeriesSheet
    data object WhereStop : SeriesSheet
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SeriesScreen(work: WorkId, actions: AppActions) {
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    val seriesMap by store.reading.series.collectAsStateWithLifecycle()
    val chapterMap by store.reading.chapters.collectAsStateWithLifecycle()
    val failures by store.reading.failures.collectAsStateWithLifecycle()
    LaunchedEffect(work) {
        store.reading.loadSeries(work)
        store.bookSources.loadForWork(work)
    }
    val series = seriesMap[work]
    val chapters = chapterMap[work]
    val volumes = chapters?.volumes.orEmpty()
    var chosen by remember { mutableStateOf<Set<ChapterId>?>(null) }
    var sheet by remember { mutableStateOf<SeriesSheet?>(null) }
    var statusOpen by remember { mutableStateOf(false) }
    var downloadOpen by remember { mutableStateOf(false) }
    var moreOpen by remember { mutableStateOf(false) }
    var newestFirst by rememberSaveable { mutableStateOf(true) }
    // Volumes the person opened or closed; the others are open while not all read.
    val folds = remember { mutableStateMapOf<String, Boolean>() }
    val wide = wideWindow()

    // The screenshots' way in: a sheet or menu named once the page has what it shows.
    val shot = SeriesShots.open
    LaunchedEffect(shot, series != null, chapters != null) {
        if (shot == null || series == null || chapters == null) return@LaunchedEffect
        val rows = volumes.chapterRows()
        when (shot) {
            "chapter" -> (rows.firstOrNull { it.versions > 1u } ?: rows.lastOrNull())?.let { sheet = SeriesSheet.Chapter(it.id) }
            "read-from" -> sheet = SeriesSheet.ReadFrom
            "tracking" -> sheet = SeriesSheet.Tracking
            "read-again" -> sheet = SeriesSheet.ReadAgain
            "send" -> sheet = SeriesSheet.Send(volumes.downloaded().map { it.id }.ifEmpty { rows.takeLast(3).map { it.id } })
            "where-stop" -> sheet = SeriesSheet.WhereStop
            "choose" -> chosen = volumes.chooseStart()
            "status" -> statusOpen = true
            "download" -> downloadOpen = true
        }
        SeriesShots.open = null
    }

    BackHandler(enabled = chosen != null) { chosen = null }

    val failure = failures[ReadingAbout.SERIES]
    val coverKey = readerCoverKey("series", work)

    Scaffold(
        containerColor = MaterialTheme.colorScheme.surface,
        topBar = {
            val choosing = chosen
            if (choosing != null && series != null) {
                TopAppBar(
                    title = { Text(Strings.seriesChosen(context, choosing.size)) },
                    navigationIcon = {
                        IconButton(onClick = { chosen = null }) { Icon(Icons.Clear, contentDescription = Strings.actionCancel(context)) }
                    },
                    actions = {
                        TextButton(
                            onClick = { chosen = volumes.allUnread() },
                            colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
                        ) { Text(Strings.seriesChooseAllUnread(context)) }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.surface),
                )
            } else {
                TopAppBar(
                    title = {},
                    navigationIcon = {
                        IconButton(onClick = navigator::back) { Icon(Icons.Back, contentDescription = Strings.actionBack(context)) }
                    },
                    actions = {
                        if (series != null) {
                            Box {
                                IconButton(onClick = { moreOpen = true }) { Icon(Icons.More, contentDescription = Strings.actionMore(context)) }
                                Menu(expanded = moreOpen, onDismissRequest = { moreOpen = false }) {
                                    DropdownMenuItem(text = { Text(Strings.seriesTracking(context)) }, onClick = {
                                        moreOpen = false
                                        sheet = SeriesSheet.Tracking
                                    })
                                    DropdownMenuItem(text = { Text(Strings.seriesReadFromMenu(context)) }, onClick = {
                                        moreOpen = false
                                        sheet = SeriesSheet.ReadFrom
                                    })
                                    if (volumes.chapterRows().any { it.state !is ChapterReadState.Unread }) {
                                        DropdownMenuItem(text = { Text(Strings.seriesReadAgainMenu(context)) }, onClick = {
                                            moreOpen = false
                                            sheet = SeriesSheet.ReadAgain
                                        })
                                    }
                                }
                            }
                        }
                    },
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.surface),
                )
            }
        },
        bottomBar = {
            val choosing = chosen
            if (choosing != null && series != null) {
                ChooseBar(
                    series,
                    choosing,
                    onMarkRead = {
                        store.reading.markRead(work, choosing.toList(), true)
                        chosen = null
                    },
                    onDownload = {
                        store.reading.download(work, DownloadPick.Chapters(choosing.toList()))
                        chosen = null
                    },
                )
            }
        },
    ) { padding ->
        when {
            series == null && failure != null -> FailedState(
                title = Strings.titleLoadFailed(context),
                reason = failureWords(context, failure),
                retry = { store.reading.loadSeries(work) },
                modifier = Modifier.padding(padding),
            )
            series == null -> SeriesLoading(Modifier.padding(padding))
            else -> {
                val top: @Composable () -> Unit = {
                    SeriesHeader(series, wide, coverKey, onCover = {
                        actions.openReader(work, series.action.chapter, series.cover, coverKey)
                    })
                    Spacer(Modifier.height(20.dp))
                    SeriesActions(
                        series = series,
                        chapters = chapters,
                        choosing = chosen != null,
                        statusOpen = statusOpen,
                        downloadOpen = downloadOpen,
                        onStatus = { statusOpen = it },
                        onDownload = { downloadOpen = it },
                        onMain = { action ->
                            if (action is SeriesAction.ReadAgain) {
                                sheet = SeriesSheet.ReadAgain
                            } else {
                                actions.openReader(work, action.chapter, series.cover, coverKey)
                            }
                        },
                        onReadFrom = { sheet = SeriesSheet.ReadFrom },
                        onChoose = { chosen = volumes.chooseStart() },
                        onSend = { sheet = SeriesSheet.Send(it) },
                    )
                    Spacer(Modifier.height(12.dp))
                    SeriesPanel(series, onReadFrom = { sheet = SeriesSheet.ReadFrom }, onTracking = { sheet = SeriesSheet.Tracking })
                    if (series.askWhereStopped != null) {
                        WhereStopNote(series, onAsk = { sheet = SeriesSheet.WhereStop }, modifier = Modifier.padding(top = 16.dp))
                    }
                }
                val list: LazyListScope.() -> Unit = {
                    seriesChapters(
                        series = series,
                        chapters = chapters,
                        newestFirst = newestFirst,
                        onOrder = { newestFirst = it },
                        folds = folds,
                        chosen = chosen,
                        onChosen = { chosen = it },
                        onOpen = { row -> actions.openReader(work, row.id, series.cover, null) },
                        onLong = { row -> sheet = SeriesSheet.Chapter(row.id) },
                        onRetry = { store.reading.loadChapters(work, chapters?.edition?.sourceId) },
                        onReadFrom = { sheet = SeriesSheet.ReadFrom },
                    )
                    if (chosen == null && series.recommended.isNotEmpty()) {
                        item(key = "recommended") { SeriesRecommended(series) }
                    }
                }
                if (wide) {
                    Row(Modifier.padding(padding).fillMaxSize()) {
                        Column(
                            Modifier
                                .width(420.dp)
                                .fillMaxHeight()
                                .verticalScroll(rememberScrollState())
                                .padding(start = 24.dp, end = 24.dp, bottom = 24.dp),
                        ) { top() }
                        VerticalDivider(color = ObiSilk.FaintRule)
                        LazyColumn(Modifier.weight(1f).fillMaxHeight(), contentPadding = PaddingValues(bottom = 24.dp)) { list() }
                    }
                } else {
                    LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
                        if (chosen == null) {
                            item(key = "top") { Column(Modifier.padding(horizontal = 16.dp)) { top() } }
                        }
                        list()
                    }
                }
            }
        }
    }

    when (val open = sheet) {
        null -> {}
        is SeriesSheet.Chapter -> {
            val row = volumes.chapterRows().firstOrNull { it.id == open.id }
            if (series != null && row != null) {
                ChapterSheet(series, row, actions, onDismiss = { sheet = null }, onSend = { sheet = SeriesSheet.Send(it) })
            }
        }
        SeriesSheet.ReadFrom -> ReadFromSheet(work) { sheet = null }
        SeriesSheet.Tracking -> TrackingSheet(work) { sheet = null }
        SeriesSheet.ReadAgain -> series?.let { ReadAgainDialog(it) { sheet = null } }
        is SeriesSheet.Send -> SendToEreaderSheet(work, open.chapters, actions) { sheet = null }
        SeriesSheet.WhereStop -> series?.let { WhereStopSheet(it) { sheet = null } }
    }
}

/** While the series loads: the cover's plate and the title's lines where they will be. */
@Composable
private fun SeriesLoading(modifier: Modifier) {
    Row(modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        ReadCover(null, 112.dp)
        Column(Modifier.weight(1f).padding(top = 4.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            TextPlaceholder(0.8f, 28.dp)
            TextPlaceholder(0.6f)
            TextPlaceholder(0.5f)
        }
    }
}

/** The cover beside the title, the facts and the rhythm; on a tablet the synopsis under them. */
@Composable
private fun SeriesHeader(series: SeriesView, wide: Boolean, coverKey: String, onCover: () -> Unit) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val canOpen = series.action.chapter != null
    Column {
        Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            ReadCover(
                series.cover,
                112.dp,
                Modifier
                    .sharedWork(Shared.Cover, series.work)
                    .readerZoomSource(coverKey)
                    .then(if (canOpen) Modifier.clickable(onClickLabel = actionWords(context, series.action), onClick = onCover) else Modifier),
            )
            Column(Modifier.weight(1f).padding(top = 2.dp)) {
                Text(
                    series.titles.display,
                    style = ShowTitle.page,
                    color = colors.onSurface,
                    modifier = Modifier.semantics { heading() },
                )
                val facts = series.facts.map { it.text(context) }
                if (facts.isNotEmpty()) {
                    Text(
                        facts.joinToString(" · "), // i18n-skip
                        style = MaterialTheme.typography.bodyMedium,
                        color = colors.onSurfaceVariant,
                        modifier = Modifier.padding(top = 6.dp),
                    )
                }
                series.rhythm?.let {
                    Text(
                        it.text(context),
                        style = MaterialTheme.typography.bodyMedium,
                        color = colors.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
            }
        }
        val synopsis = series.synopsis?.replace(Regex("\\s*\\n+\\s*"), " ")
        if (wide && !synopsis.isNullOrBlank()) {
            var open by rememberSaveable { mutableStateOf(false) }
            Text(
                synopsis,
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                maxLines = if (open) Int.MAX_VALUE else 4,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 16.dp).clickable { open = !open },
            )
        }
    }
}

/**
 * The one main action, amber, 56 high, with Status and Download outlined side by side under it.
 * With no source, or a source that failed, Read from another source takes its place as a
 * secondary button, since reading from here is not possible.
 */
@Composable
private fun SeriesActions(
    series: SeriesView,
    chapters: ReadChapters?,
    choosing: Boolean,
    statusOpen: Boolean,
    downloadOpen: Boolean,
    onStatus: (Boolean) -> Unit,
    onDownload: (Boolean) -> Unit,
    onMain: (SeriesAction) -> Unit,
    onReadFrom: () -> Unit,
    onChoose: () -> Unit,
    onSend: (List<ChapterId>) -> Unit,
) {
    val context = LocalContext.current
    val words = SeriesWords.action(context, series.action)
    val failed = chapters?.failure != null
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (words == null || failed) {
            FilledTonalButton(onClick = onReadFrom, modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp)) {
                Text(Strings.seriesReadFromAnother(context), style = MaterialTheme.typography.titleMedium)
            }
        } else {
            Button(
                onClick = { onMain(series.action) },
                enabled = !choosing,
                modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp),
            ) { Text(words, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold) }
        }
        if (!failed && words != null) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.weight(1f)) {
                    MenuButton(series.list?.let { listWords(context, it) } ?: Strings.seriesAddToList(context), enabled = !choosing) { onStatus(true) }
                    StatusMenu(series, statusOpen) { onStatus(false) }
                }
                Box(Modifier.weight(1f)) {
                    MenuButton(Strings.seriesDownload(context), enabled = !choosing && chapters != null) { onDownload(true) }
                    DownloadMenu(series, chapters?.volumes.orEmpty(), downloadOpen, onDismiss = { onDownload(false) }, onChoose = onChoose, onSend = onSend)
                }
            }
        }
    }
}

/** An outlined button that opens a menu: its words with the down arrow. */
@Composable
private fun MenuButton(text: String, enabled: Boolean, onClick: () -> Unit) {
    OutlinedButton(
        onClick = onClick,
        enabled = enabled,
        modifier = Modifier.fillMaxWidth().heightIn(min = 48.dp),
        colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
    ) {
        Text(text, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Spacer(Modifier.width(4.dp))
        Icon(Icons.Down, contentDescription = null, modifier = Modifier.size(18.dp))
    }
}

/**
 * The rows under the actions: the source in use with Change, the trackers and how far they have
 * it, and, with Both, the same story as a show.
 */
@Composable
private fun SeriesPanel(series: SeriesView, onReadFrom: () -> Unit, onTracking: () -> Unit) {
    val context = LocalContext.current
    val store = LocalAppStore.current
    val navigator = LocalNavigator.current
    val tracking by store.reading.tracking.collectAsStateWithLifecycle()
    val forWork by store.bookSources.forWork.collectAsStateWithLifecycle()
    val rows = tracking[series.work] ?: series.tracking
    val followed = rows.filter { it.mapping == TrackerMapping.Mapped }
    val editions = forWork[series.work]?.editions.orEmpty()
    val both = kaeriIsFor() == KaeriIsFor.BOTH
    Column {
        series.edition?.let { edition ->
            PanelRule()
            PanelRow(
                Strings.seriesReadingFrom(context),
                SeriesWords.source(context, edition, (editions.size - 1).coerceAtLeast(0)),
                Strings.seriesChange(context),
                onReadFrom,
            )
        }
        val first = followed.firstOrNull()
        if (first != null) {
            PanelRule()
            PanelRow(SeriesWords.trackers(followed), SeriesWords.progress(context, first, series), null, onTracking)
        } else if (rows.isNotEmpty()) {
            PanelRule()
            PanelRow(SeriesWords.trackers(rows), Strings.seriesTrackerAsk(context), Strings.seriesChange(context), onTracking)
        }
        val other = series.otherMedium
        if (both && other != null) {
            PanelRule()
            val anilist = other.work.substringAfter("a:", "").toUIntOrNull()
            Row(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 64.dp)
                    .clickable {
                        if (other.medium == Medium.ANIME && anilist != null) {
                            navigator.openTitle(anilist, "series-other")
                        } else {
                            navigator.openSeries(other.work, "series-other")
                        }
                    }
                    .padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f)) {
                    Text(other.relation.text(context), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(other.title, style = MaterialTheme.typography.bodyLarge)
                }
                Icon(Icons.Chevron, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
            }
        }
    }
}

@Composable
private fun PanelRule() {
    HorizontalDivider(color = ObiSilk.FaintRule)
}

/** A panel row: its label small in haze over its value, and an action in silk at the end. */
@Composable
private fun PanelRow(label: String, value: String, action: String?, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .clickable(onClick = onClick)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(value, style = MaterialTheme.typography.bodyLarge)
        }
        if (action != null) {
            Text(action, style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(start = 12.dp))
        }
    }
}

/**
 * The chapters: their heading with the order, the failure when the source failed (with what is
 * downloaded still listed), and each volume with its chapters under it while it is open.
 */
private fun LazyListScope.seriesChapters(
    series: SeriesView,
    chapters: ReadChapters?,
    newestFirst: Boolean,
    onOrder: (Boolean) -> Unit,
    folds: SnapshotStateMap<String, Boolean>,
    chosen: Set<ChapterId>?,
    onChosen: (Set<ChapterId>) -> Unit,
    onOpen: (ChapterRow) -> Unit,
    onLong: (ChapterRow) -> Unit,
    onRetry: () -> Unit,
    onReadFrom: () -> Unit,
) {
    val novel = series.medium == Medium.NOVEL
    if (chosen == null) {
        item(key = "chapters-heading") { ChaptersHeading(novel, newestFirst, onOrder) }
    }
    if (chapters == null) {
        items(4, key = { "placeholder-$it" }) {
            Column(
                Modifier.fillMaxWidth().heightIn(min = 72.dp).padding(horizontal = 16.dp, vertical = 14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                TextPlaceholder(0.5f)
                TextPlaceholder(0.3f)
            }
        }
        return
    }
    val failure = chapters.failure
    // With the source failed, what is downloaded still opens: only those chapters, under no volume.
    val groups = if (failure != null) {
        listOfNotNull(
            chapters.volumes.downloaded().takeIf { it.isNotEmpty() }?.let { VolumeGroup(volume = null, title = null, chapters = it, allRead = false) },
        )
    } else {
        chapters.volumes
    }
    if (failure != null) {
        item(key = "chapters-failed") { ChaptersFailed(series, failure, groups.chapterRows().size, onRetry) }
    } else if (groups.isEmpty()) {
        item(key = "chapters-none") {
            val context = LocalContext.current
            PlainEmpty(
                Strings.seriesNoChaptersTitle(context),
                Strings.seriesNoChaptersLine(context),
                action = Strings.seriesReadFromAnother(context),
                onAction = onReadFrom,
            )
        }
    }
    val ordered = if (newestFirst) groups.reversed() else groups
    val single = failure != null || (ordered.size == 1 && ordered.first().volume == null)
    ordered.forEachIndexed { index, group ->
        val key = group.volume ?: "none-$index"
        val open = single || (folds[key] ?: !group.allRead)
        if (!single) {
            item(key = "volume-$key") {
                val context = LocalContext.current
                val (name, rest) = SeriesWords.volume(context, group, novel)
                Column {
                    SeriesRule()
                    VolumeRowItem(name, rest, open, onToggle = { folds[key] = !open })
                }
            }
        }
        if (open) {
            val rows = if (newestFirst) group.chapters.reversed() else group.chapters
            items(rows, key = { "chapter-${it.id}" }) { row -> ChapterItem(series, row, chosen, onChosen, onOpen, onLong) }
        }
    }
}

@Composable
private fun ChapterItem(
    series: SeriesView,
    row: ChapterRow,
    chosen: Set<ChapterId>?,
    onChosen: (Set<ChapterId>) -> Unit,
    onOpen: (ChapterRow) -> Unit,
    onLong: (ChapterRow) -> Unit,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val work = series.work
    val read = row.state is ChapterReadState.Read
    Column {
        SeriesRule()
        ChapterRowItem(
            row = row,
            chosen = chosen?.contains(row.id),
            onClick = {
                if (chosen != null) onChosen(if (row.id in chosen) chosen - row.id else chosen + row.id) else onOpen(row)
            },
            onLongClick = { onLong(row) },
            onStop = { store.reading.cancelDownloads(work, listOf(row.id)) },
            onRetry = { store.reading.download(work, DownloadPick.Chapters(listOf(row.id))) },
            actions = listOf(
                CustomAccessibilityAction(if (read) Strings.seriesMarkUnread(context) else Strings.seriesMarkRead(context)) {
                    store.reading.markRead(work, listOf(row.id), !read)
                    true
                },
                CustomAccessibilityAction(Strings.seriesMarkReadUpTo(context)) {
                    store.reading.markReadUpTo(work, row.id)
                    true
                },
            ),
        )
    }
}

/** "Chapters" or "Volumes", with the order at the end: Newest first or Oldest first. */
@Composable
private fun ChaptersHeading(novel: Boolean, newestFirst: Boolean, onOrder: (Boolean) -> Unit) {
    val context = LocalContext.current
    var open by remember { mutableStateOf(false) }
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(start = 16.dp, end = 4.dp, top = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            if (novel) Strings.seriesVolumes(context) else Strings.seriesChapters(context),
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.weight(1f).semantics { heading() },
        )
        Box {
            TextButton(
                onClick = { open = true },
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.onSurface),
            ) {
                Text(if (newestFirst) Strings.seriesNewestFirst(context) else Strings.seriesOldestFirst(context))
                Spacer(Modifier.width(2.dp))
                Icon(Icons.Down, contentDescription = null, modifier = Modifier.size(18.dp))
            }
            Menu(expanded = open, onDismissRequest = { open = false }) {
                listOf(true, false).forEach { newest ->
                    DropdownMenuItem(
                        text = { Text(if (newest) Strings.seriesNewestFirst(context) else Strings.seriesOldestFirst(context)) },
                        trailingIcon = if (newest == newestFirst) {
                            { Icon(Icons.Check, contentDescription = null, modifier = Modifier.size(20.dp)) }
                        } else {
                            null
                        },
                        onClick = {
                            onOrder(newest)
                            open = false
                        },
                    )
                }
            }
        }
    }
}

/** The source failed (fig 82): what failed and where, why, what still opens, and Try again. */
@Composable
private fun ChaptersFailed(series: SeriesView, failure: ReadingFailure, downloaded: Int, onRetry: () -> Unit) {
    val context = LocalContext.current
    val source = series.edition?.sourceName.orEmpty()
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Icon(Icons.Problem, contentDescription = null, tint = ObiSilk.Vermilion, modifier = Modifier.padding(top = 2.dp).size(20.dp))
        Column(Modifier.weight(1f)) {
            Text(Strings.seriesChaptersFailed(context, source), style = MaterialTheme.typography.bodyLarge)
            val line = listOfNotNull(
                failureWords(context, failure),
                if (downloaded > 0) Strings.readSeriesDownloadedOpen(context, downloaded) else null,
            ).joinToString(" ") // i18n-skip
            Text(line, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 4.dp))
            FilledTonalButton(onClick = onRetry, modifier = Modifier.padding(top = 12.dp)) { Text(Strings.actionTryAgain(context)) }
        }
    }
}

/** More like this: the engine's recommendations as covers to open. */
@Composable
private fun SeriesRecommended(series: SeriesView) {
    val context = LocalContext.current
    Column(Modifier.padding(top = 24.dp)) {
        ReadShelfHeading(Strings.moreLikeThis(context))
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            items(series.recommended, key = { it.work }) { tile -> ReadTile(tile, 112.dp, from = "series-recommended") }
        }
    }
}
