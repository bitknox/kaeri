// The e-ink shell's own pages, as the e-ink board draws them: Home on one screen (the series in
// progress with its one solid button, Continue, New chapters on an e-reader or This week on a
// phone, then the Browse row), and the e-reader's Downloads and More tabs. Paper and ink, no
// motion, no spinners: what changes redraws in place.

package app.kaeri.ui.reading

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.Placeable
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkOutlinedButton
import app.kaeri.eink.EinkSizes
import app.kaeri.eink.EinkSolidButton
import app.kaeri.eink.EinkTextButton
import app.kaeri.eink.EinkType
import app.kaeri.eink.einkClick
import app.kaeri.eink.hairline
import app.kaeri.eink.topRule
import app.kaeri.engine.core.SettingValue
import app.kaeri.engine.model.BookDownloadSeries
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.DownloadPick
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.ReadSpotlight
import app.kaeri.engine.model.ReadWeekDay
import app.kaeri.engine.model.ReadWeekEntry
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.SettingKey
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.Wording
import app.kaeri.store.loadSync
import app.kaeri.store.text
import app.kaeri.ui.AccountWords
import app.kaeri.ui.AppActions
import app.kaeri.ui.Destination
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.SettingsPane
import app.kaeri.ui.appVersion
import app.kaeri.ui.devicesSummary
import app.kaeri.ui.einkReaderShape
import app.kaeri.ui.freeBytes
import app.kaeri.ui.kaeriIsFor
import java.time.Instant
import java.time.format.TextStyle

/** A 7 inch panel and up: the covers grow and New chapters keeps two. */
@Composable
private fun tallPanel(): Boolean = LocalConfiguration.current.screenHeightDp >= 860

/** The hero's cover, as the board sizes it for the panel. */
@Composable
private fun heroCover(reader: Boolean): Dp = when {
    !reader -> 96.dp
    tallPanel() -> 124.dp
    else -> 104.dp
}

/** A Continue cover: four across on an e-reader, three on a phone, at the board's size. */
@Composable
private fun continueCover(reader: Boolean): Dp = when {
    !reader -> 100.dp
    tallPanel() -> 104.dp
    else -> 90.dp
}

// Home

/**
 * Reading's Home on e-ink, one screen with no paging: the series in progress naming its next step
 * on the one solid button, Continue, New chapters (This week on a phone) and the Browse row under
 * a 2 dp rule. A first run has the demo in a bordered box and the Browse list in place of shelves.
 */
@Composable
internal fun EinkHomeScreen(actions: AppActions) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val home by store.reading.home.collectAsStateWithLifecycle()
    val failures by store.reading.failures.collectAsStateWithLifecycle()
    val demo by store.demo.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.reading.loadHome() }
    DemoLaunchHost(actions)
    val reader = einkReaderShape()
    // Large text keeps the Browse list to what fits no longer: it scrolls rather than clip.
    val large = LocalDensity.current.fontScale > 1.3f
    val title = if (kaeriIsFor() == KaeriIsFor.READ) Strings.homeTitle(context) else Strings.navRead(context)
    PaperPage(back = null, title = title) {
        val view = home
        val failure = failures[ReadingAbout.HOME]
        when {
            view == null && failure != null -> PaperFailure(
                Strings.homeFailed(context),
                failureWords(context, failure),
                Strings.homeTryAgain(context),
            ) { store.reading.loadHome(refresh = true) }
            // Loading with no motion: one word, then the page in one refresh.
            view == null -> Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp))
            else -> {
                val tile = view.spotlight
                val spot = view.spot
                val continuing = view.continueReading
                val came = cameOut(view.week)
                val rows = view.rows.filter { it.tiles.isNotEmpty() }
                val own = continuing.isNotEmpty() || came.isNotEmpty() || (tile != null && !demo.onlyBooks)
                // Home is one screen; what can't fit (large text, a small panel) scrolls rather
                // than clip. The first run's Browse list instead keeps the rows that fit.
                val scroll = large || own
                Column(Modifier.fillMaxSize().then(if (scroll) Modifier.verticalScroll(rememberScrollState()) else Modifier)) {
                    if (demo.onlyBooks) {
                        PaperDemo(actions, reader)
                        PaperDemoLine()
                    } else if (tile != null && spot != null) {
                        PaperHero(tile, spot, actions, reader)
                    }
                    if (continuing.isNotEmpty()) PaperContinue(continuing.take(if (reader) 4 else 3), reader)
                    if (came.isNotEmpty()) PaperChapters(came.take(if (reader && tallPanel()) 2 else 1), reader)
                    if (rows.isNotEmpty()) {
                        PaperBrowse(rows, inline = !own, fit = !scroll)
                    } else if (view.rows.any { it.pending }) {
                        Text(Strings.stateLoading(context), style = EinkType.Secondary, modifier = Modifier.padding(top = 16.dp))
                    }
                }
            }
        }
    }
}

/** The series in progress: its cover with an ink edge, what it is, where the person is, and the one solid button. */
@Composable
private fun PaperHero(tile: WorkTile, spot: ReadSpotlight, actions: AppActions, reader: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val words = actionWords(context, spot.action)
    val chapter = spot.action?.chapter
    val open = {
        if (chapter != null) actions.openReader(tile.work, chapter, tile.cover, readerCoverKey("spotlight", tile.work))
        else navigator.openSeries(tile.work, "eink-home")
    }
    val button: @Composable (Modifier) -> Unit = { m ->
        if (words != null) EinkSolidButton(words, open, m)
        else EinkOutlinedButton(Strings.homeDetails(context), { navigator.openSeries(tile.work, "eink-home") }, m)
    }
    Column(Modifier.padding(top = 12.dp)) {
        Row(verticalAlignment = if (reader) Alignment.Top else Alignment.Bottom) {
            PaperCover(tile.cover, tile.title, heroCover(reader), Modifier.einkClick { navigator.openSeries(tile.work, "eink-home") })
            Column(Modifier.weight(1f).padding(start = 22.dp, top = if (reader) 12.dp else 0.dp)) {
                Column(Modifier.semantics(mergeDescendants = true) {}) {
                    Text(spot.label.text(context), style = EinkType.Secondary)
                    Text(tile.title, style = EinkType.ReaderTitle, maxLines = 3, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 6.dp))
                    spot.line?.let { Text(it.text(context), style = EinkType.Secondary, modifier = Modifier.padding(top = 2.dp)) }
                }
                if (reader) button(Modifier.padding(top = 16.dp))
            }
        }
        // On a phone the button runs the full width, within thumb reach.
        if (!reader) button(Modifier.fillMaxWidth().padding(top = 14.dp))
    }
}

/** The demo while it is the only thing to read: a bordered box with its cover, credit line, title, blurb and Read. */
@Composable
private fun PaperDemo(actions: AppActions, reader: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val comic = readingDemo.comic ?: return
    val work = comic.workId
    Row(
        Modifier
            .padding(top = 12.dp)
            .fillMaxWidth()
            .border(EinkSizes.Rule, EinkColours.Ink, RoundedCornerShape(12.dp))
            .padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PaperCover(comic.coverArt, comic.series, if (reader) 92.dp else 80.dp, if (work != null) Modifier.einkClick { navigator.openSeries(work, "eink-home") } else Modifier)
        Column(Modifier.weight(1f).padding(start = 18.dp)) {
            Column(Modifier.semantics(mergeDescendants = true) {}) {
                Text(Strings.homeSpotlightDemo(context), style = EinkType.Secondary)
                Text(comic.series, style = EinkType.ReaderTitle, modifier = Modifier.padding(top = 4.dp).semantics { heading() })
                comic.blurb?.let { Text(it, style = EinkType.Secondary.copy(color = EinkColours.Ink), maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 2.dp)) }
            }
            if (work != null) {
                EinkSolidButton(Strings.homeSpotlightRead(context), { actions.openReader(work, null, comic.coverArt, readerCoverKey("spotlight", work)) }, Modifier.padding(top = 12.dp))
            }
        }
    }
}

/** Under the demo: only the demo is on, with Add a source outlined beside the words. */
@Composable
private fun PaperDemoLine() {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    Row(Modifier.fillMaxWidth().padding(top = 14.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(Strings.homeDemoOnlyBooks(context), style = EinkType.Secondary, modifier = Modifier.weight(1f))
        EinkOutlinedButton(Strings.sourcesAddTitle(context), { navigator.open(app.kaeri.ui.PROVIDERS_ROUTE) }, Modifier.padding(start = 12.dp), height = 48.dp)
    }
}

/** A heading on paper with an underlined link at its end. */
@Composable
private fun PaperHeading(title: String, link: String?, onLink: () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = EinkType.RowTitle, modifier = Modifier.weight(1f).semantics { heading() })
        if (link != null) EinkTextButton(link, onLink, underline = true)
    }
}

/** Continue: four covers on an e-reader, three on a phone, each with its title and short line. */
@Composable
private fun PaperContinue(tiles: List<WorkTile>, reader: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    PaperHeading(Strings.homeReadContinue(context), Strings.einkHomeAll(context)) {
        ReadPlaces.showList(LibrarySegmentRead.READING)
        navigator.go(Destination.Library)
    }
    val width = continueCover(reader)
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(if (reader) 24.dp else 20.dp)) {
        for (tile in tiles) {
            Column(
                Modifier
                    .width(width)
                    .einkClick { navigator.openSeries(tile.work, "eink-continue") }
                    .semantics(mergeDescendants = true) {},
            ) {
                PaperCover(tile.cover, tile.title, width, Modifier)
                Text(tile.title, style = EinkType.Label.copy(fontSize = EinkType.Secondary.fontSize), maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
                val line = tile.line(context)
                if (line.isNotEmpty()) Text(line, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** A chapter that came out this week, with its day. */
private data class CameOut(val day: ReadWeekDay, val entry: ReadWeekEntry)

/** This week's chapters that came out, the newest first. */
private fun cameOut(week: List<ReadWeekDay>): List<CameOut> =
    week.flatMap { day -> day.entries.filter { !it.usually && it.outAt != null }.map { CameOut(day, it) } }
        .sortedByDescending { it.entry.outAt }

/**
 * New chapters on an e-reader, with All opening Updates; This week on a phone, with Updates. Each
 * row: the title, then the dot with the chapter and its day, and a chevron.
 */
@Composable
private fun PaperChapters(came: List<CameOut>, reader: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    if (reader) {
        PaperHeading(Strings.einkHomeNewChapters(context), Strings.einkHomeAll(context)) { navigator.go(Destination.Updates) }
    } else {
        PaperHeading(Strings.homeThisWeek(context), Strings.navUpdates(context)) { navigator.go(Destination.Updates) }
    }
    came.forEachIndexed { index, (day, entry) ->
        val tile = entry.tile
        val dayWords = if (day.today) Strings.readingAgoToday(context)
        else Instant.ofEpochSecond(day.day).atZone(app.kaeri.store.AppClock.zone).dayOfWeek.getDisplayName(TextStyle.FULL, app.kaeri.store.AirTime.locale)
        val line = listOfNotNull((entry.chapter ?: tile.next)?.text(context), dayWords).joinToString(" · ") // i18n-skip
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = EinkSizes.Row)
                .einkClick { navigator.openSeries(tile.work, "eink-week") }
                .then(if (index < came.size - 1) Modifier.hairline() else Modifier)
                .padding(vertical = 8.dp)
                .semantics(mergeDescendants = true) { contentDescription = "${tile.title}. $line" }, // i18n-skip
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f)) {
                Text(tile.title, style = EinkType.Label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 2.dp)) {
                    Box(Modifier.size(EinkSizes.NewDot).clip(CircleShape).background(EinkColours.Ink))
                    Text(line, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = 8.dp))
                }
            }
            Icon(Icons.Chevron, contentDescription = null, tint = EinkColours.Ink, modifier = Modifier.padding(start = 12.dp).size(22.dp))
        }
    }
}

/**
 * Browse under a 2 dp rule: one row opening the list of rows, or with nothing of the person's own
 * above it ([inline]), the list itself, as many rows as fit the screen ([fit]).
 */
@Composable
private fun ColumnScope.PaperBrowse(rows: List<app.kaeri.engine.model.ReadHomeRow>, inline: Boolean, fit: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    if (inline) {
        Text(Strings.readHomeBrowse(context), style = EinkType.RowTitle, modifier = Modifier.padding(top = 18.dp, bottom = 4.dp).semantics { heading() })
        FitLines(if (fit) Modifier.weight(1f, fill = false) else Modifier) {
            for (row in rows) BrowseLine(row) { navigator.openReadPage(READ_PAGE_ROW + row.id) }
        }
    } else {
        Box(Modifier.fillMaxWidth().padding(top = if (fit) 0.dp else 8.dp).topRule())
        BrowseLink(Strings.readHomeBrowse(context), Strings.readHomeBrowseLine(context), colourHome = false) {
            navigator.openReadPage(READ_PAGE_BROWSE)
        }
    }
}

/** Lines one under another, as many as fit whole in the height there is; the rest stay off. */
@Composable
private fun FitLines(modifier: Modifier, content: @Composable () -> Unit) {
    Layout(content, modifier) { measurables, constraints ->
        val loose = constraints.copy(minHeight = 0)
        val placed = mutableListOf<Placeable>()
        var used = 0
        for (measurable in measurables) {
            val placeable = measurable.measure(loose)
            if (constraints.hasBoundedHeight && used + placeable.height > constraints.maxHeight) break
            placed += placeable
            used += placeable.height
        }
        layout(constraints.maxWidth, used) {
            var y = 0
            placed.forEach { it.place(0, y); y += it.height }
        }
    }
}

/** A cover on paper: square corners and a 1 dp ink edge, or the title on its plate with no art. */
@Composable
private fun PaperCover(url: String?, title: String, width: Dp, modifier: Modifier) {
    ReadCover(url, width, modifier.border(EinkSizes.Hairline, EinkColours.Ink), shape = RoundedCornerShape(0.dp), title = title)
}

// Downloads

/** A line of the e-reader's Downloads, packed into pages like any paper list. */
private sealed interface DownLine {
    data class Heading(val text: String, val side: String?) : DownLine
    data class Moving(val series: BookDownloadSeries) : DownLine
    data class Here(val series: BookDownloadSeries) : DownLine
    data class Note(val title: String, val line: String) : DownLine
    data object KeepReady : DownLine
}

/**
 * The e-reader's Downloads: what is downloading with Stop, then what is here by series, then Keep
 * next chapters ready under a 2 dp rule. Progress is the engine's words rewritten in place, never
 * a moving bar.
 */
@Composable
internal fun EinkDownloadsPage() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val view by store.reading.downloads.collectAsStateWithLifecycle()
    val settings by store.settings.view.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { store.reading.loadDownloads() }
    val free = freeBytes()
    var page by rememberSaveable { mutableIntStateOf(0) }
    PaperPage(back = null, title = Strings.downloadsTitle(context)) {
        val v = view
        val used = Wording.bytes(context, v?.meter?.used?.toLong() ?: 0L)
        Text(
            if (free != null) Strings.einkDownloadsSpace(context, used, Wording.bytes(context, free)) else Strings.einkDownloadsSpaceUsed(context, used),
            style = EinkType.Secondary,
        )
        if (v == null) {
            Text(Strings.stateLoading(context), style = EinkType.Body, modifier = Modifier.padding(top = 16.dp))
            return@PaperPage
        }
        val wifiOnly = !store.settings.isOn(SettingKey.CHAPTER_DOWNLOADS_ON_CELLULAR, settings)
        val moving = v.series.filter { it.queued > 0u || it.failed.isNotEmpty() }
        val here = v.series.filter { it.chapters > 0u }
        val lines = buildList {
            if (moving.isNotEmpty()) {
                add(DownLine.Heading(Strings.einkDownloadsDownloading(context), if (wifiOnly) Strings.settingsWifiOnly(context) else null))
                moving.forEach { add(DownLine.Moving(it)) }
            }
            if (here.isNotEmpty()) {
                add(DownLine.Heading(Strings.einkDownloadsHere(context), null))
                here.forEach { add(DownLine.Here(it)) }
            }
            if (moving.isEmpty() && here.isEmpty()) add(DownLine.Note(Strings.downloadsEmptyTitle(context), Strings.readDownloadsEmptyDetail(context)))
            add(DownLine.KeepReady)
        }
        PagedItems(lines, page, { page = it }, Modifier.padding(top = 8.dp)) { line -> PaperDownLine(line, v.rows) }
    }
}

@Composable
private fun PaperDownLine(line: DownLine, rows: List<app.kaeri.engine.model.BookDownloadRow>) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    when (line) {
        is DownLine.Heading -> Row(Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(line.text, style = EinkType.RowTitle, modifier = Modifier.weight(1f).semantics { heading() })
            line.side?.let { Text(it, style = EinkType.Secondary) }
        }
        is DownLine.Moving -> {
            val series = line.series
            val failed = series.failed.isNotEmpty()
            val words = series.line.joinToString(" · ") { it.text(context) } // i18n-skip
            Row(
                Modifier.fillMaxWidth().heightIn(min = EinkSizes.Row).hairline().padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f).semantics(mergeDescendants = true) {}) {
                    Text(series.title, style = EinkType.Label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (words.isNotEmpty()) Text(words, style = EinkType.Secondary, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
                if (failed) {
                    EinkTextButton(stringResource(R.string.chapter_retry), { store.reading.download(series.work, DownloadPick.Chapters(series.failed)) })
                } else {
                    val going = rows.filter { it.work == series.work && it.state !is ChapterDownloadState.Done && it.state !is ChapterDownloadState.Failed }.map { it.chapter.id }
                    EinkTextButton(Strings.actionStop(context), { store.reading.cancelDownloads(series.work, going) })
                }
            }
        }
        is DownLine.Here -> {
            val series = line.series
            // While chapters still come, the row says what is here; the engine's line says the progress above.
            val words = if (series.queued > 0u || series.failed.isNotEmpty()) Strings.readDownloadsTotal(context, series.chapters.toInt(), Wording.bytes(context, series.bytes.toLong()))
            else series.line.joinToString(" · ") { it.text(context) } // i18n-skip
            BrowseLink(series.title, words, colourHome = false) { navigator.openSeries(series.work, "eink-downloads") }
        }
        is DownLine.Note -> Column(Modifier.padding(vertical = 12.dp)) {
            Text(line.title, style = EinkType.Body)
            Text(line.line, style = EinkType.Secondary)
        }
        DownLine.KeepReady -> {
            val settings by store.settings.view.collectAsStateWithLifecycle()
            val on = store.settings.isOn(SettingKey.KEEP_CHAPTERS_READY, settings)
            val state = if (on) Strings.einkKeepReadyOn(context) else Strings.einkKeepReadyOff(context)
            Column(
                Modifier
                    .padding(top = 16.dp)
                    .fillMaxWidth()
                    .topRule()
                    .heightIn(min = EinkSizes.Row)
                    .einkClick(role = Role.Switch) { store.setSetting(SettingKey.KEEP_CHAPTERS_READY, SettingValue.Bool(!on)) }
                    .padding(top = 12.dp, bottom = 8.dp)
                    .semantics(mergeDescendants = true) { stateDescription = state },
            ) {
                Text(Strings.settingsKeepChaptersReady(context), style = EinkType.Label)
                Text(state, style = EinkType.Secondary, modifier = Modifier.padding(top = 2.dp))
            }
        }
    }
}

// More

/**
 * The e-reader's More: Sources, Trackers, Sync, Reader settings, Settings and About, each saying
 * what is behind it, as rows with a chevron.
 */
@Composable
internal fun EinkMorePage() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val books by store.bookSources.view.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val sync by store.sync.status.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) {
        store.bookSources.load()
        store.loadSync()
    }
    val answer = kaeriIsFor()
    val version = appVersion()
    val rows = listOf(
        Triple(Strings.sourcesTitle(context), bookSourcesSummary(context, books).orEmpty(), SettingsPane.Sources),
        Triple(Strings.einkMoreTrackers(context), AccountWords.connected(context, trackers, answer), SettingsPane.Accounts),
        Triple(Strings.einkMoreSync(context), devicesSummary(context, sync), SettingsPane.Devices),
        Triple(Strings.readerSettings(context), readingSummary().orEmpty(), SettingsPane.Reading),
        Triple(Strings.settingsTitle(context), Strings.einkMoreSettingsLine(context), null),
        Triple(Strings.aboutTitle(context), Strings.aboutVersion(context, version), SettingsPane.About),
    )
    PaperPage(back = null, title = Strings.einkTabMore(context)) {
        Column(Modifier.padding(top = 8.dp)) {
            for ((title, line, pane) in rows) BrowseLink(title, line, colourHome = false) { navigator.openSettings(pane) }
        }
    }
}
