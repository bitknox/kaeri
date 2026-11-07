// Library's reading side: AniList's six reading lists, then Your files for series in folders and
// servers that are on no list, as M3 filter chips in one scrolling line, and the chosen list's
// covers. With Both, the Shows or Reading control sits first. An empty list is a title, a line
// and one outlined button, with no picture.

package app.kaeri.ui.reading

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.CoverPlate
import app.kaeri.design.tabular
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.text
import app.kaeri.ui.AfterWait
import app.kaeri.ui.AppActions
import app.kaeri.ui.Destination
import app.kaeri.ui.DownloadsRow
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.Menu
import app.kaeri.ui.PosterGrid
import app.kaeri.ui.Refreshable
import app.kaeri.ui.ScreenFrame
import app.kaeri.ui.TextPlaceholder
import app.kaeri.ui.itemMotion
import app.kaeri.ui.kaeriIsFor
import app.kaeri.ui.profileAccount
import app.kaeri.ui.storageLine
import app.kaeri.ui.wideWindow

/** How Library orders a reading list. */
private enum class ReadSort { LastRead, Title }

@Composable
internal fun ReadLibraryScreen(actions: AppActions) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val lists by store.reading.lists.collectAsStateWithLifecycle()
    val failures by store.reading.failures.collectAsStateWithLifecycle()
    val downloads by store.reading.downloads.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val user = trackers.values.firstOrNull { it.connected }?.user
    val both = kaeriIsFor() == KaeriIsFor.BOTH
    val segment = ReadPlaces.list
    var sort by rememberSaveable { mutableStateOf(ReadSort.LastRead) }
    var sorting by remember { mutableStateOf(false) }
    // Every list once, for the chips' counts; the chosen one again on each visit.
    LaunchedEffect(Unit) {
        store.reading.loadDownloads()
        for (each in READ_LISTS) if (each != segment) store.reading.loadList(each)
    }
    LaunchedEffect(segment) { store.reading.loadList(segment) }
    val sortWords = { each: ReadSort -> if (each == ReadSort.LastRead) Strings.readSortLastRead(context) else Strings.librarySortTitle(context) }

    ScreenFrame(
        title = stringResource(R.string.nav_library),
        account = if (wideWindow()) null else profileAccount(user, actions.openSettings),
        actions = {
            Box {
                IconButton(onClick = { sorting = true }) {
                    Icon(Icons.Sort, contentDescription = Strings.sortButton(context, sortWords(sort)))
                }
                Menu(expanded = sorting, onDismissRequest = { sorting = false }) {
                    for (each in ReadSort.entries) {
                        DropdownMenuItem(
                            text = { Text(sortWords(each)) },
                            leadingIcon = {
                                Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
                                    if (each == sort) Icon(Icons.Check, contentDescription = stringResource(R.string.library_sort_current))
                                }
                            },
                            onClick = { sorting = false; sort = each },
                        )
                    }
                }
            }
        },
    ) { padding ->
        val top = padding.calculateTopPadding()
        val tiles = lists[segment]?.let { list ->
            when (sort) {
                ReadSort.LastRead -> list
                ReadSort.Title -> list.sortedBy { it.title.lowercase(app.kaeri.store.AirTime.locale) }
            }
        }
        val failure = failures[ReadingAbout.LIBRARY]
        Refreshable({ store.reading.loadList(segment) }, top = top) {
            PosterGrid(PaddingValues(start = 16.dp, end = 16.dp, top = top, bottom = 24.dp)) {
                if (both) {
                    item(span = { GridItemSpan(maxLineSpan) }, key = "scope", contentType = "scope") {
                        ScopeControl(reading = true, modifier = itemMotion().padding(top = 4.dp, bottom = 4.dp)) { ReadPlaces.libraryReading = it }
                    }
                }
                item(span = { GridItemSpan(maxLineSpan) }, key = "downloads", contentType = "downloads") {
                    val total = downloads?.total.orEmpty()
                    val detail = if (total.isEmpty()) storageLine(context, 0) else total.joinToString(" · ") { it.text(context) } // i18n-skip
                    DownloadsRow(detail, inset = false, modifier = itemMotion())
                }
                item(span = { GridItemSpan(maxLineSpan) }, key = "lists", contentType = "lists") {
                    ListChips(segment, lists, itemMotion())
                }
                when {
                    tiles == null && failure != null -> item(span = { GridItemSpan(maxLineSpan) }, key = "quiet", contentType = "quiet") {
                        Box(itemMotion()) {
                            PlainEmpty(
                                Strings.libraryListLoadFailed(context, listWords(context, segment)),
                                failureWords(context, failure),
                                Modifier.padding(top = 16.dp),
                                inset = 0.dp,
                                action = Strings.actionTryAgain(context),
                                onAction = { store.reading.loadList(segment) },
                            )
                        }
                    }
                    tiles == null -> items(9, key = { "placeholder-$it" }, contentType = { "placeholder" }) {
                        Box(itemMotion()) { AfterWait { CellPlaceholder() } }
                    }
                    tiles.isEmpty() -> item(span = { GridItemSpan(maxLineSpan) }, key = "quiet", contentType = "quiet") {
                        Box(itemMotion()) { EmptyList(segment) }
                    }
                    else -> items(tiles, key = { it.work }, contentType = { "cover" }) { tile ->
                        Box(itemMotion()) { LibraryCell(tile, segment) }
                    }
                }
            }
        }
    }
}

/** The lists as filter chips in one scrolling line, each with its count once it is held. */
@Composable
private fun ListChips(segment: LibrarySegmentRead, lists: Map<LibrarySegmentRead, List<WorkTile>>, modifier: Modifier) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val row = rememberLazyListState()
    // The chosen list's chip in view, as a link from Home may choose one off screen; again once
    // the counts land, as they widen the chips before it.
    LaunchedEffect(segment, lists.size) { row.animateScrollToItem(READ_LISTS.indexOf(segment).coerceAtLeast(0)) }
    LazyRow(modifier, state = row, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        items(READ_LISTS, key = { it.name }) { each ->
            val selected = each == segment
            val count = lists[each]?.size
            FilterChip(
                selected = selected,
                onClick = { ReadPlaces.list = each },
                label = {
                    Text(listWords(context, each))
                    if (count != null) {
                        Spacer(Modifier.size(8.dp))
                        Text("$count", style = MaterialTheme.typography.labelLarge.tabular().copy(fontWeight = FontWeight.Normal)) // i18n-skip
                    }
                },
                leadingIcon = if (selected) ({ Icon(Icons.Check, contentDescription = null, modifier = Modifier.size(FilterChipDefaults.IconSize)) }) else null,
                colors = FilterChipDefaults.filterChipColors(
                    selectedContainerColor = colors.secondaryContainer,
                    selectedLabelColor = colors.onSecondaryContainer,
                    selectedLeadingIconColor = colors.onSecondaryContainer,
                ),
                modifier = Modifier.height(48.dp).padding(vertical = 8.dp),
            )
        }
    }
}

/** A cover in the grid with its line, "Chapter 41 of 43"; a long press offers Details and the lists. */
@Composable
private fun LibraryCell(tile: WorkTile, segment: LibrarySegmentRead) {
    var menu by remember { mutableStateOf(false) }
    BoxWithConstraints {
        ReadTile(tile, maxWidth, from = "read-library", lines = if (wideWindow()) 1 else 2, onLongClick = { menu = true })
        ReadTileMenu(tile, menu, current = segment) { menu = false }
    }
}

/** An empty list: its title, a line and one outlined button, Search or Add a folder or server. */
@Composable
private fun EmptyList(segment: LibrarySegmentRead) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val store = LocalAppStore.current
    if (segment == LibrarySegmentRead.YOUR_FILES) {
        PlainEmpty(
            Strings.readLibraryFilesEmptyTitle(context),
            Strings.readLibraryFilesEmptyLine(context),
            Modifier.padding(top = 16.dp),
            inset = 0.dp,
            action = Strings.readLibraryFilesAdd(context),
            onAction = { navigator.open(BOOK_SOURCES_ROUTE) },
        )
    } else {
        PlainEmpty(
            Strings.readLibraryEmpty(context, listWords(context, segment)),
            Strings.readLibraryEmptyLine(context),
            Modifier.padding(top = 16.dp),
            inset = 0.dp,
            action = Strings.homeSearch(context),
            onAction = {
                ReadPlaces.search(store.reading, app.kaeri.store.ReadingModel.EMPTY_QUERY)
                navigator.go(Destination.Search)
            },
        )
    }
}

/** A cell before its list arrives: the cover's plate and two lines. */
@Composable
private fun CellPlaceholder() {
    Column {
        CoverPlate(null, Modifier.fillMaxWidth().aspectRatio(2f / 3f).clip(coverShape(112.dp)))
        TextPlaceholder(0.8f, 22.dp, Modifier.padding(top = 4.dp))
        TextPlaceholder(0.6f, 18.dp)
    }
}
