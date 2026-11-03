// The title page: the banner header, the title, its metadata in caps and the action row with the
// amber action focused; then the franchise's seasons as a row of tabs, when it has more than one;
// the episodes, focus landing on the next one to play; Related; and the details. Back returns to
// the opener, whichever season the tabs went to.

package app.kaeri.tv.screens

import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import app.kaeri.i18n.R
import app.kaeri.tv.ui.TrailerScreen
import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import app.kaeri.i18n.Strings
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import kotlinx.coroutines.launch
import app.kaeri.design.CoverPlate
import app.kaeri.design.ObiSilk
import app.kaeri.design.Reveals
import app.kaeri.design.rememberTagged
import app.kaeri.engine.ffi.episodeRangeToOpen
import app.kaeri.engine.ffi.episodeRanges
import app.kaeri.engine.ffi.episodeText
import app.kaeri.engine.model.EpisodeText
import androidx.compose.ui.unit.sp
import androidx.compose.runtime.getValue
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.model.Entry
import app.kaeri.engine.model.EpisodeRow
import app.kaeri.engine.model.LibrarySegment
import app.kaeri.engine.model.LinkedTitle
import app.kaeri.engine.model.SettingKey
import app.kaeri.store.AppStore
import app.kaeri.store.label
import app.kaeri.store.line
import app.kaeri.store.text
import app.kaeri.store.words
import app.kaeri.tv.PlayAction
import app.kaeri.tv.cardLine
import app.kaeri.tv.cardTitle
import app.kaeri.tv.metadataLine
import app.kaeri.tv.synopsisWithoutSpoilers
import app.kaeri.tv.ui.ActionSheet
import app.kaeri.tv.ui.DiscussionPanel
import app.kaeri.engine.model.AvailabilityCase
import app.kaeri.tv.ui.AmberButton
import app.kaeri.tv.ui.openOnTv
import app.kaeri.tv.remote.LocalRemoteHost
import app.kaeri.tv.ui.ArtCard
import app.kaeri.tv.ui.ArtImage
import app.kaeri.tv.ui.BrowseRow
import app.kaeri.tv.ui.ButtonLabel
import app.kaeri.tv.ui.Canvas
import app.kaeri.tv.ui.CardAction
import app.kaeri.tv.ui.CardText
import app.kaeri.tv.ui.EnterFocus
import app.kaeri.tv.ui.FocusMemory
import app.kaeri.tv.ui.HeldScroll
import app.kaeri.tv.ui.HeroHeader
import app.kaeri.tv.ui.LocalActions
import app.kaeri.tv.ui.LocalAppStore
import app.kaeri.tv.ui.PosterCard
import app.kaeri.tv.ui.ProgressEdge
import app.kaeri.tv.ui.ReleaseList
import app.kaeri.tv.ui.ScrollRule
import app.kaeri.tv.ui.TextBar
import app.kaeri.tv.ui.StateBlock
import app.kaeri.tv.ui.TonalButton
import app.kaeri.tv.ui.TonalIconButton
import app.kaeri.tv.ui.cardLabel
import app.kaeri.tv.ui.focusSoon
import app.kaeri.tv.ui.showSettingsSheet
import app.kaeri.tv.ui.sourcePickerSheet
import app.kaeri.tv.ui.matchPanel
import app.kaeri.store.MatchWords
import app.kaeri.tv.ui.statusSheet
import app.kaeri.tv.ui.swapFade
import app.kaeri.tv.ui.TvIcons
import app.kaeri.tv.ui.TvTab
import app.kaeri.tv.ui.firstFocus
import app.kaeri.tv.ui.pixels
import app.kaeri.tv.ui.placeholdersAfterWait
import app.kaeri.tv.ui.remembered
import app.kaeri.tv.ui.settlesAt

@Composable
fun TitleScreen(
    id: AnilistId,
    memory: FocusMemory,
    onOpen: (AnilistId) -> Unit,
    onPlay: (AnilistId, UInt) -> Unit,
    onPlayFromStart: (AnilistId, UInt) -> Unit = onPlay,
    onAddStore: () -> Unit = {},
    /** Another season's page in this one's place. */
    onSeason: (AnilistId) -> Unit = onOpen,
) {
    val store = LocalAppStore.current
    LaunchedEffect(id) {
        store.loadDetail(id)
        store.loadEpisodes(id)
        store.loadDub(id)
    }
    val entries by store.titles.entries.collectAsStateWithLifecycle()
    val tables by store.titles.episodes.collectAsStateWithLifecycle()
    val dubs by store.titles.dubLines.collectAsStateWithLifecycle()
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val failed by store.titles.failed.collectAsStateWithLifecycle()
    // Another season comes in place: the one showing stays until the new one's detail arrives,
    // so the page never blanks, the list keeps its place and focus stays on the chip pressed.
    var shownId by remember { mutableStateOf(id) }
    if (shownId != id && (entries[id]?.detail != null || id in failed)) shownId = id
    val switching = shownId != id
    val entry = entries[shownId]
    val rows = tables[shownId]
    val spoilers = settings?.let { store.settings.isOn(SettingKey.SPOILER_PROTECTION, it) } ?: true

    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val rowTop = with(LocalDensity.current) { 48.dp.roundToPx() }

    val providersLoaded by store.providers.loaded.collectAsStateWithLifecycle()
    val providers by store.providers.rows.collectAsStateWithLifecycle()
    // No source can play anything: the amber action leads to a plugin store instead.
    val noSources = providersLoaded && providers.none { it.usable }
    val waiting = placeholdersAfterWait(entry == null && id !in failed)
    if (entry == null) {
        if (id in failed) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                StateBlock(
                    stringResource(R.string.title_load_failed),
                    action = stringResource(R.string.action_try_again),
                    onAction = { store.loadDetail(id) },
                    actionModifier = Modifier.firstFocus(memory, "retry"),
                )
            }
            EnterFocus(memory, entry = "failed/$id")
        } else if (waiting) {
            TitlePlaceholder()
        }
        return
    }
    val action = remember(entry, rows) { PlayAction.of(entry, rows = rows) }
    ScrollRule(HeldScroll) {
        LazyColumn(
            state = list,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(Canvas.RowGap),
        ) {
            item(key = "header", contentType = "header") {
                Header(entry, rows, action, noSources, memory, onPlay, onAddStore, swapFade().onFocusChanged {
                    if (it.hasFocus) scope.launch { list.animateScrollToItem(0) }
                })
            }
            val seasons = entry.detail?.seasons.orEmpty()
            if (seasons.size > 1) {
                item(key = "seasons", contentType = "seasons") {
                    // The chip pressed shows chosen at once, before its season arrives.
                    Seasons(id, seasons, memory, onSeason, swapFade().settlesAt(list, scope, "seasons", rowTop))
                }
            }
            if (rows == null || rows.isNotEmpty()) {
                item(key = "episodes", contentType = "row") {
                    // Each season's row starts at its own next episode.
                    key(shownId) {
                        Episodes(entry, rows, if (switching) stringResource(R.string.state_loading) else dubs[shownId]?.line(LocalContext.current), action, spoilers, memory, onPlay, onPlayFromStart, swapFade().settlesAt(list, scope, "episodes", rowTop))
                    }
                }
            }
            val related = entry.detail?.relations.orEmpty()
            if (related.isNotEmpty()) {
                item(key = "related", contentType = "row") {
                    BrowseRow(stringResource(R.string.title_related), modifier = swapFade().settlesAt(list, scope, "related", rowTop)) {
                        items(related, key = { it.id.toLong() }, contentType = { "poster" }) { linked ->
                            PosterCard(
                                linked.title,
                                linked.coverUrl,
                                linked.coverColor,
                                onClick = { onOpen(linked.id) },
                                line = linked.relation?.name?.lowercase()?.replace('_', ' ')?.replaceFirstChar { it.uppercase() },
                                modifier = Modifier.remembered(memory, "related/${linked.id}"),
                            )
                        }
                    }
                }
            }
            item(key = "details", contentType = "details") {
                Details(entry, memory, swapFade().settlesAt(list, scope, "details", rowTop))
            }
        }
    }
    // Once per page: a season coming in place keeps the focus it has.
    EnterFocus(memory)
}


@Composable
private fun Header(
    entry: Entry,
    rows: List<EpisodeRow>?,
    action: PlayAction,
    noSources: Boolean,
    memory: FocusMemory,
    onPlay: (AnilistId, UInt) -> Unit,
    onAddStore: () -> Unit,
    modifier: Modifier,
) {
    val store = LocalAppStore.current
    val actions = LocalActions.current
    val context = LocalContext.current
    var watching by remember { mutableStateOf(false) }
    HeroHeader(entry, rows, modifier) {
        Spacer(Modifier.height(6.dp))
        Text(entry.metadataLine(context), style = MaterialTheme.typography.labelSmall, color = ObiSilk.Haze, maxLines = 1)
        val uncertain = store.match.state.collectAsStateWithLifecycle().value.asked[entry.id]
        if (uncertain != null) {
            Spacer(Modifier.height(8.dp))
            Text(MatchWords.uncertain(context, uncertain), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 2)
        }
        Spacer(Modifier.height(16.dp))
        // Whether a source plays it; the engine works out which licensed service case this is.
        val playsHere = !noSources && rows != null && rows.firstOrNull()?.metadataOnly != true
        LaunchedEffect(entry.id, playsHere, rows != null) { if (rows != null) store.loadAvailability(entry.id, playsHere) }
        val availability = store.licensed.availability.collectAsStateWithLifecycle().value[entry.id]
        val remoteHost = LocalRemoteHost.current
        val onService = availability?.main?.takeIf { availability.case == AvailabilityCase.ON_SERVICE }
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (onService != null) {
                AmberButton(
                    onClick = { openOnTv(context, store, actions, entry.id, onService, remote = remoteHost?.remote) },
                    icon = null,
                    modifier = Modifier.firstFocus(memory, "play"),
                ) { ButtonLabel(stringResource(R.string.title_watch_on, onService.service)) }
            } else if (noSources) {
                AmberButton(onClick = onAddStore, icon = null, modifier = Modifier.firstFocus(memory, "play")) { ButtonLabel(stringResource(R.string.tv_add_store_to_play)) }
            } else {
                AmberButton(
                    onClick = { onPlay(entry.id, action.episode) },
                    // Still focusable while it waits for the episode, so it can say when it is out.
                    enabled = action.playable,
                    modifier = Modifier.firstFocus(memory, "play"),
                ) { ButtonLabel(if (action.unsourced) Strings.episodeUnsourced(context) else action.label(context)) }
            }
            val match by store.match.state.collectAsStateWithLifecycle()
            val asks = match.asks(entry.id)
            // Once the match is settled the button is gone: focus goes back to Play.
            var wasAsking by remember { mutableStateOf(asks) }
            LaunchedEffect(asks) {
                if (wasAsking && !asks) memory.focus("play")
                wasAsking = asks
            }
            if (asks) {
                Spacer(Modifier.width(16.dp))
                AmberButton(
                    onClick = { actions?.open(matchPanel(context, store, entry, fromMenu = false, host = actions)) },
                    icon = TvIcons.Search,
                    modifier = Modifier.remembered(memory, "match"),
                ) { ButtonLabel(Strings.matchChoose(context)) }
            }
            Spacer(Modifier.width(16.dp))
            val trailer = entry.trailer
            if (trailer != null) {
                TonalButton(
                    onClick = { watching = true },
                    modifier = Modifier.remembered(memory, "trailer"),
                ) { ButtonLabel(stringResource(R.string.title_trailer)) }
                Spacer(Modifier.width(16.dp))
                if (watching) {
                    TrailerScreen(trailer) {
                        watching = false
                        memory.focus("trailer")
                    }
                }
            }
            val status = entry.listStatus
            TonalButton(
                onClick = { actions?.open(statusSheet(context, store, entry.id, entry.title, status)) },
                icon = if (status == null) TvIcons.Library else TvIcons.Check,
                modifier = Modifier.remembered(memory, "status"),
            ) { ButtonLabel(status?.label(context) ?: stringResource(R.string.library_add_to_list)) }
            Spacer(Modifier.width(16.dp))
            TonalIconButton(
                TvIcons.More,
                stringResource(R.string.action_more),
                onClick = { actions?.open(titleSheet(context, store, entry, rows, action, Strings.sourcePickerTitle(context), actions)) },
                modifier = Modifier.remembered(memory, "more"),
            )
        }
        ServiceLine(availability, memory)
    }
}

/** The haze line under the actions: where else it is, or where it is listed when not in your sources. */
@Composable
private fun ServiceLine(availability: app.kaeri.engine.model.TitleAvailability?, memory: FocusMemory) {
    availability ?: return
    val line = when (availability.case) {
        AvailabilityCase.PLAYS_ALSO_ON, AvailabilityCase.ON_SERVICE ->
            availability.alsoOn.takeIf { it.isNotEmpty() }?.let { stringResource(R.string.licensed_also_on, app.kaeri.tv.signin.joinServices(it.map { link -> link.service })) }
        AvailabilityCase.NOT_IN_SOURCES ->
            availability.others.takeIf { it.isNotEmpty() }?.let { stringResource(R.string.title_listed_on, app.kaeri.tv.signin.joinServices(it)) }
        AvailabilityCase.PLAYS_HERE -> null
    } ?: return
    Spacer(Modifier.height(12.dp))
    Text(line, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 2, modifier = Modifier.widthIn(max = 640.dp))
}

/**
 * The season picker: the franchise's seasons in watch order as tabs, "Season 1" first, the one on
 * screen selected; a film or an OVA, which is none of them, has none selected. Focus comes in on
 * the selected tab, and Center on another shows that season's page in this one's place.
 */
@Composable
private fun Seasons(
    id: AnilistId,
    seasons: List<LinkedTitle>,
    memory: FocusMemory,
    onSeason: (AnilistId) -> Unit,
    modifier: Modifier,
) {
    val at = seasons.indexOfFirst { it.id == id }
    // The season before this one shows too, so the row says there are earlier ones.
    val state = rememberLazyListState(initialFirstVisibleItemIndex = (at - 1).coerceAtLeast(0))
    val selected = remember { FocusRequester() }
    BrowseRow(
        stringResource(R.string.title_seasons),
        modifier = modifier,
        state = state,
        subtitle = if (at >= 0) stringResource(R.string.tv_seasons_position, at + 1, seasons.size) else pluralStringResource(R.plurals.tv_seasons_count, seasons.size, seasons.size),
        fallback = if (at >= 0) selected else FocusRequester.Default,
    ) {
        items(seasons.size, key = { seasons[it].id.toLong() }, contentType = { "season" }) { index ->
            val season = seasons[index]
            val chosen = season.id == id
            TvTab(
                stringResource(R.string.season_number, index + 1),
                selected = chosen,
                onSelect = { if (!chosen) onSeason(season.id) },
                settle = null,
                chip = true,
                count = season.year?.toString(),
                modifier = Modifier
                    .then(if (chosen) Modifier.focusRequester(selected) else Modifier)
                    .remembered(memory, "season/${season.id}"),
            )
        }
    }
}

@Composable
private fun Episodes(
    entry: Entry,
    allRows: List<EpisodeRow>?,
    dub: String?,
    action: PlayAction,
    spoilers: Boolean,
    memory: FocusMemory,
    onPlay: (AnilistId, UInt) -> Unit,
    onPlayFromStart: (AnilistId, UInt) -> Unit,
    modifier: Modifier,
) {
    val store = LocalAppStore.current
    val host = LocalActions.current
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    // A long series shows one range of 100 at a time, opening on the one holding the next episode.
    val ranges = remember(allRows) { if (allRows != null && allRows.size > 100) episodeRanges(allRows, System.currentTimeMillis() / 1000) else emptyList() }
    val opens = remember(allRows, ranges) { if (allRows != null && ranges.isNotEmpty()) episodeRangeToOpen(allRows).toInt() else 0 }
    var picked by remember { mutableStateOf<Int?>(null) }
    val selected = (picked ?: opens).coerceIn(0, maxOf(0, ranges.size - 1))
    val rows = remember(allRows, ranges, selected) {
        if (allRows == null || ranges.isEmpty()) allRows else allRows.subList(ranges[selected].start.toInt().coerceIn(0, allRows.size), ranges[selected].end.toInt().coerceIn(0, allRows.size))
    }
    // Room for text is decided once for a table or range, so text that turns up later for a table
    // laid out short waits for the next layout.
    val outlook = remember(entry.id, selected, allRows == null) { allRows?.let(::episodeText) ?: EpisodeText.NONE }
    val band = outlook != EpisodeText.NONE
    val reveals = remember(entry.id) { Reveals() }
    val next = rows?.indexOfFirst { it.number == action.episode.toString() }?.takeIf { it >= 0 }
        ?: rows?.indexOfFirst { !it.completed }?.takeIf { it >= 0 } ?: 0
    val state = rememberLazyListState(initialFirstVisibleItemIndex = next)
    val nextRequester = remember { FocusRequester() }
    val placeholders = placeholdersAfterWait(rows == null)
    val failed = store.titles.episodesFailed.collectAsStateWithLifecycle().value[entry.id]
    val found by store.discussion.found.collectAsStateWithLifecycle()
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val discussions = settings == null || store.settings.isOn(app.kaeri.engine.model.SettingKey.EPISODE_DISCUSSIONS, settings)
    // Choosing a range moves focus to the new range's first unwatched card.
    var moved by remember { mutableStateOf(false) }
    androidx.compose.runtime.LaunchedEffect(selected, moved) {
        if (!moved) return@LaunchedEffect
        moved = false
        val at = rows?.indexOfFirst { !it.completed }?.takeIf { it >= 0 } ?: 0
        state.scrollToItem(at)
        repeat(5) {
            androidx.compose.runtime.withFrameNanos { }
            if (runCatching { nextRequester.requestFocus() }.isSuccess) return@LaunchedEffect
        }
    }
    val chipRequesters = remember(ranges.size) { List(ranges.size) { FocusRequester() } }
    BrowseRow(
        stringResource(R.string.title_episodes),
        modifier = modifier,
        subtitle = dub,
        state = state,
        fallback = nextRequester,
        belowHeader = if (ranges.isEmpty() || allRows == null) null else {
            {
                androidx.compose.foundation.lazy.LazyRow(
                    modifier = Modifier.fillMaxWidth().padding(vertical = Canvas.FocusRoom).focusGroup(),
                    contentPadding = PaddingValues(start = Canvas.Keyline, end = Canvas.SafeX),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(ranges.size, key = { "range/$it" }) { index ->
                        val (from, to) = rangeBounds(allRows, ranges[index])
                        TvTab(
                            Strings.episodeRange(context, from, to),
                            selected = index == selected,
                            onSelect = {
                                if (index != selected) {
                                    picked = index
                                    moved = true
                                }
                            },
                            settle = null,
                            chip = true,
                            count = when {
                                ranges[index].watched -> Strings.episodeWatched(context)
                                ranges[index].notOut -> Strings.episodeRowNotOut(context)
                                else -> null
                            },
                            modifier = Modifier.focusRequester(chipRequesters[index]).remembered(memory, "range/$index"),
                        )
                    }
                }
            }
        },
    ) {
        if (failed != null && rows.isNullOrEmpty()) {
            // The table could not be loaded: why, and a way to ask again.
            item(key = "episodes-failed", contentType = "failed") {
                Column(Modifier.padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(failed.text(context), style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Haze)
                    app.kaeri.tv.ui.TonalButton(onClick = { store.loadEpisodes(entry.id, force = true) }) {
                        app.kaeri.tv.ui.ButtonLabel(stringResource(R.string.action_try_again))
                    }
                }
            }
            return@BrowseRow
        }
        if (rows == null) {
            if (placeholders) {
                items(PLACEHOLDER_CARDS, key = { "placeholder/$it" }) { Box(swapFade()) { app.kaeri.tv.ui.CardPlaceholder(EpisodeCardDims.Width, EpisodeCardDims.StillHeight) } }
            }
            return@BrowseRow
        }
        items(rows, key = { it.number }, contentType = { "episode" }) { row ->
            val index = rows.indexOf(row)
            val playsNext = index == next
            // Ahead of the next episode under protection, unless this visit revealed it.
            val ahead = spoilers && index > next && !row.completed
            val covered = ahead && !reveals.isShown(row.number)
            Box(swapFade()) {
            EpisodeCard(
                row = row,
                entry = entry,
                line = if (row.unsourced) Strings.episodeUnsourced(context) else row.cardLine(context, entry.resume, playsNext),
                progress = if (playsNext) entry.resume?.fraction?.toFloat() else null,
                // The quiet line under a watched episode that has a thread: not focusable.
                discussion = row.number.toUIntOrNull()?.takeIf { row.completed && discussions }
                    ?.let { found[app.kaeri.store.DiscussionKey(entry.id, it)] }
                    ?.let { Strings.discussionRow(context, it.toInt()) },
                // Unwatched stills after the next one keep their colours and none of their detail.
                // Only an episode's own still can spoil; the show's banner in its place cannot.
                spoiler = covered && row.thumbnail != null,
                covered = covered,
                band = band,
                // One no source lists plays nothing: Center offers what can be done instead.
                onClick = {
                    if (row.unsourced) host?.open(episodeSheet(context, store, entry, rows, row, onPlayFromStart, covered) { reveals.show(row.number) })
                    else row.number.toUIntOrNull()?.let { onPlay(entry.id, it) }
                },
                sheet = { episodeSheet(context, store, entry, rows, row, onPlayFromStart, covered) { reveals.show(row.number) } },
                modifier = Modifier
                    .then(if (index == next) Modifier.focusRequester(nextRequester) else Modifier)
                    .remembered(memory, "episode/${row.number}"),
            )
            }
        }
    }
}

/** The measures of an episode card, from the board: three across the safe area. */
internal object EpisodeCardDims {
    val Width = 252.dp
    val StillHeight = 142.dp
    val BodyLine = 20.sp
    const val Lines = 3
}

/** A range of a long series as its chip names it: the first and last episode numbers in it. */
internal fun rangeBounds(rows: List<EpisodeRow>, range: app.kaeri.engine.model.EpisodeRange): Pair<Int, Int> {
    val first = rows.getOrNull(range.start.toInt())?.number?.toDoubleOrNull()?.toInt() ?: (range.start.toInt() + 1)
    val last = rows.getOrNull(range.end.toInt() - 1)?.number?.toDoubleOrNull()?.toInt() ?: range.end.toInt()
    return first to last
}

@Composable
private fun EpisodeCard(
    row: EpisodeRow,
    entry: Entry,
    line: String?,
    progress: Float?,
    discussion: String?,
    spoiler: Boolean,
    /** Spoiler protection covers this card's text: facts and a blurred still. */
    covered: Boolean,
    /** Whether the cards keep room for three lines under them. */
    band: Boolean,
    onClick: () -> Unit,
    sheet: () -> ActionSheet,
    modifier: Modifier,
) {
    val context = LocalContext.current
    val density = LocalDensity.current
    var focused by remember { mutableStateOf(false) }
    val title = row.title?.takeIf { it.isNotBlank() } ?: Strings.episodeNumber(context, row.number.toInt())
    val aired = row.airsAt?.takeIf { it <= System.currentTimeMillis() / 1000 }?.let { airedDate(context, it) }
    val facts = listOfNotNull(Strings.episodeLineDetail(context, row.number.toInt(), line ?: ""), aired)
        .joinToString(" · ") // i18n-skip
        .replace(Regex(" · $"), "")
    val synopsis = row.description?.takeIf { it.isNotBlank() && !covered }
    val hiddenWord = stringResource(R.string.episode_synopsis_hidden)
    // The card reads as one: number, title, facts, then the synopsis last, or that it is hidden.
    val base = cardLabel(context, title, facts, progress)
    val said = base.let { if (row.completed) Strings.tvCardLabelWatched(context, it) else it } +
        (synopsis?.let { ". $it" } ?: if (covered) ". $hiddenWord" else "") // i18n-skip
    Column(Modifier.width(EpisodeCardDims.Width).onFocusChanged { focused = it.hasFocus }) {
        ArtCard(onClick, EpisodeCardDims.Width, EpisodeCardDims.StillHeight, modifier, sheet, said) {
            // The show's banner stands in for a still the episode does not have, and is never blurred.
            ArtImage(
                row.thumbnail ?: entry.bannerUrl,
                CoverPlate.parse(entry.coverColor),
                pixels(EpisodeCardDims.Width),
                Modifier.fillMaxSize().alpha(if (row.unsourced) 0.45f else 1f),
                spoiler = spoiler,
            )
            if (progress != null) ProgressEdge(progress, Modifier.align(Alignment.BottomStart))
            if (row.completed) {
                Box(
                    Modifier.align(Alignment.TopEnd).padding(8.dp).size(24.dp).background(ObiSilk.Ground.copy(alpha = 0.72f), CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(TvIcons.Check, contentDescription = null, tint = ObiSilk.Water, modifier = Modifier.size(24.dp * 2 / 3))
                }
            }
        }
        Spacer(Modifier.height(10.dp))
        Text(
            title,
            style = MaterialTheme.typography.titleSmall,
            color = if (row.completed || row.unsourced) ObiSilk.Haze else ObiSilk.Silk,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.clearAndSetSemantics {},
        )
        Text(
            facts,
            style = MaterialTheme.typography.labelMedium,
            color = ObiSilk.Haze,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.clearAndSetSemantics {},
        )
        if (band) {
            // The room for text: it fades in on the fade-in token, and the focused card's goes from haze to silk.
            val room = with(density) { (EpisodeCardDims.BodyLine * EpisodeCardDims.Lines).toDp() }
            Box(Modifier.padding(top = 6.dp).fillMaxWidth().height(room).clearAndSetSemantics {}) {
                androidx.compose.animation.Crossfade(synopsis, animationSpec = app.kaeri.design.Motion.fadeIn(), label = "synopsis") { shown -> // i18n-skip
                    if (shown != null) {
                        val color by androidx.compose.animation.animateColorAsState(if (focused) ObiSilk.Silk else ObiSilk.Haze, app.kaeri.design.Motion.quick(), label = "synopsis-color") // i18n-skip
                        Text(
                            rememberTagged(shown),
                            style = MaterialTheme.typography.bodyMedium,
                            color = color,
                            maxLines = EpisodeCardDims.Lines,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
        }
        if (discussion != null) {
            Text(
                discussion,
                style = MaterialTheme.typography.bodyMedium,
                color = ObiSilk.Haze,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(top = 2.dp).clearAndSetSemantics {},
            )
        }
    }
}

/** "Aired 21 May 2014". */
private fun airedDate(context: Context, airsAt: Long): String =
    Strings.titleFactsAired(
        context,
        java.time.format.DateTimeFormatter.ofLocalizedDate(java.time.format.FormatStyle.MEDIUM)
            .format(java.time.Instant.ofEpochSecond(airsAt).atZone(java.time.ZoneId.systemDefault())),
    )

/** The synopsis (six lines at most), then studio, genres and source. Focusable, so it scrolls in. */
@Composable
private fun Details(entry: Entry, memory: FocusMemory, modifier: Modifier) {
    Column(modifier.padding(start = Canvas.Keyline - 16.dp, end = Canvas.SafeX)) {
        val actions = LocalActions.current
        Text(
            stringResource(R.string.title_details),
            style = MaterialTheme.typography.titleMedium,
            color = ObiSilk.Silk,
            modifier = Modifier.padding(start = 16.dp, bottom = Canvas.HeaderGap).semantics { heading() },
        )
        val full = synopsisWithoutSpoilers(entry.synopsis)
        Surface(
            // Center opens the whole synopsis in a panel.
            onClick = { if (full.isNotEmpty()) actions?.open(ActionSheet(entry.title, body = { Synopsis(full) })) },
            modifier = Modifier.widthIn(max = 640.dp).remembered(memory, "details"),
            shape = ClickableSurfaceDefaults.shape(RoundedCornerShape(12.dp)),
            colors = ClickableSurfaceDefaults.colors(
                containerColor = androidx.compose.ui.graphics.Color.Transparent,
                contentColor = ObiSilk.Silk,
                focusedContainerColor = ObiSilk.Raised,
                focusedContentColor = ObiSilk.Silk,
            ),
            scale = ClickableSurfaceDefaults.scale(focusedScale = 1f),
        ) {
            Column(Modifier.padding(16.dp)) {
                val synopsis = synopsisWithoutSpoilers(entry.synopsis)
                if (synopsis.isNotEmpty()) {
                    Text(synopsis, style = MaterialTheme.typography.bodyLarge, maxLines = 6, overflow = TextOverflow.Ellipsis)
                    Spacer(Modifier.height(16.dp))
                }
                listOfNotNull(
                    entry.studio?.let { stringResource(R.string.title_studio) to it },
                    entry.genres.takeIf { it.isNotEmpty() }?.let { stringResource(R.string.filter_genres) to it.joinToString(", ") }, // i18n-skip
                    entry.source?.words(LocalContext.current)?.let { stringResource(R.string.title_based_on) to it },
                ).forEach { (label, value) ->
                    Row(Modifier.padding(vertical = 2.dp)) {
                        Text(label, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.width(96.dp))
                        Text(value, style = MaterialTheme.typography.bodyMedium, maxLines = 2)
                    }
                }
            }
        }
    }
}

/** The whole synopsis in a panel: focusable, so Up and Down scroll it. */
@Composable
private fun Synopsis(text: String) {
    val scroll = rememberScrollState()
    val focus = remember { FocusRequester() }
    val scope = rememberCoroutineScope()
    val step = with(LocalDensity.current) { 120.dp.toPx() }
    Box(
        Modifier
            .fillMaxSize()
            .focusRequester(focus)
            .onPreviewKeyEvent { event ->
                if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                when (event.key) {
                    Key.DirectionDown -> { scope.launch { scroll.animateScrollBy(step) }; true }
                    Key.DirectionUp -> { scope.launch { scroll.animateScrollBy(-step) }; true }
                    else -> false
                }
            }
            .focusable()
            .verticalScroll(scroll),
    ) {
        Text(text, style = MaterialTheme.typography.bodyLarge, color = ObiSilk.Silk, modifier = Modifier.padding(horizontal = 16.dp))
    }
    LaunchedEffect(Unit) { focus.focusSoon() }
}

@Composable
private fun TitlePlaceholder() {
    Column(Modifier.fillMaxWidth()) {
        Spacer(Modifier.height(212.dp))
        Column(Modifier.padding(start = Canvas.Keyline)) {
            TextBar(360.dp, height = 32.dp)
            Spacer(Modifier.height(14.dp))
            TextBar(280.dp)
        }
    }
}

private const val PLACEHOLDER_CARDS = 5

/**
 * An episode's actions: play it from the start, mark it watched or every episode up to it, or
 * choose the release it plays from. One AniList alone lists has nothing to play.
 */
fun episodeSheet(
    context: Context,
    store: AppStore,
    entry: Entry,
    rows: List<EpisodeRow>,
    row: EpisodeRow,
    onPlayFromStart: (AnilistId, UInt) -> Unit,
    /** Spoiler protection covers this episode's text and still: Show synopsis leads the actions. */
    covered: Boolean = false,
    onShow: () -> Unit = {},
): ActionSheet {
    val upTo = rows.takeWhile { it.number != row.number }.map { it.number } + row.number
    val number = row.number.toUIntOrNull()?.takeIf { !row.metadataOnly && !row.unsourced }
    val title = "${entry.title} · ${row.cardTitle(context)}" // i18n-skip
    val episode = row.number.toUIntOrNull()
    val comments = episode?.takeIf { row.completed && (store.settings.view.value == null || store.discussionsOn()) }?.let { store.discussion.count(entry.id, it) }
    val synopsis = row.description?.takeIf { it.isNotBlank() && !covered }
    return ActionSheet(
        title,
        listOfNotNull(
            if (covered) CardAction(Strings.episodeSynopsisShow(context)) { onShow() } else null,
            number?.let { CardAction(Strings.episodePlayFromBeginning(context)) { onPlayFromStart(entry.id, it) } },
            if (row.completed) {
                CardAction(Strings.episodeMarkUnwatched(context)) { store.setWatched(entry.id, listOf(row.number), watched = false) }
            } else {
                CardAction(Strings.episodeMarkWatched(context)) { store.setWatched(entry.id, listOf(row.number)) }
            },
            CardAction(Strings.tvEpisodeMarkUpToHere(context)) { store.setWatched(entry.id, upTo) },
            if (comments != null && episode != null) {
                CardAction(
                    Strings.discussionOpen(context),
                    supporting = Strings.discussionCount(context, comments.toInt()),
                    then = ActionSheet(
                        Strings.discussionTitleEpisode(context, episode.toInt()),
                        width = 560.dp,
                        body = { DiscussionPanel(entry, episode) },
                    ),
                )
            } else {
                null
            },
            number?.let {
                CardAction(Strings.playerChooseSource(context), then = ActionSheet(title, body = { EpisodeReleases(store, entry.id, row.number) }))
            },
        ),
        // The whole synopsis at the top, in its own language; the card shows three lines of it.
        width = if (synopsis != null) 520.dp else 400.dp,
        lead = synopsis?.let { text ->
            {
                Text(
                    rememberTagged(text),
                    style = MaterialTheme.typography.bodyMedium,
                    color = ObiSilk.Haze,
                    maxLines = 9,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
                )
            }
        },
    )
}

/**
 * The title page's More: Show settings, Mark watched up to the episode it would play, Hide from
 * Continue, Drop (keeps progress).
 */
fun titleSheet(context: Context, store: AppStore, entry: Entry, rows: List<EpisodeRow>?, action: PlayAction, sourcesLabel: String, host: app.kaeri.tv.ui.ActionsHost? = null): ActionSheet {
    val before = rows?.takeWhile { it.number != action.episode.toString() }?.filter { !it.completed }?.map { it.number }.orEmpty()
    return ActionSheet(
        entry.title,
        listOfNotNull(
            CardAction(Strings.showSettingsTitle(context), then = showSettingsSheet(context, store, entry.id, entry.title)),
            CardAction(sourcesLabel, then = sourcePickerSheet(store, entry, sourcesLabel)),
            if (store.match.canFix(store.providers.rows.value)) {
                CardAction(Strings.matchFix(context), then = matchPanel(context, store, entry, fromMenu = true, host = host))
            } else {
                null
            },
            before.takeIf { it.isNotEmpty() }?.let { upTo ->
                CardAction(Strings.episodeMarkUpTo(context, (action.episode - 1u).toInt())) { store.setWatched(entry.id, upTo) }
            },
            CardAction(Strings.continueHide(context)) { store.hideFromContinue(entry.id) },
            CardAction(Strings.continueDrop(context)) { store.setStatus(entry.id, LibrarySegment.DROPPED) },
        ),
    )
}

/** The episode's releases, listed as the panel opens; choosing one plays it. */
@Composable
private fun EpisodeReleases(store: AppStore, id: AnilistId, episode: String) {
    val actions = LocalActions.current
    val choices by store.sources.collectAsStateWithLifecycle()
    LaunchedEffect(id, episode) { store.loadSources(id, episode) }
    DisposableEffect(id, episode) { onDispose { store.closeSources() } }
    ReleaseList(
        choices?.takeIf { it.id == id && it.episode == episode }?.candidates,
        onPick = { candidate ->
            actions?.close()
            store.playSource(id, episode, candidate)
        },
    )
}
