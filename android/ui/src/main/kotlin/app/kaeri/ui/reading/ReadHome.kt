// Reading's Home: the spotlight, Continue as covers with a silk track, This week by day, Up next
// for you, the discovery rows (ReadDiscovery.kt) and the closing line. The engine
// sends the whole view and decides the spotlight's case and words; this draws it. A section with
// nothing leaves with its heading.

package app.kaeri.ui.reading

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.kaeri.design.ArtShape
import app.kaeri.design.CoverPlate
import app.kaeri.design.Motion
import app.kaeri.design.ShowTitle
import app.kaeri.design.tabular
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.KaeriIsFor
import app.kaeri.engine.model.LibrarySegmentRead
import app.kaeri.engine.model.ReadHomeEnd
import app.kaeri.engine.model.ReadHomePart
import app.kaeri.engine.model.ReadSpotlight
import app.kaeri.engine.model.ReadSpotlightCase
import app.kaeri.engine.model.ReadWeekDay
import app.kaeri.engine.model.ReadWeekEntry
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingHomeView
import app.kaeri.engine.model.WorkTile
import app.kaeri.i18n.R
import app.kaeri.i18n.Strings
import app.kaeri.store.AirTime
import app.kaeri.store.AppClock
import app.kaeri.store.ReadingModel
import app.kaeri.store.text
import app.kaeri.ui.AfterWait
import app.kaeri.ui.AppActions
import app.kaeri.ui.ArtworkImage
import app.kaeri.ui.DOWNLOADS_ROUTE
import app.kaeri.ui.Destination
import app.kaeri.ui.FrameLine
import app.kaeri.ui.Icons
import app.kaeri.ui.LocalAppStore
import app.kaeri.ui.LocalNavigator
import app.kaeri.ui.LocalSnackbars
import app.kaeri.ui.Menu
import app.kaeri.ui.PLACEHOLDERS_AFTER
import app.kaeri.ui.PROVIDERS_ROUTE
import app.kaeri.ui.PosterPlaceholder
import app.kaeri.ui.Refreshable
import app.kaeri.ui.ScreenFrame
import app.kaeri.ui.Shared
import app.kaeri.ui.SpotlightPlaceholder
import app.kaeri.ui.StateBlock
import app.kaeri.ui.TextPlaceholder
import app.kaeri.ui.WaterDot
import app.kaeri.ui.cardGap
import app.kaeri.ui.itemMotion
import app.kaeri.ui.kaeriIsFor
import app.kaeri.ui.pixels
import app.kaeri.ui.posterWidth
import app.kaeri.ui.profileAccount
import app.kaeri.ui.sharedWork
import app.kaeri.ui.shelfTop
import app.kaeri.ui.wideWindow
import java.time.Instant
import java.time.format.TextStyle
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** How long a part waits before the activity line says it in words, as the Watch Home's does. */
private const val ACTIVITY_AFTER = 4_000L

@Composable
internal fun ReadHomeScreen(actions: AppActions) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val snackbars = LocalSnackbars.current
    val home by store.reading.home.collectAsStateWithLifecycle()
    val failures by store.reading.failures.collectAsStateWithLifecycle()
    val offline by store.offline.collectAsStateWithLifecycle()
    val trackers by store.accounts.trackers.collectAsStateWithLifecycle()
    val user = trackers.values.firstOrNull { it.connected }?.user
    LaunchedEffect(Unit) { store.reading.loadHome() }
    DemoLaunchHost(actions)

    val view = home
    val failure = failures[ReadingAbout.HOME]
    // A part that did not load while the rest stands: one notice with Try again, as the Watch Home.
    val failedText = stringResource(R.string.home_failed)
    val tryAgain = stringResource(R.string.home_try_again)
    LaunchedEffect(view?.failed) {
        if (view?.failed == true) snackbars.show(failedText, tryAgain) { store.reading.loadHome(refresh = true) }
    }

    // Series taken off Continue, gone at once while their Undo stands.
    val gone = remember { mutableStateListOf<WorkId>() }
    val waiting = (view == null && failure == null) || view?.pending.orEmpty().isNotEmpty()
    val slow = pendingForLong(waiting)
    val since = view?.offlineSince
    val line = when {
        view?.offline == true && since != null -> FrameLine.OfflineFrom(Strings.homeOffline(context, clockWords(since)))
        else -> null
    }
    ScreenFrame(
        // With Read alone, reading's Home is the Home; beside Watch it is Read, as the bar says.
        title = stringResource(if (kaeriIsFor() == KaeriIsFor.READ) R.string.home_title else R.string.nav_read),
        account = if (wideWindow()) null else profileAccount(user, actions.openSettings),
        line = line,
        ownLine = true,
        activity = if (slow && view?.offline != true) Strings.homeLoading(context) else null,
    ) { padding ->
        val top = padding.calculateTopPadding()
        Refreshable({ store.reading.loadHome(refresh = true) }, top = top) {
            val nothingKept = view == null || (view.offline && since == null && view.spotlight == null && view.continueReading.isEmpty())
            when {
                view == null && failure != null && failure !is app.kaeri.engine.model.ReadingFailure.Offline ->
                    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = top)) {
                        item(key = "failed") {
                            PlainEmpty(
                                Strings.homeFailed(context),
                                failureWords(context, failure),
                                Modifier.padding(top = 24.dp),
                                action = Strings.homeTryAgain(context),
                                onAction = { store.reading.loadHome(refresh = true) },
                            )
                        }
                    }
                // Offline with nothing kept: the way to what opens without the network.
                nothingKept && (offline || failure is app.kaeri.engine.model.ReadingFailure.Offline || view?.offline == true) ->
                    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = top)) {
                        item(key = "offline") { ReadOfflineHome() }
                    }
                view == null -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = top, bottom = 24.dp), userScrollEnabled = false) {
                    item(key = "loading") { AfterWait { Column { SpotlightPlaceholder(); ShelfPlaceholder() } } }
                }
                else -> ReadHomeColumn(view, actions, gone, top)
            }
        }
    }
}

@Composable
private fun ReadOfflineHome() {
    val navigator = LocalNavigator.current
    val context = LocalContext.current
    StateBlock(
        glyph = { Icon(Icons.Offline, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(32.dp)) },
        title = Strings.homeOfflineEmptyTitle(context),
        line = Strings.readHomeOfflineLine(context),
    ) {
        FilledTonalButton(onClick = { navigator.open(DOWNLOADS_ROUTE) }) { Text(Strings.homeOpenDownloads(context)) }
    }
}

/** Whether something has been pending for four seconds: the activity line's cue. */
@Composable
private fun pendingForLong(pending: Boolean): Boolean {
    var past by remember { mutableStateOf(false) }
    LaunchedEffect(pending) {
        past = false
        if (pending) {
            delay(ACTIVITY_AFTER)
            past = true
        }
    }
    return pending && past
}

/**
 * Whether placeholders stand for [pending]: not for the first 600 ms, so a fast answer never shows
 * them, and once shown, at least 0.3 s, so they never flicker away.
 */
@Composable
private fun heldPlaceholders(pending: Boolean): Boolean {
    var shown by remember { mutableStateOf(false) }
    var since by remember { mutableLongStateOf(0L) }
    LaunchedEffect(pending) {
        if (pending) {
            if (!shown) {
                delay(PLACEHOLDERS_AFTER)
                since = withFrameMillis { it }
                shown = true
            }
        } else if (shown) {
            val left = PLACEHOLDERS_HELD - (withFrameMillis { it } - since)
            if (left > 0) delay(left)
            shown = false
        }
    }
    return shown
}

@Composable
private fun ReadHomeColumn(home: ReadingHomeView, actions: AppActions, gone: MutableList<WorkId>, top: Dp) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val store = LocalAppStore.current
    val goneSet = gone.toSet()
    val hide = rememberHide(gone)
    val yours = heldPlaceholders(ReadHomePart.YOURS in home.pending)
    val discovery = heldPlaceholders(ReadHomePart.DISCOVERY in home.pending || ReadHomePart.BECAUSE in home.pending)
    // While the demo is the only thing to read, its own spotlight and end, as the demo's board draws them.
    val demo by store.demo.collectAsStateWithLifecycle()
    val viewAll = rememberViewAll()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = top, bottom = 24.dp)) {
        // The spotlight changes in place, a crossfade when its series or case moves.
        item(key = "spotlight", contentType = "spotlight") {
            val tile = home.spotlight?.takeUnless { it.work in goneSet }
            val spot = home.spot
            if (demo.onlyBooks) {
                Box(itemMotion()) {
                    Column {
                        DemoHomeSpotlight(actions, Modifier.padding(top = 8.dp))
                        DemoOnlyLine()
                    }
                }
            } else Box(itemMotion()) {
                AnimatedContent(
                    targetState = if (tile != null && spot != null) tile to spot else null,
                    contentKey = { it?.let { (t, s) -> t.work to s.case } },
                    transitionSpec = { fadeIn(Motion.fadeIn()) togetherWith fadeOut(Motion.fadeOut()) },
                    label = "read-spotlight", // i18n-skip
                ) { shown ->
                    if (shown != null) ReadSpotlightBlock(shown.first, shown.second, actions, hide)
                }
            }
        }
        val continuing = home.continueReading.filter { it.work !in goneSet }
        if (continuing.isNotEmpty()) {
            item(key = "continue", contentType = "shelf") {
                Box(itemMotion()) { ContinueShelf(continuing, actions, hide) }
            }
        }
        if (home.week.isNotEmpty()) {
            item(key = "week", contentType = "week") { Box(itemMotion()) { ReadWeek(home.week) } }
        }
        if (yours) {
            item(key = "yours-pending", contentType = "pending") { Box(itemMotion()) { ShelfPlaceholder() } }
        }
        if (home.upNext.isNotEmpty()) {
            item(key = "up-next", contentType = "shelf") {
                Box(itemMotion()) {
                    TileShelf(Strings.homeUpNext(context), home.upNext, "read-up-next", Strings.homeViewAll(context)) {
                        ReadPlaces.showList(LibrarySegmentRead.PLANNING)
                        navigator.go(Destination.Library)
                    }
                }
            }
        }
        // Discovery: the engine's rows in its order, each landing in place over its placeholders.
        // While only the demo is on they show too, as the first run's catalogue to browse.
        val eink = LocalEinkLayout.current
        val rows = home.rows.filter { it.tiles.isNotEmpty() || it.pending }
        if (eink != null && rows.isNotEmpty()) {
            // E-ink keeps Home short: one Browse row opens the list of rows, or the list itself
            // when there is nothing of the person's own above it.
            val own = home.continueReading.isNotEmpty() || home.week.isNotEmpty() || home.upNext.isNotEmpty()
            item(key = "browse", contentType = "browse") { Box(rowMotion()) { BrowseEntry(rows, inline = !own) } }
        } else {
            items(rows, key = { "row-${it.id}" }, contentType = { "row" }) { row ->
                Box(rowMotion()) { DiscoveryRow(row, viewAll) }
            }
        }
        // Before the engine has planned any rows, two shelves' placeholders hold their place.
        if (rows.isEmpty() && discovery) {
            item(key = "discovery-pending", contentType = "pending") {
                Box(itemMotion()) { Column { ShelfPlaceholder(); ShelfPlaceholder() } }
            }
        }
        if (rows.isEmpty() && home.offline && !discovery) {
            item(key = "offline-rows", contentType = "end") { Box(itemMotion()) { OfflineRowsLine() } }
        }
        if (demo.onlyBooks) {
            // The demo's line sits under its spotlight, so the end is the everyday one.
            item(key = "end", contentType = "end") { Box(itemMotion()) { ReadEndLine(ReadHomeEnd.EVERYTHING) } }
        } else home.end?.let { end ->
            item(key = "end", contentType = "end") { Box(itemMotion()) { ReadEndLine(end) } }
        }
    }
}

/**
 * Remove from Continue: the cover goes at once and a notice offers Undo; only once the notice
 * has gone does the engine hide it, as there is nothing to undo there.
 */
@Composable
private fun rememberHide(gone: MutableList<WorkId>): (WorkId) -> Unit {
    val store = LocalAppStore.current
    val snackbars = LocalSnackbars.current
    val scope = rememberCoroutineScope()
    val hidden = stringResource(R.string.continue_hidden)
    val undo = stringResource(R.string.action_undo)
    return remember(store, snackbars, hidden, undo) {
        { work ->
            gone += work
            scope.launch {
                var undone = false
                try {
                    snackbars.host.currentSnackbarData?.dismiss()
                    val result = snackbars.host.showSnackbar(hidden, actionLabel = undo, withDismissAction = true, duration = SnackbarDuration.Indefinite)
                    undone = result == SnackbarResult.ActionPerformed
                } finally {
                    if (!undone) store.reading.hideFromContinue(work)
                    gone -= work
                }
            }
        }
    }
}

// The spotlight

/**
 * The one series at the top: art above the text on a phone, beside it at 700 dp and wider. Its
 * label, title, facts in small capitals, synopsis, what is new or where you stopped, then the one
 * amber action, 56 high and full width on a phone.
 */
@Composable
private fun ReadSpotlightBlock(tile: WorkTile, spot: ReadSpotlight, actions: AppActions, hide: (WorkId) -> Unit) {
    val configuration = LocalConfiguration.current
    val side = configuration.screenWidthDp >= 700
    val short = configuration.screenHeightDp < 480
    val key = readerCoverKey("spotlight", tile.work)
    val navigator = LocalNavigator.current
    val art: @Composable (Modifier) -> Unit = { m ->
        SpotlightArt(
            spot.banner,
            tile.cover,
            m.aspectRatio(16f / 9f)
                .clip(ArtShape.banner)
                .clickable(onClickLabel = stringResource(R.string.title_open), role = Role.Button) { navigator.openSeries(tile.work, "read-spotlight") },
            zoom = Modifier.readerZoomSource(key),
        )
    }
    val page = Modifier.sharedWork(Shared.Page, tile.work, "read-spotlight")
    if (side) {
        BoxWithConstraints(Modifier.fillMaxWidth().padding(horizontal = 16.dp).then(page)) {
            // The art takes a little over half, and at most 560, so the text keeps its column.
            val artWidth = if (short) 340.dp else minOf(maxWidth * 0.56f, 560.dp)
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(32.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                SpotlightText(tile, spot, side = true, actions, hide, Modifier.weight(1f))
                art(Modifier.width(artWidth))
            }
        }
    } else {
        Column(Modifier.padding(horizontal = 16.dp).then(page)) {
            art(Modifier.fillMaxWidth())
            SpotlightText(tile, spot, side = false, actions, hide, Modifier.padding(top = 16.dp))
        }
    }
}

/** The banner, else the cover at its own aspect against a plate, from the leading edge. */
@Composable
private fun SpotlightArt(banner: String?, cover: String?, modifier: Modifier, zoom: Modifier) {
    BoxWithConstraints(modifier) {
        if (banner != null) {
            ArtworkImage(banner, plate = null, maxPixelSize = pixels(maxWidth), modifier = Modifier.fillMaxSize().then(zoom))
        } else {
            val tall = maxHeight
            val inset = tall * 0.06f
            Box(Modifier.fillMaxSize()) {
                CoverPlate(null, Modifier.fillMaxSize())
                ArtworkImage(
                    cover,
                    plate = null,
                    maxPixelSize = pixels(tall),
                    modifier = Modifier
                        .align(Alignment.CenterStart)
                        .padding(start = inset)
                        .fillMaxHeight(fraction = 0.88f)
                        .aspectRatio(2f / 3f)
                        .clip(coverShape(tall * 0.6f))
                        .then(zoom),
                )
            }
        }
    }
}

@Composable
private fun SpotlightText(tile: WorkTile, spot: ReadSpotlight, side: Boolean, actions: AppActions, hide: (WorkId) -> Unit, modifier: Modifier) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    // The other title under the shown one, from the series' own page once it is held.
    LaunchedEffect(tile.work) { store.reading.loadSeriesInfo(tile.work) }
    val series by store.reading.series.collectAsStateWithLifecycle()
    val other = series[tile.work]?.titles?.let { t ->
        listOfNotNull(t.romaji, t.native, t.english).firstOrNull { it.isNotBlank() && it != tile.title }
    }
    Column(modifier) {
        Column(Modifier.semantics(mergeDescendants = true) {}) {
            Text(spot.label.text(context), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
            Text(
                tile.title,
                style = ShowTitle.page,
                modifier = Modifier.padding(top = 8.dp),
            )
            if (other != null) {
                Text(other, style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant, modifier = Modifier.padding(top = 2.dp))
            }
            if (spot.facts.isNotEmpty()) {
                Text(
                    spot.facts.joinToString(" · ") { it.text(context) }.uppercase(AirTime.locale), // i18n-skip
                    style = MaterialTheme.typography.labelSmall.copy(letterSpacing = 0.9.sp, fontWeight = FontWeight.Medium),
                    color = colors.onSurfaceVariant,
                    modifier = Modifier.padding(top = 10.dp),
                )
            }
            spot.synopsis?.takeIf { it.isNotBlank() }?.let { synopsis ->
                Text(
                    synopsis.replace(Regex("\\s*\\n+\\s*"), " "),
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(top = 12.dp),
                )
            }
            SpotlightLine(tile, spot, Modifier.padding(top = 16.dp))
        }
        SpotlightActions(tile, spot, side, actions, hide, Modifier.padding(top = 16.dp))
    }
}

/** Under the synopsis: what is new with the water dot, where you stopped with the track, or the engine's line. */
@Composable
private fun SpotlightLine(tile: WorkTile, spot: ReadSpotlight, modifier: Modifier) {
    val context = LocalContext.current
    val colors = MaterialTheme.colorScheme
    val style = MaterialTheme.typography.bodyMedium.tabular()
    val line = spot.line ?: return
    when (spot.case) {
        ReadSpotlightCase.NEW_CHAPTER -> Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            WaterDot()
            Text(line.text(context), style = style, color = colors.onSurface)
        }
        ReadSpotlightCase.CONTINUING -> Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            tile.progress?.let { SilkTrack(it, Modifier.width(120.dp)) }
            Text(line.text(context), style = style, color = colors.onSurfaceVariant)
        }
        ReadSpotlightCase.UP_NEXT, ReadSpotlightCase.POPULAR -> Text(line.text(context), modifier, style = style, color = colors.onSurfaceVariant)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SpotlightActions(tile: WorkTile, spot: ReadSpotlight, side: Boolean, actions: AppActions, hide: (WorkId) -> Unit, modifier: Modifier) {
    val store = LocalAppStore.current
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    var planned by rememberSaveable(tile.work) { mutableStateOf(spot.planned) }
    var menu by remember { mutableStateOf(false) }
    val words = actionWords(context, spot.action)
    val full = if (side) Modifier else Modifier.fillMaxWidth()
    // 56 high and full width on a phone; beside the art one row with the rest, all 48.
    val mainHeight = if (side) 48.dp else 56.dp
    val secondHeight = if (side) 48.dp else 44.dp
    val key = readerCoverKey("spotlight", tile.work)
    val action = spot.action
    val main: (@Composable () -> Unit)? = when {
        words != null && action != null -> {
            {
                Button(
                    onClick = {
                        val chapter = action.chapter
                        // Read again asks first, on the series page; the rest open the reader.
                        if (chapter != null) actions.openReader(tile.work, chapter, tile.cover, key) else navigator.openSeries(tile.work, "read-spotlight")
                    },
                    modifier = full.heightIn(min = mainHeight),
                ) {
                    Icon(Icons.Read, contentDescription = null, modifier = Modifier.size(20.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(words, style = MaterialTheme.typography.titleMedium.tabular())
                }
            }
        }
        action == null && !planned -> {
            {
                Button(
                    onClick = {
                        planned = true
                        store.reading.setStatus(tile.work, LibrarySegmentRead.PLANNING)
                    },
                    modifier = full.heightIn(min = mainHeight),
                ) {
                    Icon(Icons.Plus, contentDescription = null, modifier = Modifier.size(20.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(Strings.libraryAddPlanning(context), style = MaterialTheme.typography.titleMedium)
                }
            }
        }
        else -> null
    }
    val second: @Composable (Modifier) -> Unit = { stretch ->
        // Details, or On your Planning list with a check where the series is planned already.
        FilledTonalButton(onClick = { navigator.openSeries(tile.work, "read-spotlight") }, modifier = stretch.heightIn(min = secondHeight)) {
            if (planned && action == null) {
                Icon(Icons.Check, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text(Strings.readingOnPlanning(context), maxLines = 1, overflow = TextOverflow.Ellipsis)
            } else {
                Text(Strings.homeDetails(context), maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        // More: Remove from Continue, for a series in Continue.
        if (spot.case == ReadSpotlightCase.NEW_CHAPTER || spot.case == ReadSpotlightCase.CONTINUING) {
            Box {
                FilledTonalIconButton(onClick = { menu = true }, modifier = Modifier.size(secondHeight)) {
                    Icon(Icons.More, contentDescription = Strings.homeMore(context))
                }
                Menu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text(Strings.homeRemoveContinue(context)) },
                        leadingIcon = { Icon(Icons.HideRow, null) },
                        onClick = { menu = false; hide(tile.work) },
                    )
                }
            }
        }
    }
    if (side) {
        FlowRow(
            modifier,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            itemVerticalAlignment = Alignment.CenterVertically,
        ) {
            main?.invoke()
            second(Modifier)
        }
    } else {
        Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
            main?.invoke()
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                second(Modifier.weight(1f))
            }
        }
    }
}

// Shelves

/** Continue: covers with the silk track, up to twelve; a long press offers Read, Details and Remove. */
@Composable
private fun ContinueShelf(tiles: List<WorkTile>, actions: AppActions, hide: (WorkId) -> Unit) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    Column(Modifier.padding(top = shelfTop())) {
        ReadShelfHeading(Strings.navContinue(context))
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(cardGap())) {
            items(tiles.take(12), key = { "c-${it.work}" }, contentType = { "cover" }) { tile ->
                var menu by remember { mutableStateOf(false) }
                val key = readerCoverKey("continue", tile.work)
                Box(itemMotion()) {
                    ReadTile(
                        tile,
                        posterWidth(),
                        from = "read-continue",
                        track = true,
                        lines = 2,
                        cover = Modifier.readerZoomSource(key),
                        onLongClick = { menu = true },
                    )
                    Menu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(
                            text = { Text(Strings.seriesReadChapterMenu(context)) },
                            leadingIcon = { Icon(Icons.Read, null) },
                            onClick = { menu = false; actions.openReader(tile.work, null, tile.cover, key) },
                        )
                        DropdownMenuItem(
                            text = { Text(Strings.homeDetails(context)) },
                            leadingIcon = { Icon(Icons.External, null) },
                            onClick = { menu = false; navigator.openSeries(tile.work, "read-continue") },
                        )
                        HorizontalDivider(Modifier.padding(vertical = 8.dp), color = MaterialTheme.colorScheme.outlineVariant)
                        DropdownMenuItem(
                            text = { Text(Strings.homeRemoveContinue(context)) },
                            leadingIcon = { Icon(Icons.HideRow, null) },
                            onClick = { menu = false; hide(tile.work) },
                        )
                    }
                }
            }
        }
    }
}

/** A discovery shelf: covers, each with its line, and the heading's link when it has one. */
@Composable
private fun TileShelf(title: String, tiles: List<WorkTile>, from: String, link: String? = null, onLink: () -> Unit = {}) {
    Column(Modifier.padding(top = shelfTop())) {
        ReadShelfHeading(title, link, onLink)
        LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(cardGap())) {
            items(tiles, key = { "$from-${it.work}" }, contentType = { "cover" }) { tile ->
                Box(itemMotion()) { ReadTile(tile, posterWidth(), from = from, lines = 2) }
            }
        }
    }
}

/** A shelf before it arrives: its heading's bar and four covers' plates. */
@Composable
private fun ShelfPlaceholder() {
    Column(Modifier.padding(top = shelfTop())) {
        TextPlaceholder(0.3f, 48.dp, Modifier.padding(start = 16.dp))
        LazyRow(
            userScrollEnabled = false,
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(cardGap()),
        ) {
            items(4) { Box(Modifier.width(posterWidth())) { PosterPlaceholder() } }
        }
    }
}

// This week

/**
 * This week by day from today. A phone sets the days in a sideways strip, 150 wide each; wider
 * windows set them side by side, four or seven.
 */
@Composable
private fun ReadWeek(days: List<ReadWeekDay>) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val width = LocalConfiguration.current.screenWidthDp
    Column(Modifier.padding(top = shelfTop())) {
        // Updates has no tab of its own beside Watch: it opens from here.
        ReadShelfHeading(Strings.homeThisWeek(context), Strings.navUpdates(context)) { navigator.go(Destination.Updates) }
        HorizontalDivider(Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp), color = MaterialTheme.colorScheme.outlineVariant)
        if (width >= 700) {
            val shown = if (width >= 900) 7 else 4
            Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                // A column for each day, filled or not, so the days keep their width.
                for (index in 0 until shown) Box(Modifier.weight(1f)) { days.getOrNull(index)?.let { ReadDay(it) } }
            }
        } else {
            LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                items(days, key = { "day-${it.day}" }) { day -> Box(Modifier.width(150.dp)) { ReadDay(day) } }
            }
        }
    }
}

/** One day: its name, then what came out or usually comes, at most three, then how many more. */
@Composable
private fun ReadDay(day: ReadWeekDay) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    val name = if (day.today) Strings.homeToday(context)
    else Instant.ofEpochSecond(day.day).atZone(AppClock.zone).dayOfWeek.getDisplayName(TextStyle.SHORT, AirTime.locale)
    Column(Modifier.fillMaxWidth()) {
        Text(
            name,
            style = MaterialTheme.typography.labelLarge,
            color = if (day.today) colors.onSurface else colors.onSurfaceVariant,
            modifier = Modifier.padding(bottom = 4.dp).semantics { heading() },
        )
        if (day.entries.isEmpty()) {
            Text(Strings.weekNothingUsually(context), style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
        }
        for (entry in day.entries.take(3)) ReadWeekCell(entry)
        if (day.entries.size > 3) {
            TextButton(
                onClick = { navigator.go(Destination.Updates) },
                colors = ButtonDefaults.textButtonColors(contentColor = colors.onSurface),
                contentPadding = PaddingValues(0.dp),
            ) { Text(Strings.homeDayMore(context, day.entries.size - 3), style = MaterialTheme.typography.bodySmall) }
        }
    }
}

/** A series due that day: its small cover, the time it came out with the water dot or Usually, the title and the chapter. */
@Composable
private fun ReadWeekCell(entry: ReadWeekEntry) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val colors = MaterialTheme.colorScheme
    val tile = entry.tile
    val out = entry.outAt
    // The chapter that came out this day; for one only expected, the chapter the engine expects,
    // and nothing when it has no confident guess.
    val chapter = if (entry.usually) entry.chapter?.text(context) else (entry.chapter ?: tile.next)?.text(context)
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .sharedWork(Shared.Page, tile.work, "read-week")
            .clickable(onClickLabel = stringResource(R.string.title_open), role = Role.Button) { navigator.openSeries(tile.work, "read-week") }
            .padding(vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        ReadCover(tile.cover, 40.dp, Modifier.sharedWork(Shared.Cover, tile.work, "read-week"))
        Column(Modifier.weight(1f)) {
            if (out != null && !entry.usually) {
                // The time it came out, in silk. The water dot waits for words that say new.
                Text(clockWords(out), style = MaterialTheme.typography.labelMedium.tabular(), color = colors.onSurface)
            } else {
                Text(Strings.weekUsually(context), style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant)
            }
            Text(tile.title, style = ShowTitle.caption, maxLines = 2, overflow = TextOverflow.Ellipsis)
            chapter?.let { Text(it, style = MaterialTheme.typography.bodySmall.tabular(), color = colors.onSurfaceVariant) }
        }
    }
}

// The end

/** The closing line: Updates and Search as links, or Add a source while there is none. */
@Composable
private fun ReadEndLine(end: ReadHomeEnd) {
    val context = LocalContext.current
    val navigator = LocalNavigator.current
    val store = LocalAppStore.current
    val colors = MaterialTheme.colorScheme
    Column(Modifier.padding(start = 16.dp, end = 16.dp, top = 32.dp)) {
        HorizontalDivider(color = colors.outlineVariant)
        Spacer(Modifier.height(16.dp))
        when (end) {
            ReadHomeEnd.EVERYTHING -> {
                val text = Strings.homeReadEnd(context)
                val links = listOf(
                    Strings.navUpdates(context) to { navigator.go(Destination.Updates) },
                    Strings.homeSearch(context) to {
                        ReadPlaces.search(store.reading, ReadingModel.EMPTY_QUERY)
                        navigator.go(Destination.Search)
                    },
                )
                Text(linked(text, links), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
            }
            ReadHomeEnd.NO_SOURCES -> {
                Text(Strings.homeReadNoSources(context), style = MaterialTheme.typography.bodyMedium, color = colors.onSurfaceVariant)
                OutlinedButton(
                    onClick = { navigator.open(PROVIDERS_ROUTE) },
                    modifier = Modifier.padding(top = 12.dp),
                    colors = ButtonDefaults.outlinedButtonColors(contentColor = colors.onSurface),
                ) { Text(Strings.sourcesAddTitle(context)) }
            }
        }
    }
}

/** [text] with each of [links]' words made a link where the sentence has it. */
@Composable
private fun linked(text: String, links: List<Pair<String, () -> Unit>>): AnnotatedString {
    val colors = MaterialTheme.colorScheme
    val found = links.mapNotNull { (word, go) -> text.indexOf(word).takeIf { it >= 0 }?.let { Triple(it, word, go) } }.sortedBy { it.first }
    return buildAnnotatedString {
        var at = 0
        for ((start, word, go) in found) {
            if (start < at) continue
            append(text.substring(at, start))
            withLink(
                LinkAnnotation.Clickable(word, TextLinkStyles(SpanStyle(color = colors.onSurface, textDecoration = TextDecoration.Underline))) { go() },
            ) { append(word) }
            at = start + word.length
        }
        append(text.substring(at))
    }
}
