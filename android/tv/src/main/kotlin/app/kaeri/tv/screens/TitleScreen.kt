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
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.focusGroup
import androidx.compose.ui.focus.focusProperties
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
import app.kaeri.design.Motion
import app.kaeri.design.ObiSilk
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
    // Another season comes in place: the one showing stays until the new one's head arrives, so
    // the page never blanks, the list keeps its place and focus stays on the chip pressed. A head
    // the engine kept comes at once, ahead of the page (detail.partial), and the season changes
    // then: its head crossfades in, and the rest crossfades in as it comes.
    var shownId by remember { mutableStateOf(id) }
    // The season the page was opened as: its rows wait 600 ms for placeholders, as T8 has it,
    // while another season's come at once, in the crossfade, for the rows they stand for.
    val openedAs = remember { id }
    if (shownId != id && (entries[id]?.detail != null || id in failed)) shownId = id
    val reduced = app.kaeri.tv.ui.LocalReducedMotion.current
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
    val seasons = entry.detail?.seasons.orEmpty()
    SeasonArt(seasons, shownId, entries)
    // The episodes the head reads for its art: the season's once they are here. Before that, a
    // season with no banner whose franchise has no stills in the episodes the app has is compact,
    // as it will be, rather than keeping the banner's place for a still that will not come.
    val artRows = rows ?: seasons.firstNotNullOfOrNull { tables[it.id] }
        ?.takeIf { known -> known.none { it.thumbnail != null } }?.let { emptyList() }
    ScrollRule(HeldScroll) {
        LazyColumn(
            state = list,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = 120.dp),
            verticalArrangement = Arrangement.spacedBy(Canvas.RowGap),
        ) {
            item(key = "header", contentType = "header") {
                Header(entry, rows, artRows, action, noSources, memory, onPlay, onAddStore, modifier = swapFade().onFocusChanged {
                    if (it.hasFocus) scope.launch { list.animateScrollToItem(0) }
                })
            }
            if (seasons.size > 1) {
                item(key = "seasons", contentType = "seasons") {
                    // The chip pressed shows chosen at once, before its season arrives.
                    Seasons(id, seasons, memory, onSeason, swapFade().settlesAt(list, scope, "seasons", rowTop)) { season ->
                        // The banner of the chip in focus, decoded for the press.
                        entries[season]?.bannerUrl?.let { store.artwork.request(it, app.kaeri.tv.ui.BANNER_PIXELS) }
                    }
                }
            }
            if (rows == null || rows.isNotEmpty()) {
                item(key = "episodes", contentType = "row") {
                    val placed = swapFade().settlesAt(list, scope, "episodes", rowTop)
                    // Each season's row starts at its own next episode, and crossfades in over the
                    // last season's. The row leaving takes no focus.
                    androidx.compose.animation.AnimatedContent(
                        shownId,
                        transitionSpec = {
                            if (reduced) androidx.compose.animation.EnterTransition.None togetherWith androidx.compose.animation.ExitTransition.None
                            else fadeIn(Motion.standard()) togetherWith fadeOut(Motion.standard())
                        },
                        label = "season-episodes", // i18n-skip
                    ) { season ->
                        val leaving = transition
                        val seasonEntry = entries[season] ?: entry
                        val seasonRows = tables[season]
                        val seasonAction = remember(seasonEntry, seasonRows) { PlayAction.of(seasonEntry, rows = seasonRows) }
                        Box(Modifier.closedWhile { leaving.targetState != androidx.compose.animation.EnterExitState.Visible }) {
                            Episodes(
                                seasonEntry, seasonRows,
                                if (season == shownId && switching) stringResource(R.string.state_loading) else dubs[season]?.line(LocalContext.current),
                                seasonAction, spoilers, memory, onPlay, onPlayFromStart, placed,
                                placeholdersAtOnce = season != openedAs,
                            )
                        }
                    }
                }
            }
            // The header brings the relations by kind and id, so the row is there or gone as the
            // whole page will have it, with its cards' places: a card whose title is not here yet
            // is a placeholder of its size, and crossfades into the card when the page fills it in.
            val related = entry.detail?.relations.orEmpty()
            if (related.isNotEmpty()) {
                item(key = "related", contentType = "row") {
                    BrowseRow(stringResource(R.string.title_related), modifier = swapFade().settlesAt(list, scope, "related", rowTop)) {
                        items(related, key = { it.id.toLong() }, contentType = { "poster" }) { linked ->
                            androidx.compose.animation.Crossfade(linked.title.isNotEmpty(), animationSpec = Motion.standard(), label = "related-card") { named -> // i18n-skip
                                if (named) {
                                    PosterCard(
                                        linked.title,
                                        linked.coverUrl,
                                        linked.coverColor,
                                        onClick = { onOpen(linked.id) },
                                        line = linked.relation?.name?.lowercase()?.replace('_', ' ')?.replaceFirstChar { it.uppercase() },
                                        modifier = Modifier.remembered(memory, "related/${linked.id}"),
                                    )
                                } else {
                                    app.kaeri.tv.ui.CardPlaceholder(Canvas.PosterWidth, Canvas.PosterHeight)
                                }
                            }
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
    /** The episodes the head's art reads: [rows], or a guess while they come. */
    artRows: List<EpisodeRow>?,
    action: PlayAction,
    noSources: Boolean,
    memory: FocusMemory,
    onPlay: (AnilistId, UInt) -> Unit,
    onAddStore: () -> Unit,
    modifier: Modifier,
) {
    // Another season's head crossfades in over this one's, as after a season change.
    val crossfade = true
    val store = LocalAppStore.current
    val actions = LocalActions.current
    val context = LocalContext.current
    var watching by remember { mutableStateOf(false) }
    HeroHeader(entry, artRows, modifier, crossfade = crossfade) {
        Spacer(Modifier.height(6.dp))
        app.kaeri.tv.ui.Swap(entry.metadataLine(context), crossfade) {
            Text(it, style = MaterialTheme.typography.labelSmall, color = ObiSilk.Haze, maxLines = 1)
        }
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

/**
 * The haze line under the actions: where else it is, or where it is listed when not in your
 * sources. The answer comes after the page, so the line opens on standard and fades in, and
 * Seasons and Episodes below move down with it rather than jumping.
 */
@Composable
private fun ServiceLine(availability: app.kaeri.engine.model.TitleAvailability?, memory: FocusMemory) {
    val line = when (availability?.case) {
        AvailabilityCase.PLAYS_ALSO_ON, AvailabilityCase.ON_SERVICE ->
            availability.alsoOn.takeIf { it.isNotEmpty() }?.let { stringResource(R.string.licensed_also_on, app.kaeri.tv.signin.joinServices(it.map { link -> link.service })) }
        AvailabilityCase.NOT_IN_SOURCES ->
            availability.others.takeIf { it.isNotEmpty() }?.let { stringResource(R.string.title_listed_on, app.kaeri.tv.signin.joinServices(it)) }
        AvailabilityCase.PLAYS_HERE, null -> null
    }
    // The last line shown, kept while it closes.
    var kept by remember { mutableStateOf(line) }
    if (line != null) kept = line
    AnimatedVisibility(
        line != null,
        enter = expandVertically(Motion.standard(), expandFrom = Alignment.Top) + fadeIn(Motion.fadeIn()),
        exit = shrinkVertically(Motion.standard(), shrinkTowards = Alignment.Top) + fadeOut(Motion.fadeOut()),
        label = "service-line", // i18n-skip
    ) {
        Text(kept.orEmpty(), style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, maxLines = 2, modifier = Modifier.padding(top = 12.dp).widthIn(max = 640.dp))
    }
}

/**
 * Every season's banner and cover, once the app has its head (the engine says the franchise's
 * heads as cards once a page knows its seasons), so a season changed to shows its art at once: the
 * banners of the two either side of [here] decoded, and the rest fetched only to the disk, which
 * a chip coming into focus then decodes in a moment. A TV has 40 MB of art to share.
 */
@Composable
private fun SeasonArt(seasons: List<LinkedTitle>, here: AnilistId, entries: Map<AnilistId, Entry>) {
    val store = LocalAppStore.current
    val order = remember(seasons, here) {
        val at = seasons.indexOfFirst { it.id == here }.coerceAtLeast(0)
        seasons.withIndex().filter { it.value.id != here }.sortedBy { kotlin.math.abs(it.index - at) }.map { it.value.id }
    }
    val art = order.map { season -> entries[season]?.let { it.bannerUrl to it.coverUrl } }
    LaunchedEffect(art) {
        art.forEachIndexed { index, urls ->
            val (banner, cover) = urls ?: return@forEachIndexed
            if (banner != null) {
                if (index < DECODED_AHEAD) store.artwork.request(banner, app.kaeri.tv.ui.BANNER_PIXELS) else store.artwork.warm(banner)
            }
            cover?.let(store.artwork::warm)
        }
    }
}

/** How many seasons nearest the one on screen have their banners decoded ahead. */
private const val DECODED_AHEAD = 2

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
    /** A chip came into focus, for its season's art to be ready for the press. */
    onFocusSeason: (AnilistId) -> Unit = {},
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
                    .onFocusChanged { if (it.isFocused && !chosen) onFocusSeason(season.id) }
                    .remembered(memory, "season/${season.id}"),
            )
        }
    }
}

@Composable
private fun Episodes(
    entry: Entry,
    rows: List<EpisodeRow>?,
    dub: String?,
    action: PlayAction,
    spoilers: Boolean,
    memory: FocusMemory,
    onPlay: (AnilistId, UInt) -> Unit,
    onPlayFromStart: (AnilistId, UInt) -> Unit,
    modifier: Modifier,
    /** Placeholders at once while the rows come, rather than after T8's 600 ms: for a season changed to. */
    placeholdersAtOnce: Boolean = false,
) {
    val store = LocalAppStore.current
    val host = LocalActions.current
    val context = LocalContext.current
    val next = rows?.indexOfFirst { it.number == action.episode.toString() }?.takeIf { it >= 0 } ?: 0
    val state = rememberLazyListState(initialFirstVisibleItemIndex = next)
    // A row that came before its episodes starts at its next one once they come.
    val cameEmpty = remember { rows == null }
    LaunchedEffect(rows != null) { if (cameEmpty && rows != null && next > 0) state.scrollToItem(next) }
    val nextRequester = remember { FocusRequester() }
    val waited = placeholdersAfterWait(rows == null && !placeholdersAtOnce)
    val placeholders = if (placeholdersAtOnce) rows == null else waited
    val failed = store.titles.episodesFailed.collectAsStateWithLifecycle().value[entry.id]
    val found by store.discussion.found.collectAsStateWithLifecycle()
    val settings by store.settings.view.collectAsStateWithLifecycle()
    val discussions = settings == null || store.settings.isOn(app.kaeri.engine.model.SettingKey.EPISODE_DISCUSSIONS, settings)
    BrowseRow(stringResource(R.string.title_episodes), modifier = modifier, subtitle = dub, state = state, fallback = nextRequester) {
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
                items(PLACEHOLDER_CARDS, key = { "placeholder/$it" }) { Box(swapFade()) { app.kaeri.tv.ui.CardPlaceholder(Canvas.CardWidth, Canvas.CardHeight) } }
            }
            return@BrowseRow
        }
        items(rows, key = { it.number }, contentType = { "episode" }) { row ->
            val index = rows.indexOf(row)
            val playsNext = index == next
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
                spoiler = spoilers && index > next && !row.completed && row.thumbnail != null,
                // One no source lists plays nothing: Center offers what can be done instead.
                onClick = {
                    if (row.unsourced) host?.open(episodeSheet(context, store, entry, rows, row, onPlayFromStart))
                    else row.number.toUIntOrNull()?.let { onPlay(entry.id, it) }
                },
                sheet = { episodeSheet(context, store, entry, rows, row, onPlayFromStart) },
                modifier = Modifier
                    .then(if (playsNext) Modifier.focusRequester(nextRequester) else Modifier)
                    .remembered(memory, "episode/${row.number}"),
            )
            }
        }
    }
}

@Composable
private fun EpisodeCard(
    row: EpisodeRow,
    entry: Entry,
    line: String?,
    progress: Float?,
    discussion: String?,
    spoiler: Boolean,
    onClick: () -> Unit,
    sheet: () -> ActionSheet,
    modifier: Modifier,
) {
    val context = LocalContext.current
    val base = cardLabel(context, row.cardTitle(context), line, progress)
    val label = if (row.completed) Strings.tvCardLabelWatched(context, base) else base
    Column(Modifier.width(Canvas.CardWidth)) {
        ArtCard(onClick, Canvas.CardWidth, Canvas.CardHeight, modifier, sheet, label) {
            // One no source lists is dimmed under its focus ring, which stays as bright as any.
            ArtImage(
                row.thumbnail ?: entry.bannerUrl,
                CoverPlate.parse(entry.coverColor),
                pixels(Canvas.CardWidth),
                Modifier.fillMaxSize().alpha(if (row.unsourced) 0.45f else 1f),
                spoiler = spoiler,
            )
            if (progress != null) ProgressEdge(progress, Modifier.align(Alignment.BottomStart))
            if (row.completed) {
                Box(
                    Modifier.align(Alignment.TopEnd).padding(8.dp).size(24.dp).background(ObiSilk.Ground.copy(alpha = 0.72f), CircleShape),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(TvIcons.Check, contentDescription = null, tint = ObiSilk.Water, modifier = Modifier.size(16.dp))
                }
            }
        }
        CardText(row.cardTitle(context), line, titleColor = if (row.completed || row.unsourced) ObiSilk.Haze else ObiSilk.Silk)
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
            // Another season's details, or the rest of a season's as its page comes after its
            // head, crossfade in over these.
            val shown = DetailsShown(
                synopsisWithoutSpoilers(entry.synopsis),
                listOfNotNull(
                    entry.studio?.let { stringResource(R.string.title_studio) to it },
                    entry.genres.takeIf { it.isNotEmpty() }?.let { stringResource(R.string.filter_genres) to it.joinToString(", ") }, // i18n-skip
                    entry.source?.words(LocalContext.current)?.let { stringResource(R.string.title_based_on) to it },
                ),
            )
            app.kaeri.tv.ui.Swap(shown, crossfade = true) { (synopsis, lines) ->
                Column(Modifier.padding(16.dp)) {
                    if (synopsis.isNotEmpty()) {
                        Text(synopsis, style = MaterialTheme.typography.bodyLarge, maxLines = 6, overflow = TextOverflow.Ellipsis)
                        Spacer(Modifier.height(16.dp))
                    }
                    lines.forEach { (label, value) ->
                        Row(Modifier.padding(vertical = 2.dp)) {
                            Text(label, style = MaterialTheme.typography.bodyMedium, color = ObiSilk.Haze, modifier = Modifier.width(96.dp))
                            Text(value, style = MaterialTheme.typography.bodyMedium, maxLines = 2)
                        }
                    }
                }
            }
        }
    }
}

/** A group focus does not go into while [closed] says so, as a row crossfading out. */
private fun Modifier.closedWhile(closed: () -> Boolean): Modifier =
    focusProperties { onEnter = { if (closed()) cancelFocusChange() } }.focusGroup()

/** What the details say: the synopsis, then each line's label and value. */
private data class DetailsShown(val synopsis: String, val lines: List<Pair<String, String>>)

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
): ActionSheet {
    val upTo = rows.takeWhile { it.number != row.number }.map { it.number } + row.number
    val number = row.number.toUIntOrNull()?.takeIf { !row.metadataOnly && !row.unsourced }
    val title = "${entry.title} · ${row.cardTitle(context)}" // i18n-skip
    val episode = row.number.toUIntOrNull()
    val comments = episode?.takeIf { row.completed && (store.settings.view.value == null || store.discussionsOn()) }?.let { store.discussion.count(entry.id, it) }
    return ActionSheet(
        title,
        listOfNotNull(
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
