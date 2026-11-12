// A reading series' page (figs 35 to 37, 82): the cover beside the title and facts, the one main
// action with Status and Download beside it, the source in use and the trackers, then the volumes
// and chapters. A phone scrolls it as one list; a tablet in landscape puts the chapters in a pane
// of their own (fig 38). Choose chapters turns the list into a selection with its bar inset where
// the navigation bar was.

package app.kaeri.ui.reading

import androidx.activity.compose.BackHandler
import app.kaeri.ui.synopsisCut
import app.kaeri.ui.SynopsisCuts
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.BoxWithConstraints
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
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.focusable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalDensity
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
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.Motion
import app.kaeri.design.centredOffset
import app.kaeri.design.ObiSilk
import app.kaeri.design.ShowTitle
import app.kaeri.design.SwapFade
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
import app.kaeri.engine.model.WorkTile
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
import app.kaeri.ui.itemMotion
import app.kaeri.ui.kaeriIsFor
import app.kaeri.ui.labelFor
import app.kaeri.ui.sharedWork
import app.kaeri.ui.wideWindow
import kotlinx.coroutines.delay

/**
 * What the screenshots open on the series page once it has loaded: a sheet or menu by name
 * (chapter, read-from, group, tracking, read-again, send, where-stop, choose, status, remove, download),
 * set by the JVM shots and the Fixtures app's `series` extra.
 */
object SeriesShots {
    var open by mutableStateOf<String?>(null)

    /** The status menu opens on Remove from lists' question rather than the lists. */
    var removing by mutableStateOf(false)
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
    // The chapters' placeholders come only once the wait is long enough to notice (native-ux.md).
    var lateChapters by remember(work) { mutableStateOf(false) }
    LaunchedEffect(work, chapters == null) {
        lateChapters = false
        if (chapters == null) {
            delay(600)
            lateChapters = true
        }
    }
    val volumes = chapters?.volumes.orEmpty()
    var chosen by remember { mutableStateOf<Set<ChapterId>?>(null) }
    // The reader's place, which Choose chapters opens centred on.
    var opening by remember { mutableStateOf<OpenFocus?>(null) }
    fun startChoosing() {
        chosen = volumes.chooseStart()
        opening = volumes.readerPlace()?.let(::OpenFocus)
    }
    var sheet by remember { mutableStateOf<SeriesSheet?>(null) }
    var statusOpen by remember { mutableStateOf(false) }
    var downloadOpen by remember { mutableStateOf(false) }
    var moreOpen by remember { mutableStateOf(false) }
    // The group picker: a menu from Change on a tablet, a sheet on a phone, a page on e-ink.
    var groupOpen by remember { mutableStateOf(false) }
    // The order last chosen, on any series: the engine sends chapters in reading order.
    var newestFirst by remember { mutableStateOf(ChapterOrder.newestFirst(context)) }
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
            "group" -> groupOpen = true
            "tracking" -> sheet = SeriesSheet.Tracking
            "read-again" -> sheet = SeriesSheet.ReadAgain
            "send" -> sheet = SeriesSheet.Send(volumes.downloaded().map { it.id }.ifEmpty { rows.takeLast(3).map { it.id } })
            "where-stop" -> sheet = SeriesSheet.WhereStop
            "choose" -> startChoosing()
            "status" -> statusOpen = true
            "remove" -> {
                SeriesShots.removing = true
                statusOpen = true
            }
            "download" -> downloadOpen = true
        }
        SeriesShots.open = null
    }

    BackHandler(enabled = chosen != null) { chosen = null }

    // The demo's works have their own page: the credit, Read chapter 1 and the other work.
    val demo = series?.demoWork
    if (series != null && demo != null) return DemoSeriesScreen(series, demo, actions)

    val failure = failures[ReadingAbout.SERIES]
    val coverKey = readerCoverKey("series", work)

    // The series' menu: Tracking, Read from and Read again.
    val overflow: @Composable () -> Unit = {
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
    }

    Scaffold(
        // The page grows out of the tile it was opened from and shrinks back into it, as a show's does.
        modifier = Modifier.fillMaxSize().sharedWork(Shared.Page, work, ground = MaterialTheme.colorScheme.surface),
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
            } else if (einkLarge) {
                // A large e-ink panel: where Back goes in words at the page's margin, as every
                // pushed paper page has it; the menu keeps Tracking, Read from and Read again.
                val under = navigator.under()
                val answer = kaeriIsFor()
                EinkSeriesBack(
                    back = under?.let { androidx.compose.ui.res.stringResource(it.labelFor(answer)) } ?: Strings.einkBack(context),
                    onBack = navigator::back,
                    menu = overflow,
                )
            } else {
                TopAppBar(
                    title = {},
                    navigationIcon = {
                        IconButton(onClick = navigator::back) { Icon(Icons.Back, contentDescription = Strings.actionBack(context)) }
                    },
                    actions = { overflow() },
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
            series == null && failure != null -> Column(Modifier.padding(padding)) {
                // The header that was known stays above why the rest did not come.
                store.reading.knownTile(work)?.let { KnownSeriesHeader(it) }
                FailedState(
                    title = Strings.titleLoadFailed(context),
                    reason = failureWords(context, failure),
                    retry = { store.reading.loadSeries(work) },
                    modifier = Modifier.weight(1f),
                )
            }
            series == null -> store.reading.knownTile(work)?.let { tile ->
                // What the tile tapped already showed, at once and where the header puts it.
                Box(Modifier.padding(padding)) { KnownSeriesHeader(tile) }
            } ?: SeriesLoading(Modifier.padding(padding))
            // A large e-ink panel: the board's page, chapters in columns a page at a time (EinkSeries.kt).
            einkLarge -> EinkSeriesLarge(
                series = series,
                chapters = chapters,
                late = lateChapters,
                newestFirst = newestFirst,
                onOrder = {
                    newestFirst = it
                    ChapterOrder.keep(context, it)
                },
                chosen = chosen,
                onChosen = { chosen = it },
                onCover = { actions.openReader(work, series.action.chapter, series.cover, null) },
                onMain = { action ->
                    if (action is SeriesAction.ReadAgain) sheet = SeriesSheet.ReadAgain else actions.openReader(work, action.chapter, series.cover, null)
                },
                onReadFrom = { sheet = SeriesSheet.ReadFrom },
                onGroup = { groupOpen = true },
                onStatus = { statusOpen = true },
                onDownload = { downloadOpen = true },
                onChoose = { chosen = volumes.chooseStart() },
                onOpen = { row -> actions.openReader(work, row.id, series.cover, null) },
                onLong = { row -> sheet = SeriesSheet.Chapter(row.id) },
                onRetry = { store.reading.loadChapters(work, chapters?.edition?.sourceId) },
                statusMenu = { StatusMenu(series, statusOpen) { statusOpen = false } },
                downloadMenu = {
                    DownloadMenu(series, volumes, downloadOpen, onDismiss = { downloadOpen = false }, onChoose = { chosen = volumes.chooseStart() }, onSend = { sheet = SeriesSheet.Send(it) })
                },
                notes = {
                    if (series.askWhereStopped != null) {
                        WhereStopNote(series, onAsk = { sheet = SeriesSheet.WhereStop }, modifier = Modifier.padding(top = 16.dp))
                    }
                    if (sheet !is SeriesSheet.Send) SendStatusRows(work, actions, Modifier.padding(top = 16.dp))
                },
                modifier = Modifier.padding(padding),
            )
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
                        onChoose = { startChoosing() },
                        onSend = { sheet = SeriesSheet.Send(it) },
                    )
                    Spacer(Modifier.height(12.dp))
                    SeriesPanel(
                        series,
                        onReadFrom = { sheet = SeriesSheet.ReadFrom },
                        onTracking = { sheet = SeriesSheet.Tracking },
                        groupMenu = wide && LocalEinkLayout.current == null,
                        groupOpen = groupOpen,
                        onGroup = { groupOpen = it },
                    )
                    if (series.askWhereStopped != null) {
                        WhereStopNote(series, onAsk = { sheet = SeriesSheet.WhereStop }, modifier = Modifier.padding(top = 16.dp))
                    }
                    // A send keeps its row here once its sheet is closed, until it is shared or put away.
                    if (sheet !is SeriesSheet.Send) SendStatusRows(work, actions, Modifier.padding(top = 16.dp))
                }
                val list: LazyListScope.() -> Unit = {
                    seriesChapters(
                        series = series,
                        chapters = chapters,
                        late = lateChapters,
                        newestFirst = newestFirst,
                        onOrder = {
                            newestFirst = it
                            ChapterOrder.keep(context, it)
                        },
                        folds = folds,
                        chosen = chosen,
                        onChosen = { chosen = it },
                        onOpen = { row -> actions.openReader(work, row.id, series.cover, null) },
                        onLong = { row -> sheet = SeriesSheet.Chapter(row.id) },
                        opening = opening.takeIf { chosen != null },
                        onRetry = { store.reading.loadChapters(work, chapters?.edition?.sourceId) },
                        onReadFrom = { sheet = SeriesSheet.ReadFrom },
                    )
                    if (chosen == null && series.recommended.isNotEmpty()) {
                        item(key = "recommended") { SeriesRecommended(series) }
                    }
                }
                val place = opening?.takeIf { chosen != null }
                val choosingAt = if (place != null && chapters != null) chapters.choosingItem(newestFirst, folds, place.id) else -1
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
                        ChapterColumn(Modifier.weight(1f).fillMaxHeight(), choosingAt) { list() }
                    }
                } else {
                    ChapterColumn(Modifier.padding(padding).fillMaxSize(), choosingAt) {
                        if (chosen == null) {
                            item(key = "top") { Column(Modifier.padding(horizontal = 16.dp)) { top() } }
                        }
                        list()
                    }
                }
            }
        }
    }

    val edition = series?.edition
    if (groupOpen && edition != null && edition.groups.size > 1) {
        when {
            LocalEinkLayout.current != null -> EinkGroupPage(work, series.titles.display, edition) { groupOpen = false }
            !wide -> GroupSheet(work, edition) { groupOpen = false }
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

/** The cover and title the tapped tile carried, in the header's own sizes and place. */
@Composable
private fun KnownSeriesHeader(tile: WorkTile) {
    Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        ReadCover(tile.cover, 112.dp, Modifier.sharedWork(Shared.Cover, tile.work))
        Text(
            tile.title,
            style = ShowTitle.page,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 3,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f).semantics { heading() },
        )
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
                    .then(
                        if (canOpen) {
                            // Named for a screen reader by the series it opens.
                            Modifier
                                .clickable(onClickLabel = actionWords(context, series.action), role = Role.Button, onClick = onCover)
                                .semantics { contentDescription = series.titles.display }
                        } else {
                            Modifier
                        },
                    ),
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
            SeriesSynopsis(synopsis, series.work, Modifier.padding(top = 16.dp))
        }
    }
}

/**
 * The synopsis in four lines, ending "… More" where there is more, as on a show's page; a tap, or
 * the screen reader's action, opens it in place and it stays open. The height grows with the text
 * on the standard spring, so what sits under it moves with it instead of jumping, and a screen
 * reader reads the whole text either way.
 */
@Composable
private fun SeriesSynopsis(text: String, work: WorkId, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    var expanded by rememberSaveable(work) { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    val style = MaterialTheme.typography.bodyMedium
    val measurer = rememberTextMeasurer()
    val more = Strings.actionMore(context)
    val eink = LocalEinkLayout.current != null
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val width = constraints.maxWidth
        val density = LocalDensity.current
        val cut = remember(text, width, style, density, more) {
            SynopsisCuts.get(text, width, style, density, more, SERIES_SYNOPSIS_LINES) {
                synopsisCut(text, width, style, measurer, more, SERIES_SYNOPSIS_LINES)
            }
        }
        val open = expanded || cut == null
        Text(
            buildAnnotatedString {
                if (open) {
                    append(text)
                } else {
                    append(cut)
                    append(" ")
                    withStyle(SpanStyle(color = colors.onSurface, fontWeight = FontWeight.Medium)) { append(more) }
                }
            },
            style = style,
            color = colors.onSurfaceVariant,
            modifier = Modifier
                .fillMaxWidth()
                .then(if (eink) Modifier else Modifier.animateContentSize(Motion.standard()))
                .then(
                    if (open) {
                        Modifier
                    } else {
                        Modifier.clickable(onClickLabel = Strings.titleSynopsisHint(context), role = Role.Button) { expanded = true }
                    },
                )
                .semantics { contentDescription = text },
        )
    }
}

/** How many lines the series synopsis shows before "More". */
private const val SERIES_SYNOPSIS_LINES = 4

/**
 * The one main action, amber, 56 high, with Status and Download outlined side by side under it.
 * With no source, or a source that failed, Read from another source takes its place as a
 * secondary button, since reading from here is not possible; a failed source keeps the main
 * action when its chapter is downloaded, which still opens.
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
    val failed = chapters?.let { it.failure != null && !series.action.opensDownloaded(it.volumes) } == true
    // No source is known to have it yet, but the engine is still asking: the chapters have not
    // come. The main button holds its place, quiet, rather than say there is nothing to read.
    val asking = series.action is SeriesAction.NoSource && chapters == null
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (asking) {
            Button(
                onClick = {},
                enabled = false,
                modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp).clearAndSetSemantics {},
            ) {}
        } else if (words == null || failed) {
            FilledTonalButton(onClick = onReadFrom, modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp)) {
                Text(Strings.seriesReadFromAnother(context), style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
            }
        } else {
            Button(
                onClick = { onMain(series.action) },
                enabled = !choosing,
                modifier = Modifier.fillMaxWidth().heightIn(min = 56.dp),
            ) { Text(words, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center) }
        }
        if (!failed && (words != null || asking)) {
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
private fun SeriesPanel(
    series: SeriesView,
    onReadFrom: () -> Unit,
    onTracking: () -> Unit,
    /** The group picker is a menu from Change here, as on a tablet. */
    groupMenu: Boolean = false,
    groupOpen: Boolean = false,
    onGroup: (Boolean) -> Unit = {},
) {
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
            // Whose version of each chapter, inside the source: only when it has a choice.
            if (edition.groups.size > 1) {
                PanelRule()
                Box {
                    PanelRow(
                        Strings.seriesGroup(context),
                        GroupWords.inUse(edition),
                        Strings.seriesChange(context),
                        onClick = { onGroup(true) },
                        tail = GroupWords.why(context, edition),
                        actionHint = Strings.seriesGroupAction(context),
                    )
                    if (groupMenu) {
                        Box(Modifier.align(Alignment.TopEnd)) {
                            Menu(expanded = groupOpen, onDismissRequest = { onGroup(false) }) {
                                GroupMenuRows(series.work, edition) { onGroup(false) }
                            }
                        }
                    }
                }
            }
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

/**
 * A panel row: its label small in haze over its value, and an action in silk at the end,
 * underlined on e-ink. [tail] follows the value in haze: "· most chapters".
 */
@Composable
private fun PanelRow(label: String, value: String, action: String?, onClick: () -> Unit, tail: String? = null, actionHint: String? = null) {
    val haze = MaterialTheme.colorScheme.onSurfaceVariant
    val eink = LocalEinkLayout.current != null
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 64.dp)
            .clickable(onClickLabel = actionHint, onClick = onClick)
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // TalkBack hears the label, the value and the tail, with a comma for the dot.
        val spoken = listOf(label, value, tail).filter { !it.isNullOrBlank() }.joinToString(", ") // i18n-skip: a separator
        Column(Modifier.weight(1f).clearAndSetSemantics { contentDescription = spoken }) {
            Text(label, style = MaterialTheme.typography.bodySmall, color = haze)
            val words = if (tail == null) {
                AnnotatedString(value)
            } else {
                buildAnnotatedString {
                    append(value)
                    withStyle(SpanStyle(color = haze)) { append(" · $tail") } // i18n-skip: a separator
                }
            }
            // A new value crossfades, as a group picked; e-ink swaps it at once.
            val text = @Composable { shown: AnnotatedString -> Text(shown, style = MaterialTheme.typography.bodyLarge) }
            if (eink) text(words) else SwapFade(words, animationSpec = Motion.fadeIn()) { text(it) }
        }
        if (action != null) {
            Text(
                action,
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.SemiBold,
                textDecoration = if (eink) TextDecoration.Underline else null,
                modifier = Modifier.padding(start = 12.dp),
            )
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
    late: Boolean,
    newestFirst: Boolean,
    onOrder: (Boolean) -> Unit,
    folds: SnapshotStateMap<String, Boolean>,
    chosen: Set<ChapterId>?,
    onChosen: (Set<ChapterId>) -> Unit,
    onOpen: (ChapterRow) -> Unit,
    onLong: (ChapterRow) -> Unit,
    opening: OpenFocus?,
    onRetry: () -> Unit,
    onReadFrom: () -> Unit,
) {
    val novel = series.medium == Medium.NOVEL
    // "Volumes" only over a novel's volumes; a list with none is its chapters.
    val byVolume = novel && chapters?.volumes.orEmpty().any { it.volume != null }
    if (chosen == null) {
        item(key = "chapters-heading") { ChaptersHeading(byVolume, newestFirst, onOrder) }
    }
    // Until the chapters come the space is held, its placeholders drawn only after [late]; the
    // rows fade in where they fade out, so nothing above moves and nothing goes blank.
    if (chapters == null) {
        if (!late) {
            item(key = "placeholder-hold") { Spacer(Modifier.height(288.dp)) }
            return
        }
        items(4, key = { "placeholder-$it" }) {
            Column(
                itemMotion().fillMaxWidth().heightIn(min = 72.dp).padding(horizontal = 16.dp, vertical = 14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                TextPlaceholder(0.5f)
                TextPlaceholder(0.3f)
            }
        }
        return
    }
    val failure = chapters.failure
    val groups = chapters.listedGroups()
    if (failure is ReadingFailure.NoSource) {
        // Nothing was ever read from a source, so nothing failed: an empty state that leads to Read from.
        item(key = "chapters-no-source") {
            val context = LocalContext.current
            Box(itemMotion()) {
                PlainEmpty(
                    Strings.seriesNoSourceTitle(context),
                    Strings.seriesNoSourceLine(context),
                    action = Strings.seriesNoSourceSearch(context),
                    onAction = onReadFrom,
                )
            }
        }
    } else if (failure != null) {
        item(key = "chapters-failed") { Box(itemMotion()) { ChaptersFailed(series, failure, groups.chapterRows().size, onRetry) } }
    } else if (groups.isEmpty()) {
        item(key = "chapters-none") {
            val context = LocalContext.current
            Box(itemMotion()) {
            PlainEmpty(
                Strings.seriesNoChaptersTitle(context),
                Strings.seriesNoChaptersLine(context),
                action = Strings.seriesReadFromAnother(context),
                onAction = onReadFrom,
            )
            }
        }
    }
    val ordered = if (newestFirst) groups.reversed() else groups
    val single = chapters.singleGroup(ordered)
    ordered.forEachIndexed { index, group ->
        val key = group.volume ?: "none-$index"
        val open = single || (folds[key] ?: !group.allRead)
        if (!single) {
            item(key = "volume-$key") {
                val context = LocalContext.current
                val (name, rest) = SeriesWords.volume(context, group, novel)
                Column(itemMotion()) {
                    SeriesRule()
                    VolumeRowItem(name, rest, open, onToggle = { folds[key] = !open })
                }
            }
        }
        if (open) {
            val rows = if (newestFirst) group.chapters.reversed() else group.chapters
            items(rows, key = { "chapter-${it.id}" }) { row -> Box(itemMotion()) { ChapterItem(series, row, chosen, onChosen, onOpen, onLong, opening) } }
        }
    }
}

/** Whether the action opens a chapter that is downloaded, so it opens with the source failed. */
private fun SeriesAction.opensDownloaded(volumes: List<VolumeGroup>): Boolean {
    val chapter = when (this) {
        is SeriesAction.Read -> chapter
        is SeriesAction.Continue -> chapter
        is SeriesAction.ContinuePart -> chapter
        is SeriesAction.ReadPart -> chapter
        else -> return false
    }
    return volumes.downloaded().any { it.id == chapter }
}

/** With the source failed, what is downloaded still opens: only those chapters, under no volume. */
private fun ReadChapters.listedGroups(): List<VolumeGroup> = if (failure != null) {
    listOfNotNull(
        volumes.downloaded().takeIf { it.isNotEmpty() }?.let { VolumeGroup(volume = null, title = null, chapters = it, allRead = false) },
    )
} else {
    volumes
}

/**
 * Whether the chapters are one run with no volume rows: the downloaded ones after a failure, a list
 * with no volumes, or a flat one (Show volumes off, or too few chapters with a volume).
 */
private fun ReadChapters.singleGroup(groups: List<VolumeGroup>): Boolean =
    failure != null || (groups.size == 1 && (groups.first().flat || groups.first().volume == null))

/**
 * The item [seriesChapters] lists [id]'s row at while choosing, when the page has no header or
 * heading above the chapters: -1 when the row is not listed, as in a volume folded away.
 */
private fun ReadChapters.choosingItem(newestFirst: Boolean, folds: Map<String, Boolean>, id: ChapterId): Int {
    val groups = listedGroups()
    // The failure's or the empty list's one item comes first.
    var item = if (failure != null || groups.isEmpty()) 1 else 0
    val ordered = if (newestFirst) groups.reversed() else groups
    val single = singleGroup(ordered)
    ordered.forEachIndexed { index, group ->
        val key = group.volume ?: "none-$index"
        if (!single) item += 1
        if (single || (folds[key] ?: !group.allRead)) {
            val rows = if (newestFirst) group.chapters.reversed() else group.chapters
            val at = rows.indexOfFirst { it.id == id }
            if (at >= 0) return item + at
            item += rows.size
        }
    }
    return -1
}

/** The row Choose chapters opens on, which TalkBack and the keyboard start on once. */
internal class OpenFocus(val id: ChapterId) {
    val requester = FocusRequester()
    var asked = false
}

/**
 * The chapters' list. Choose chapters opens it with the reader's place, item [choosingAt], as near
 * the middle as the list's ends allow, placed before choosing's first frame so nothing scrolls, and
 * leaving it puts the list back where it was.
 */
@Composable
private fun ChapterColumn(modifier: Modifier, choosingAt: Int, content: LazyListScope.() -> Unit) {
    BoxWithConstraints(modifier) {
        val listed = rememberLazyListState()
        val density = LocalDensity.current
        val viewport = with(density) { (maxHeight - LIST_BOTTOM).roundToPx() }
        val row = with(density) { CHAPTER_ROW.roundToPx() }
        // Where the page was before choosing moved it: leaving Choose chapters goes back there.
        val before = remember { intArrayOf(-1, 0) }
        // Run as choosing starts and ends, in the frame that lays the list out for it.
        DisposableEffect(choosingAt >= 0) {
            if (choosingAt >= 0) {
                before[0] = listed.firstVisibleItemIndex
                before[1] = listed.firstVisibleItemScrollOffset
                listed.requestScrollToItem(choosingAt, centredOffset(viewport, row))
            } else if (before[0] >= 0) {
                listed.requestScrollToItem(before[0], before[1])
                before[0] = -1
            }
            onDispose {}
        }
        LazyColumn(
            Modifier.fillMaxSize().panelPageKeys(listed),
            state = listed,
            contentPadding = PaddingValues(bottom = LIST_BOTTOM),
            content = content,
        )
    }
}

/** The space under the chapters' list. */
private val LIST_BOTTOM = 24.dp

/** A chapter row's least height, which Choose chapters centres the reader's place by. */
private val CHAPTER_ROW = 72.dp

@Composable
private fun ChapterItem(
    series: SeriesView,
    row: ChapterRow,
    chosen: Set<ChapterId>?,
    onChosen: (Set<ChapterId>) -> Unit,
    onOpen: (ChapterRow) -> Unit,
    onLong: (ChapterRow) -> Unit,
    opening: OpenFocus?,
) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val work = series.work
    val read = row.state is ChapterReadState.Read
    val place = opening?.takeIf { it.id == row.id }
    if (place != null) {
        LaunchedEffect(place) {
            if (!place.asked) {
                place.asked = true
                runCatching { place.requester.requestFocus() }
            }
        }
    }
    Column {
        SeriesRule()
        ChapterRowItem(
            modifier = if (place != null) Modifier.focusRequester(place.requester).focusable() else Modifier,
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

/** "Chapters", or "Volumes" over a novel's volumes, with the order at the end: Newest first or Oldest first. */
@Composable
private fun ChaptersHeading(volumes: Boolean, newestFirst: Boolean, onOrder: (Boolean) -> Unit) {
    val context = LocalContext.current
    var open by remember { mutableStateOf(false) }
    Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(start = 16.dp, end = 4.dp, top = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            if (volumes) Strings.seriesVolumes(context) else Strings.seriesChapters(context),
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

/** The chapter list's order, kept on this device for every series. */
private object ChapterOrder {
    private const val PREFS = "kaeri_series_ui"
    private const val NEWEST = "newest_first"

    fun newestFirst(context: android.content.Context): Boolean =
        context.getSharedPreferences(PREFS, android.content.Context.MODE_PRIVATE).getBoolean(NEWEST, true)

    fun keep(context: android.content.Context, newest: Boolean) {
        context.getSharedPreferences(PREFS, android.content.Context.MODE_PRIVATE).edit().putBoolean(NEWEST, newest).apply()
    }
}
