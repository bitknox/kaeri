// The Library on e-ink, as the large panel board draws it: the title with All, Manga and Novels and
// View on one line, the lists as chips with their counts under it, and the chosen list's covers in
// the grid every paper page shares, a page at a time. Nothing scrolls. On 6 and 7 inch panels and
// the e-ink phone the lists fold into one "Reading · 23" button that opens a sheet. View chooses
// covers or rows, the medium, the order and whether only downloaded series show. On an e-reader
// Downloads is a tab of its own, so the Downloads row stays only where it is not.

package app.kaeri.ui.reading

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.eink.EinkColours
import app.kaeri.eink.EinkLook
import app.kaeri.eink.EinkOutlinedButton
import app.kaeri.eink.EinkPager
import app.kaeri.eink.EinkSegmented
import app.kaeri.eink.EinkSizes
import app.kaeri.eink.EinkSolidButton
import app.kaeri.eink.EinkTextButton
import app.kaeri.eink.EinkTheme
import app.kaeri.eink.EinkType
import app.kaeri.eink.PaperWaiting
import app.kaeri.eink.einkClick
import app.kaeri.eink.einkFocus
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.Strings
import app.kaeri.store.text
import app.kaeri.ui.DOWNLOADS_ROUTE
import app.kaeri.ui.Destination
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.einkBarDestinations
import app.kaeri.ui.einkReaderShape
import app.kaeri.ui.kaeriIsFor
import app.kaeri.ui.storageLine

/** Covers in the grid, or rows with a small cover. */
private enum class PaperShow { Covers, List }

/** Which series a list shows by what they are. */
private enum class PaperMedium { All, Manga, Novels }

/** How a list is ordered: as last read (the engine's order), by title, or new chapters first. */
private enum class PaperSort { Recent, Title, New }

/** Every series on the list, or only those with chapters on this device. */
private enum class PaperKept { All, Here }

/** From this width the header takes one line and the lists show as chips, as on the 10.3 inch panel. */
private const val LIBRARY_LARGE_DP = 900

/** The medium control beside the title, as wide as the board draws it. */
private val MediumWidth = 330.dp

/** The medium control on a smaller panel, under the title. */
private val MediumWidthSmall = 300.dp

/** A list column is at least this wide, with this between columns and a hairline down the middle. */
private val ListColumnLeast = 420.dp
private val ListColumnGap = 40.dp

/** A row in the list view is at least this tall, with a 48 × 72 cover. */
private val ListRowLeast = 92.dp
private val ListCover = 48.dp

/** The View sheet's width at most, and each choice of its controls. */
private val SheetWidth = 680.dp
private val SheetChoice = 128.dp

/** The chips' words: one step under the body size. */
private val ChipText = EinkType.Body.copy(fontSize = EinkType.Secondary.fontSize)

/** The series [list] shows under the choices in View. */
private fun shownTiles(list: List<WorkTile>, medium: PaperMedium, kept: PaperKept, here: Set<String>, sort: PaperSort): List<WorkTile> {
    val chosen = list.filter { tile ->
        val kind = when (medium) {
            PaperMedium.All -> true
            PaperMedium.Manga -> tile.medium == Medium.MANGA
            PaperMedium.Novels -> tile.medium == Medium.NOVEL
        }
        kind && (kept == PaperKept.All || tile.work in here)
    }
    return when (sort) {
        PaperSort.Recent -> chosen
        PaperSort.Title -> chosen.sortedBy { it.title.lowercase(app.kaeri.store.AirTime.locale) }
        // Stable: within each part the engine's order, last read first.
        PaperSort.New -> chosen.sortedBy { !it.newChapter }
    }
}

/** The Library on paper: the header, then the chosen list's covers or rows a page at a time. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun EinkLibraryPage() {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val lists by store.reading.lists.collectAsStateWithLifecycle()
    val failures by store.reading.failures.collectAsStateWithLifecycle()
    val downloads by store.reading.downloads.collectAsStateWithLifecycle()
    val segment = ReadPlaces.list
    var show by rememberSaveable { mutableStateOf(PaperShow.Covers) }
    var medium by rememberSaveable { mutableStateOf(PaperMedium.All) }
    var sort by rememberSaveable { mutableStateOf(PaperSort.Recent) }
    var kept by rememberSaveable { mutableStateOf(PaperKept.All) }
    var viewing by rememberSaveable { mutableStateOf(false) }
    var choosing by rememberSaveable { mutableStateOf(false) }
    // How many covers the grid holds a page, for the View sheet's note; known once it is laid out.
    var perPage by remember { mutableIntStateOf(0) }
    // Any change of list or view starts at the first page.
    var page by rememberSaveable(segment, show, medium, sort, kept) { mutableIntStateOf(0) }
    val answer = kaeriIsFor()
    val reader = einkReaderShape()
    val downloadsTab = Destination.Downloads in einkBarDestinations(answer, reader)
    val large = LocalConfiguration.current.screenWidthDp >= LIBRARY_LARGE_DP
    // Every list once, for the counts; the chosen one again on each visit. Downloads for On this device.
    LaunchedEffect(Unit) {
        store.reading.loadDownloads()
        for (each in READ_LISTS) if (each != segment) store.reading.loadList(each)
    }
    LaunchedEffect(segment) { store.reading.loadList(segment) }
    val here = downloads?.series?.filter { it.chapters > 0u }?.mapTo(HashSet()) { it.work }
    val shown = { list: List<WorkTile>? -> list?.let { shownTiles(it, medium, kept, here.orEmpty(), sort) } }
    val count = { each: LibrarySegmentRead -> shown(lists[each])?.size }
    val mediumWords = @Composable { each: PaperMedium ->
        when (each) {
            PaperMedium.All -> Strings.einkLibraryMediumAll(context)
            PaperMedium.Manga -> Strings.einkLibraryMediumManga(context)
            PaperMedium.Novels -> Strings.einkLibraryMediumNovels(context)
        }
    }

    EinkTheme(LocalEinkLayout.current ?: EinkLook()) {
        Column(
            Modifier
                .fillMaxSize()
                .background(EinkColours.Paper)
                .windowInsetsPadding(WindowInsets.statusBars)
                .padding(horizontal = Side),
        ) {
            if (answer == KaeriIsFor.BOTH) {
                ScopeControl(reading = true, modifier = Modifier.padding(top = 8.dp)) { ReadPlaces.libraryReading = it }
            }
            val view = Strings.einkLibraryView(context)
            if (large) {
                // One line: the title, the medium and View; the lists as chips under it.
                Row(
                    Modifier.fillMaxWidth().padding(top = 16.dp).heightIn(min = 56.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    Text(Strings.navLibrary(context), style = EinkType.PageTitle, modifier = Modifier.weight(1f).semantics { heading() })
                    EinkSegmented(PaperMedium.entries, medium, mediumWords, { medium = it }, Modifier.width(MediumWidth))
                    // The words end at the margin, as the board draws them; the touch reaches past it.
                    EinkTextButton(view, { viewing = true }, Modifier.offset(x = 12.dp), underline = true)
                }
                FlowRow(
                    Modifier.fillMaxWidth().padding(top = 14.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    for (each in READ_LISTS) ListChip(listWords(context, each), count(each), each == segment) { ReadPlaces.list = each }
                }
            } else {
                // The title and View, then the medium and the list as one button opening the lists.
                Row(Modifier.fillMaxWidth().padding(top = 16.dp).heightIn(min = 56.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(Strings.navLibrary(context), style = EinkType.PageTitle, modifier = Modifier.weight(1f).semantics { heading() })
                    EinkTextButton(view, { viewing = true }, Modifier.offset(x = 12.dp), underline = true)
                }
                val words = listWords(context, segment)
                val listButton = count(segment)?.let { Strings.einkLibraryListButton(context, words, it) } ?: words
                if (reader) {
                    // A 6 or 7 inch panel: the medium and the list side by side, wrapping at large text.
                    FlowRow(
                        Modifier.fillMaxWidth().padding(top = 8.dp),
                        horizontalArrangement = Arrangement.spacedBy(16.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                        itemVerticalAlignment = Alignment.CenterVertically,
                    ) {
                        EinkSegmented(PaperMedium.entries, medium, mediumWords, { medium = it }, Modifier.width(MediumWidthSmall))
                        EinkTextButton(listButton, { choosing = true }, underline = true)
                    }
                } else {
                    // The e-ink phone: the medium across the width, the list under it at the margin.
                    EinkSegmented(PaperMedium.entries, medium, mediumWords, { medium = it }, Modifier.fillMaxWidth().padding(top = 8.dp))
                    EinkTextButton(listButton, { choosing = true }, Modifier.offset(x = (-12).dp).padding(top = 4.dp), underline = true)
                }
            }
            if (!downloadsTab) {
                // The e-ink phone has no Downloads tab: the row stays, as on the colour Library.
                val total = downloads?.total.orEmpty()
                val detail = if (total.isEmpty()) storageLine(context, 0) else total.joinToString(" · ") { it.text(context) } // i18n-skip
                Box(Modifier.padding(top = 8.dp)) {
                    BrowseLink(Strings.downloadsTitle(context), detail, colourHome = false) { navigator.open(DOWNLOADS_ROUTE) }
                }
            }
            val all = lists[segment]
            val tiles = shown(all)
            val failure = failures[ReadingAbout.LIBRARY]
            val body = Modifier.padding(top = if (large) 22.dp else 16.dp)
            when {
                all == null && failure != null -> Box(body) {
                    PaperFailure(
                        Strings.libraryListLoadFailed(context, listWords(context, segment)),
                        failureWords(context, failure),
                        Strings.actionTryAgain(context),
                    ) { store.reading.loadList(segment) }
                }
                // After 600 ms one word, then the page in one refresh: no placeholders on paper.
                all == null || tiles == null || (kept == PaperKept.Here && here == null) ->
                    PaperWaiting { Text(Strings.stateLoading(context), style = EinkType.Body, modifier = body) }
                all.isEmpty() -> PaperEmptyList(segment, body)
                tiles.isEmpty() -> Column(body) {
                    Text(Strings.einkLibraryNoneMatch(context, listWords(context, segment)), style = EinkType.Body)
                    Text(Strings.einkLibraryNoneMatchLine(context), style = EinkType.Secondary)
                }
                show == PaperShow.Covers -> PagedCovers(tiles, page, { page = it }, body) { tile, grid ->
                    SideEffect { perPage = grid.perPage }
                    LibraryCover(tile, grid, segment)
                }
                else -> PagedRows(tiles, page, { page = it }, body) { tile, first -> LibraryRow(tile, segment, first) }
            }
        }
    }
    if (viewing) {
        ViewSheet(onDone = { viewing = false }) {
            SheetChoice(Strings.einkLibraryShow(context), PaperShow.entries, show, first = true, words = {
                if (it == PaperShow.Covers) Strings.einkLibraryShowCovers(context) else Strings.einkLibraryShowList(context)
            }) { show = it }
            SheetChoice(Strings.einkLibraryMedium(context), PaperMedium.entries, medium, words = mediumWords) { medium = it }
            SheetChoice(Strings.einkLibrarySort(context), PaperSort.entries, sort, words = {
                when (it) {
                    PaperSort.Recent -> Strings.einkLibrarySortRecent(context)
                    PaperSort.Title -> Strings.einkLibrarySortTitle(context)
                    PaperSort.New -> Strings.einkLibrarySortNew(context)
                }
            }) { sort = it }
            SheetChoice(Strings.einkLibraryDownloaded(context), PaperKept.entries, kept, words = {
                if (it == PaperKept.All) Strings.einkLibraryDownloadedAll(context) else Strings.einkLibraryDownloadedHere(context)
            }) { kept = it }
            Text(
                if (perPage > 0) Strings.einkLibraryViewNote(context, perPage) else Strings.einkLibraryViewNoteShort(context),
                style = EinkType.Secondary,
                modifier = Modifier.padding(top = 10.dp, bottom = 40.dp),
            )
        }
    }
    if (choosing) {
        ListsSheet(segment, count, onChoose = { ReadPlaces.list = it; choosing = false }, onDone = { choosing = false })
    }
}

/** A list as a chip: its name and count in a 48 dp box with a 2 dp ink edge; the chosen one grey and bold. */
@Composable
private fun ListChip(name: String, count: Int?, chosen: Boolean, onClick: () -> Unit) {
    val shape = RoundedCornerShape(EinkSizes.SegmentRadius)
    val weight = if (chosen) FontWeight.Bold else FontWeight.Medium
    Row(
        Modifier
            .heightIn(min = EinkSizes.SegmentHeight)
            .background(if (chosen) EinkColours.Selected else EinkColours.Paper, shape)
            .border(EinkSizes.Rule, EinkColours.Ink, shape)
            .einkClick(role = Role.Tab, shape = shape, inset = EinkSizes.FocusInsideBorder, onClick = onClick)
            .semantics(mergeDescendants = true) { selected = chosen }
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text(name, style = ChipText.copy(fontWeight = weight))
        if (count != null) {
            Text("$count", style = ChipText.copy(fontWeight = weight, color = EinkColours.Secondary, fontFeatureSettings = "tnum")) // i18n-skip
        }
    }
}

/** A cover in the grid; a long press offers Details and the lists, as the colour Library's does. */
@Composable
private fun LibraryCover(tile: WorkTile, grid: PaperGrid, segment: LibrarySegmentRead) {
    val navigator = LocalNavigator.current
    var menu by remember { mutableStateOf(false) }
    Box(Modifier.longPress { menu = true }) {
        PaperTile(tile, grid) { navigator.openSeries(tile.work, "eink-library") }
        ReadTileMenu(tile, menu, current = segment) { menu = false }
    }
}

/**
 * A long press seen before what is under it: [onLongPress] runs once the finger has rested, and the
 * lift that follows is taken, so the tile under it doesn't open too. A tap passes through.
 */
private fun Modifier.longPress(onLongPress: () -> Unit): Modifier = pointerInput(onLongPress) {
    awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
        val ended = withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
            var gone = false
            while (!gone) {
                val change = awaitPointerEvent(PointerEventPass.Initial).changes.firstOrNull { it.id == down.id }
                gone = change == null || !change.pressed || (change.position - down.position).getDistance() > viewConfiguration.touchSlop
            }
            true
        }
        if (ended == null) {
            onLongPress()
            do {
                val event = awaitPointerEvent(PointerEventPass.Initial)
                event.changes.forEach { it.consume() }
            } while (event.changes.any { it.pressed })
        }
    }
}

/** A list row's height at the text size in use: the title and two lines, 92 dp at least. */
@Composable
private fun listRowHeight(): Dp {
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    return with(density) {
        val title = measurer.measure("Ag", EinkType.RowTitle).size.height // i18n-skip
        val line = measurer.measure("Ag", EinkType.Secondary).size.height // i18n-skip
        maxOf(ListRowLeast, (title + line * 2).toDp() + 22.dp)
    }
}

/**
 * Rows in columns of at least 420 dp with a hairline between them, laid down each column in turn
 * as a printed index reads, a page at a time with the pager under them.
 */
@Composable
private fun PagedRows(
    items: List<WorkTile>,
    page: Int,
    onPage: (Int) -> Unit,
    modifier: Modifier = Modifier,
    row: @Composable (WorkTile, Boolean) -> Unit,
) {
    val height = listRowHeight()
    BoxWithConstraints(modifier.fillMaxSize()) {
        val columns = ((maxWidth + ListColumnGap) / (ListColumnLeast + ListColumnGap)).toInt().coerceAtLeast(1)
        val column = (maxWidth - ListColumnGap * (columns - 1)) / columns
        // The pager only when one page can't hold them all.
        val whole = (maxHeight / height).toInt().coerceAtLeast(1)
        val perColumn = if (items.size <= whole * columns) whole else ((maxHeight - EinkSizes.PagerRow) / height).toInt().coerceAtLeast(1)
        val pages = items.chunked(perColumn * columns)
        val shown = page.coerceIn(0, (pages.size - 1).coerceAtLeast(0))
        val parts = pages.getOrNull(shown).orEmpty().chunked(perColumn)
        Column(Modifier.fillMaxSize()) {
            Row(Modifier.weight(1f).fillMaxWidth()) {
                parts.forEachIndexed { index, part ->
                    if (index > 0) {
                        Box(Modifier.width(ListColumnGap).height(height * parts[0].size), contentAlignment = Alignment.Center) {
                            Box(Modifier.width(EinkSizes.Hairline).fillMaxHeight().background(EinkColours.Tertiary))
                        }
                    }
                    Column(Modifier.width(column)) {
                        part.forEachIndexed { at, tile -> Box(Modifier.height(height)) { row(tile, at == 0) } }
                    }
                }
            }
            if (pages.size > 1) EinkPager(shown, pages.size, onPage)
        }
    }
}

/**
 * A row of the list view: the 48 × 72 cover, the title, the format and where the person is, and
 * New at the trailing edge when a chapter came out; a hairline above all but the first.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun LibraryRow(tile: WorkTile, segment: LibrarySegmentRead, first: Boolean) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    var menu by remember { mutableStateOf(false) }
    val line = tile.next?.let { next -> listOfNotNull(tile.facts.firstOrNull()?.text(context), next.text(context)).joinToString(" · ") } // i18n-skip
        ?: tile.factsLine(context)
    Box {
        Row(
            Modifier
                .fillMaxSize()
                .drawBehind {
                    if (!first) drawLine(EinkColours.Tertiary, Offset(0f, 0f), Offset(size.width, 0f), EinkSizes.Hairline.toPx())
                }
                .combinedClickable(
                    interactionSource = remember { MutableInteractionSource() },
                    // The cover starts at the row's edge: the mark goes round the row, not over it.
                    indication = einkFocus(inset = EinkSizes.FocusOutside),
                    onLongClick = { menu = true },
                    onClick = { navigator.openSeries(tile.work, "eink-library") },
                )
                .semantics(mergeDescendants = true) {},
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            ReadCover(tile.cover, ListCover, Modifier.border(EinkSizes.Hairline, EinkColours.Ink), shape = RoundedCornerShape(0.dp), title = tile.title)
            Column(Modifier.weight(1f)) {
                Text(tile.title, style = EinkType.RowTitle.copy(fontWeight = FontWeight.SemiBold), maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (line.isNotEmpty()) Text(line, style = EinkType.Secondary, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 2.dp))
            }
            if (tile.newChapter) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.size(EinkSizes.NewDot).background(EinkColours.Ink, CircleShape))
                    Text(Strings.einkNew(context), style = EinkType.Secondary, modifier = Modifier.padding(start = 7.dp))
                }
            }
        }
        ReadTileMenu(tile, menu, current = segment) { menu = false }
    }
}

/** An empty list on paper: its title, a line and one outlined button, Search or Add a folder or server. */
@Composable
private fun PaperEmptyList(segment: LibrarySegmentRead, modifier: Modifier) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val store = LocalAppStore.current
    val files = segment == LibrarySegmentRead.YOUR_FILES
    Column(modifier) {
        Text(if (files) Strings.readLibraryFilesEmptyTitle(context) else Strings.readLibraryEmpty(context, listWords(context, segment)), style = EinkType.Body)
        Text(if (files) Strings.readLibraryFilesEmptyLine(context) else Strings.readLibraryEmptyLine(context), style = EinkType.Secondary)
        EinkOutlinedButton(
            if (files) Strings.readLibraryFilesAdd(context) else Strings.homeSearch(context),
            {
                if (files) {
                    navigator.open(BOOK_SOURCES_ROUTE)
                } else {
                    ReadPlaces.search(store.reading, app.kaeri.store.ReadingModel.EMPTY_QUERY)
                    navigator.go(Destination.Search)
                }
            },
            Modifier.padding(top = 16.dp),
            height = 48.dp,
        )
    }
}

// Sheets

/**
 * A sheet on paper rising from the foot: solid paper, a 2 dp ink edge and 16 dp top corners, at
 * most 680 dp wide and centred, over the tab bar. No dim and no motion: it is there in one
 * refresh. A tap above it closes it, as Done does.
 */
@Composable
private fun PaperSheet(title: String, onDone: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    val context = LocalContext.current
    Dialog(onDismissRequest = onDone, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        val view = LocalView.current
        DisposableEffect(view) {
            generateSequence(view.parent) { it.parent }.filterIsInstance<DialogWindowProvider>().firstOrNull()?.window?.apply {
                setWindowAnimations(0)
                setDimAmount(0f)
            }
            onDispose {}
        }
        EinkTheme(LocalEinkLayout.current ?: EinkLook()) {
            Box(
                Modifier.fillMaxSize().pointerInput(onDone) { detectTapGestures { onDone() } },
                contentAlignment = Alignment.BottomCenter,
            ) {
                val shape = RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp)
                Column(
                    Modifier
                        .widthIn(max = SheetWidth)
                        .fillMaxWidth()
                        // The bottom edge sits under the panel's edge: the sheet is open at the foot.
                        .offset(y = EinkSizes.Rule)
                        .background(EinkColours.Paper, shape)
                        .border(EinkSizes.Rule, EinkColours.Ink, shape)
                        // Taps on the sheet stay on it.
                        .pointerInput(Unit) { detectTapGestures { } }
                        .windowInsetsPadding(WindowInsets.navigationBars)
                        .padding(horizontal = 28.dp),
                ) {
                    Row(Modifier.fillMaxWidth().height(66.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(title, style = SheetTitle, modifier = Modifier.weight(1f).semantics { heading() })
                        EinkSolidButton(Strings.actionDone(context), onDone, Modifier.height(48.dp))
                    }
                    content()
                }
            }
        }
    }
}

/** A sheet's title. */
private val SheetTitle = EinkType.Button.copy(fontSize = 21.sp)

/** The View sheet: each row a segmented control that applies at once in the page behind. */
@Composable
private fun ViewSheet(onDone: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    PaperSheet(Strings.einkLibraryView(LocalContext.current), onDone, content)
}

/** A row of the View sheet: its name, then its choices as a segmented control, 128 dp each where they fit. */
@Composable
private fun <T> SheetChoice(label: String, choices: List<T>, chosen: T, first: Boolean = false, words: @Composable (T) -> String, onChoose: (T) -> Unit) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val width = minOf(SheetChoice * choices.size, maxWidth - 120.dp)
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = 72.dp)
                .drawBehind {
                    if (!first) drawLine(EinkColours.Tertiary, Offset(0f, 0f), Offset(size.width, 0f), EinkSizes.Hairline.toPx())
                },
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(label, style = EinkType.Label, modifier = Modifier.weight(1f))
            EinkSegmented(choices, chosen, words, onChoose, Modifier.width(width))
        }
    }
}

/**
 * The lists on a 6 or 7 inch panel and the e-ink phone: each with its count, the one shown grey
 * and bold. Choosing one shows it and closes the sheet.
 */
@Composable
private fun ListsSheet(segment: LibrarySegmentRead, count: (LibrarySegmentRead) -> Int?, onChoose: (LibrarySegmentRead) -> Unit, onDone: () -> Unit) {
    val context = LocalContext.current
    PaperSheet(Strings.einkLibraryListsTitle(context), onDone) {
        Column(Modifier.selectableGroup().padding(bottom = 24.dp)) {
            READ_LISTS.forEachIndexed { index, each ->
                val chosen = each == segment
                val weight = if (chosen) FontWeight.Bold else FontWeight.Medium
                Row(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = EinkSizes.SheetRow)
                        .background(if (chosen) EinkColours.Selected else EinkColours.Paper)
                        .drawBehind {
                            if (index > 0) drawLine(EinkColours.Tertiary, Offset(0f, 0f), Offset(size.width, 0f), EinkSizes.Hairline.toPx())
                        }
                        .einkClick(role = Role.RadioButton) { onChoose(each) }
                        .semantics(mergeDescendants = true) { selected = chosen }
                        .padding(horizontal = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(listWords(context, each), style = EinkType.Body.copy(fontWeight = weight), modifier = Modifier.weight(1f))
                    count(each)?.let { Text("$it", style = EinkType.Body.copy(fontWeight = weight, fontFeatureSettings = "tnum")) } // i18n-skip
                }
            }
        }
    }
}
